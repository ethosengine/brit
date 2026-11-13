//! Replay licenses raw file identities in the parent before publishing their bytes.
use super::*;
use gix::utils::stream::{Budget, Spool};
use std::io::Seek;

impl Session {
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
        for change in &self.state.work {
            for image in [&change.before, &change.after] {
                if let switch::State::StoredFile { size, checksum, .. } = image {
                    ensure!(
                        self.state.payloads.get(checksum) == Some(size),
                        "undeclared replay payload"
                    );
                    if !matches!(self.state.phase.as_str(), "preparing" | "cancelling-prepared") {
                        switch::payload::open(&self.directory, *size, checksum)?;
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn prepare_payloads(&self, repo: &gix::Repository) -> Result<()> {
        let budget = Budget::new(repo.git_dir())?;
        for change in &self.state.work {
            let switch::State::StoredFile { size, checksum, .. } = &change.before else {
                continue;
            };
            if fs::symlink_metadata(self.directory.join(format!("payload-{checksum}"))).is_ok() {
                switch::payload::open(&self.directory, *size, checksum)?;
                continue;
            }
            let path = repo
                .workdir()
                .context("worktree required")?
                .join(gix::path::from_bstr(change.path.as_bstr())?.as_ref());
            ensure!(
                switch::payload::fingerprint(&path)? == change.before,
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
