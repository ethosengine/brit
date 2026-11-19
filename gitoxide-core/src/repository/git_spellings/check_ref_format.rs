//! `brit check-ref-format`: Git's reference-name validity check, answered by the exit code alone.

use std::io::Write;

use gix::bstr::{BStr, BString};

/// Answer like `git check-ref-format [--allow-onelevel] <refname>`: `Ok` for a valid name, status 1 for
/// an invalid one, and Git's usage status 129 when the name is missing or looks like an option.
pub fn check(refname: Option<&BString>, allow_onelevel: bool) -> gix::Result<()> {
    match refname {
        Some(name) if !name.starts_with(b"-") => {
            if is_valid(name.as_ref(), allow_onelevel) {
                Ok(())
            } else {
                Err(super::exit(1))
            }
        }
        _ => {
            // A closed stderr must not turn the usage error into a panic.
            writeln!(
                std::io::stderr(),
                "usage: brit check-ref-format [--allow-onelevel] <refname>"
            )
            .ok();
            Err(super::exit(129))
        }
    }
}

/// Whether `name` is a valid reference name for `git check-ref-format`: at least two components
/// unless `allow_onelevel`, and every rule of [`gix::validate::reference::name_partial()`].
pub fn is_valid(name: &BStr, allow_onelevel: bool) -> bool {
    (allow_onelevel || name.contains(&b'/')) && gix::validate::reference::name_partial(name).is_ok()
}

#[cfg(test)]
mod tests {
    use super::is_valid;
    use gix::bstr::ByteSlice;

    #[test]
    fn names_are_judged_like_git() {
        for valid in [
            "refs/heads/main",
            "refs/notes/brit/devices/x/build/gate%3Aelohim-storage",
            "heads/x",
        ] {
            assert!(is_valid(valid.as_bytes().as_bstr(), false), "{valid}");
        }
        for invalid in [
            "main",
            "refs/heads/a..b",
            "refs/heads/x.lock",
            "refs/heads/",
            "refs//x",
            "refs/heads/a b",
            "refs/@{x}",
            "refs/*",
        ] {
            assert!(!is_valid(invalid.as_bytes().as_bstr(), false), "{invalid}");
        }
        assert!(is_valid(b"main".as_bstr(), true));
    }
}
