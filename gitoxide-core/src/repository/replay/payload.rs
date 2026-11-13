//! Replay licenses raw file identities in the parent before publishing their bytes.
use super::*;
use gix::utils::stream::{Budget, Spool};
use std::io::Seek;

impl Session {
    /// Upgrade an unbound abort before-image, preserving any previously captured
    /// copy. The inventory is durable before raw bytes; an interrupted metadata
    /// rename is accepted only if it is exactly one of these two successors.
    pub(super) fn migrate_inline_work(&mut self, repo: &gix::Repository) -> Result<()> {
        if !self.state.work.iter().any(|change| {
            matches!(change.before, switch::State::File { .. }) || matches!(change.after, switch::State::File { .. })
        }) {
            return Ok(());
        }
        ensure!(
            self.state.phase == "retaining-abort",
            "payload upgrade requires an abort"
        );
        let mut after = self.state.clone();
        after.version = 2;
        for change in &mut after.work {
            for image in [&mut change.before, &mut change.after] {
                if let switch::State::File { mode, bytes } = image {
                    let size = bytes.len() as u64;
                    let checksum = switch::payload::checksum(&mut bytes.as_slice(), size)?;
                    after.payloads.insert(checksum.clone(), size);
                    *image = switch::State::StoredFile {
                        mode: *mode,
                        size,
                        checksum,
                    };
                }
            }
        }
        let copy = repo
            .git_dir()
            .join("brit-rebase-retained")
            .join(&self.state.nonce)
            .join(self.state.copies.to_string());
        if fs::symlink_metadata(copy).is_ok() {
            after.copies = after.copies.checked_add(1).context("replay copy count overflow")?;
        }
        self.state.version = after.version;
        self.state.payloads.clone_from(&after.payloads);
        self.state.copies = after.copies;
        let pending = self.directory.join("state.pending");
        if pending.try_exists()? {
            let candidate = Self::open_record(repo, "state.pending")?;
            let digest = json_digest(&candidate.state)?;
            let complete = digest == json_digest(&after)?;
            ensure!(
                complete || digest == json_digest(&self.state)?,
                "foreign pending payload upgrade"
            );
            switch::sync_path(&pending)?;
            fs::rename(pending, self.directory.join("state.json"))?;
            switch::sync_dir(&self.directory)?;
            self.state = candidate.state;
            if complete {
                return Ok(());
            }
        } else {
            self.save()?;
        }
        switch::checkpoint("replay-legacy-payload-licensed")?;
        let budget = Budget::new(repo.git_dir())?;
        for (change, desired) in self.state.work.iter().zip(&after.work) {
            for (image, desired) in [(&change.before, &desired.before), (&change.after, &desired.after)] {
                let (switch::State::File { bytes, .. }, switch::State::StoredFile { size, checksum, .. }) =
                    (image, desired)
                else {
                    continue;
                };
                if fs::symlink_metadata(self.directory.join(format!("payload-{checksum}"))).is_ok() {
                    switch::payload::open(&self.directory, *size, checksum)?;
                    continue;
                }
                let _charge = switch::payload::charge(&budget, &self.directory)?;
                let mut source = budget.spool()?;
                gix::utils::stream::copy(&mut bytes.as_slice(), &mut source, &gix::interrupt::IS_INTERRUPTED)?;
                source.rewind()?;
                switch::payload::persist(&self.directory, source, *size, checksum)?;
                switch::checkpoint("replay-legacy-payload-persisted")?;
            }
        }
        self.state = after;
        self.save()?;
        switch::checkpoint("replay-legacy-payload-complete")?;
        self.validate_payloads()
    }

    pub(super) fn validate_payloads(&self) -> Result<()> {
        ensure!(
            self.state.version == 2 || self.state.payloads.is_empty(),
            "legacy replay has raw payloads"
        );
        switch::payload::check_growth(&self.directory, 0)?;
        for (hash, size) in &self.state.payloads {
            ensure!(
                hash.len() == 40 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "invalid replay payload identity"
            );
            if fs::symlink_metadata(self.directory.join(format!("payload-{hash}"))).is_ok() {
                switch::payload::open(&self.directory, *size, hash)?;
            }
        }
        for image in self
            .state
            .work
            .iter()
            .flat_map(|change| [&change.before, &change.after])
            .filter(|image| matches!(image, switch::State::StoredFile { .. }))
            .cloned()
            .chain(self.state.preservation.raw_images().map(|(_, image)| image))
        {
            if let switch::State::StoredFile { size, checksum, .. } = image {
                ensure!(
                    self.state.payloads.get(&checksum) == Some(&size),
                    "undeclared replay payload"
                );
                if !matches!(self.state.phase.as_str(), "preparing" | "cancelling-prepared") {
                    switch::payload::open(&self.directory, size, &checksum)?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn prepare_payloads(&self, repo: &gix::Repository) -> Result<()> {
        let budget = Budget::new(repo.git_dir())?;
        for (path, image) in self
            .state
            .work
            .iter()
            .filter(|change| matches!(change.before, switch::State::StoredFile { .. }))
            .map(|change| (change.path.as_slice(), change.before.clone()))
            .chain(self.state.preservation.raw_images())
        {
            let switch::State::StoredFile { size, checksum, .. } = &image else {
                continue;
            };
            if fs::symlink_metadata(self.directory.join(format!("payload-{checksum}"))).is_ok() {
                switch::payload::open(&self.directory, *size, checksum)?;
                continue;
            }
            let path = repo
                .workdir()
                .context("worktree required")?
                .join(gix::path::from_bstr(path.as_bstr())?.as_ref());
            ensure!(
                switch::payload::fingerprint(&path)? == image,
                "preparation source changed: {}",
                path.display()
            );
            let _charge = switch::payload::charge(&budget, &self.directory)?;
            let mut source = budget.spool()?;
            gix::utils::stream::copy(
                &mut fs::File::open(&path)?,
                &mut source,
                &gix::interrupt::IS_INTERRUPTED,
            )?;
            source.rewind()?;
            ensure!(
                source.len() == *size && switch::payload::checksum(&mut source, *size)? == *checksum,
                "preparation source changed: {}",
                path.display()
            );
            source.rewind()?;
            switch::checkpoint("replay-payload-intent")?;
            switch::payload::persist(&self.directory, source, *size, checksum)?;
            switch::checkpoint("replay-payload-persisted")?;
        }
        self.validate_payloads()
    }

    pub(super) fn capture_payload(&mut self, repo: &gix::Repository, path: &Path) -> Result<switch::State> {
        let image = switch::payload::fingerprint(path)?;
        let switch::State::StoredFile { mode, size, checksum } = &image else {
            return Ok(image);
        };
        if self.state.payloads.get(checksum) == Some(size)
            && fs::symlink_metadata(self.directory.join(format!("payload-{checksum}"))).is_ok()
        {
            switch::payload::open(&self.directory, *size, checksum)?;
            return Ok(image);
        }
        let budget = Budget::new(repo.git_dir())?;
        let _charge = switch::payload::charge(&budget, &self.directory)?;
        let mut source = budget.spool()?;
        gix::utils::stream::copy(&mut fs::File::open(path)?, &mut source, &gix::interrupt::IS_INTERRUPTED)?;
        source.rewind()?;
        ensure!(
            source.len() == *size && switch::payload::checksum(&mut source, *size)? == *checksum,
            "replay source changed: {}",
            path.display()
        );
        self.store_payload(source, *mode)
    }

    pub(super) fn capture_after(&mut self, repo: &gix::Repository) -> Result<()> {
        let root = repo.workdir().context("worktree required")?;
        for ordinal in 0..self.state.work.len() {
            let path = root.join(gix::path::from_bstr(self.state.work[ordinal].path.as_bstr())?.as_ref());
            self.state.work[ordinal].after = self.capture_payload(repo, &path)?;
        }
        Ok(())
    }

    pub(super) fn store_payload(&mut self, mut source: Spool, mode: u32) -> Result<switch::State> {
        let size = source.len();
        source.rewind()?;
        let checksum = switch::payload::checksum(&mut source, size)?;
        source.rewind()?;
        self.state.version = 2;
        self.state.payloads.insert(checksum.clone(), size);
        self.save()?;
        switch::checkpoint("replay-payload-intent")?;
        switch::payload::persist(&self.directory, source, size, &checksum)?;
        switch::checkpoint("replay-payload-persisted")?;
        Ok(switch::State::StoredFile { mode, size, checksum })
    }
}

fn json_digest(value: &impl serde::Serialize) -> Result<String> {
    let mut writer = gix::hash::io::Write::new(std::io::sink(), gix::hash::Kind::Sha1);
    serde_json::to_writer(&mut writer, value)?;
    Ok(writer.hash.try_finalize()?.to_string())
}
