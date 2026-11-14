use std::path::PathBuf;

use crate::error::{ResultExt, message};
use crate::{
    Error, Result,
    config::tree::{Gpg, gpg},
};

pub use gix_object::signature::{
    Format,
    verify::{Outcome, Status, TrustLevel},
};

pub(crate) fn verify(commit: &crate::Commit<'_>) -> Result<Option<Outcome>> {
    verify_data(commit.repo, &commit.data, commit.id.kind())
}

pub(crate) fn verify_data(repo: &crate::Repository, data: &[u8], hash: gix_hash::Kind) -> Result<Option<Outcome>> {
    let Some((signature, signed_data)) = gix_object::CommitRefIter::signature(data, hash)? else {
        return Ok(None);
    };
    let config = repo.config_snapshot();
    let minimum_trust = config
        .string(Gpg::MIN_TRUST_LEVEL)
        .map(|value| Gpg::MIN_TRUST_LEVEL.try_into_trust_level(value))
        .transpose()?
        .unwrap_or_default();
    let format = Format::from_signature(&signature)
        .ok_or_else(|| message("The signature format is unsupported").corrupted_error())?;
    let program = super::signature_program(&config, format)
        .or_raise(|| message("Could not interpolate the configured signature-verification program path"))?;
    let options = match format {
        Format::OpenPgp => gix_object::signature::verify::Options::OpenPgp {
            program,
            program_arguments: Vec::new(),
            environment: Vec::new(),
            minimum_trust,
        },
        Format::X509 => gix_object::signature::verify::Options::X509 {
            program,
            program_arguments: Vec::new(),
            environment: Vec::new(),
            minimum_trust,
        },
        Format::Ssh => {
            let allowed_signers = config
                .trusted_path(gpg::Ssh::ALLOWED_SIGNERS_FILE)
                .or_raise(|| message("Could not interpolate a configured signature-verification path"))?
                .ok_or_else(|| {
                    Error::from_error(message(
                        "gpg.ssh.allowedSignersFile must be configured for SSH signature verification",
                    ))
                })?;
            let revocation_file = config
                .trusted_path(gpg::Ssh::REVOCATION_FILE)
                .or_raise(|| message("Could not interpolate a configured signature-verification path"))?
                .filter(|path| path.exists());
            gix_object::signature::verify::Options::Ssh {
                program,
                program_arguments: Vec::new(),
                environment: Vec::new(),
                allowed_signers: resolve_relative_to_repository(repo, allowed_signers),
                revocation_file: revocation_file.map(|path| resolve_relative_to_repository(repo, path)),
                verify_time: gix_object::CommitRefIter::from_bytes(data, hash).committer()?.time()?,
                minimum_trust,
            }
        }
    };
    let outcome = signed_data
        .verify(&signature, options)
        .or_raise(|| message("Could not verify the commit signature"))?;
    Ok(Some(outcome))
}

fn resolve_relative_to_repository(repo: &crate::Repository, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        repo.workdir().unwrap_or_else(|| repo.git_dir()).join(path)
    }
}
