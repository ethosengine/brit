//! Replay licenses raw file identities in the parent before publishing their bytes.
use super::*;
use gix::utils::stream::{Budget, Spool};
use std::{
    io::{Read, Seek, Write},
    os::unix::fs::OpenOptionsExt,
};

/// A handoff license owns bytes without changing the parent state to which the
/// live child is bound. Acceptance later incorporates this inventory atomically.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChildPayloadLicense {
    version: u32,
    binding: ParentBinding,
    size: u64,
    checksum: String,
}

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
        self.child_payload_licenses()?;
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
        self.store_payload(repo, source, *mode)
    }

    pub(super) fn capture_after(&mut self, repo: &gix::Repository) -> Result<()> {
        // Include superseded or incomplete licensed writes in the retained
        // inventory even if the user changed the resolution after interruption.
        let saved = Self::open(repo)?;
        self.state.payloads.extend(saved.child_payload_licenses()?);
        let root = repo.workdir().context("worktree required")?;
        for ordinal in 0..self.state.work.len() {
            let path = root.join(gix::path::from_bstr(self.state.work[ordinal].path.as_bstr())?.as_ref());
            self.state.work[ordinal].after = self.capture_payload(repo, &path)?;
        }
        Ok(())
    }

    /// Read-only validation: a license belongs either to the exact active
    /// child context or to a child whose acceptance is already durable.
    pub(super) fn child_payload_licenses(&self) -> Result<BTreeMap<String, u64>> {
        let owner = fs::metadata(&self.directory)?.uid();
        let mut licenses = BTreeMap::new();
        for entry in fs::read_dir(&self.directory)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(hash) = name.to_str().and_then(|name| name.strip_prefix("payload-license-")) else {
                continue;
            };
            let hash = hash
                .strip_suffix(".json")
                .context("invalid replay payload license name")?;
            ensure!(
                hash.len() == 40 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "invalid replay payload license identity"
            );
            let metadata = fs::symlink_metadata(entry.path())?;
            ensure!(
                metadata.is_file() && metadata.uid() == owner && metadata.permissions().mode().trailing_zeros() >= 6,
                "unsafe replay payload license"
            );
            // Version 1 contains only three fixed-width hashes and three bounded
            // integers. Reject corrupt oversized records before decoding strings.
            ensure!(
                metadata.len() <= 1024,
                "replay payload license at {} has {} bytes; format-1 maximum is 1024 bytes",
                entry.path().display(),
                metadata.len()
            );
            let file = fs::OpenOptions::new()
                .read(true)
                .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
                .open(entry.path())?;
            let opened = file.metadata()?;
            ensure!(
                (opened.dev(), opened.ino(), opened.len()) == (metadata.dev(), metadata.ino(), metadata.len()),
                "replay payload license changed while opening"
            );
            let mut bytes = Vec::new();
            file.take(1025).read_to_end(&mut bytes)?;
            ensure!(
                bytes.len() <= 1024,
                "replay payload license grew beyond its format-1 bound"
            );
            let license: ChildPayloadLicense = serde_json::from_slice(&bytes)?;
            ensure!(
                license.version == 1
                    && license.checksum == hash
                    && license.binding.nonce == self.state.nonce
                    && serde_json::to_vec(&license)? == bytes,
                "foreign or noncanonical replay payload license"
            );
            let accepted = self
                .state
                .accepted_children
                .iter()
                .chain(self.state.accepted_child.iter())
                .any(|(binding, _)| binding == &license.binding)
                && self.state.payloads.get(hash) == Some(&license.size);
            let active = self.state.phase == "authoring" && self.child_authorization()?.binding == license.binding;
            ensure!(
                accepted || active,
                "replay payload license has no matching child authority"
            );
            if fs::symlink_metadata(self.directory.join(format!("payload-{hash}"))).is_ok() {
                switch::payload::open(&self.directory, license.size, hash)?;
            }
            licenses.insert(hash.to_owned(), license.size);
        }
        Ok(licenses)
    }

    fn license_child_payload(&self, repo: &gix::Repository, size: u64, checksum: &str) -> Result<()> {
        let saved = Self::open(repo)?;
        let existing = saved.child_payload_licenses()?;
        if let Some(expected) = existing.get(checksum) {
            ensure!(*expected == size, "child payload license size changed");
            switch::sync_path(&self.directory.join(format!("payload-license-{checksum}.json")))?;
            return switch::sync_dir(&self.directory);
        }
        let license = ChildPayloadLicense {
            version: 1,
            binding: saved.child_authorization()?.binding,
            size,
            checksum: checksum.to_owned(),
        };
        let budget = Budget::new(repo.git_dir())?;
        let _charge = switch::payload::charge(&budget, &self.directory)?;
        let mut source = budget.spool()?;
        source.write_all(&serde_json::to_vec(&license)?)?;
        switch::private_publish(&self.directory.join(format!("payload-license-{checksum}.json")), source)?;
        switch::sync_dir(&self.directory)
    }

    pub(super) fn store_payload(
        &mut self,
        repo: &gix::Repository,
        mut source: Spool,
        mode: u32,
    ) -> Result<switch::State> {
        let size = source.len();
        source.rewind()?;
        let checksum = switch::payload::checksum(&mut source, size)?;
        source.rewind()?;
        self.state.version = 2;
        self.state.payloads.insert(checksum.clone(), size);
        if self.state.phase == "authoring" {
            self.license_child_payload(repo, size, &checksum)?;
        } else {
            self.save()?;
        }
        switch::checkpoint("replay-payload-intent")?;
        if self.state.phase == "authoring" {
            switch::checkpoint("replay-child-payload-intent")?;
        }
        switch::payload::persist(&self.directory, source, size, &checksum)?;
        switch::checkpoint("replay-payload-persisted")?;
        if self.state.phase == "authoring" {
            switch::checkpoint("replay-child-payload-persisted")?;
        }
        Ok(switch::State::StoredFile { mode, size, checksum })
    }
}

fn json_digest(value: &impl serde::Serialize) -> Result<String> {
    let mut writer = gix::hash::io::Write::new(std::io::sink(), gix::hash::Kind::Sha1);
    serde_json::to_writer(&mut writer, value)?;
    Ok(writer.hash.try_finalize()?.to_string())
}
