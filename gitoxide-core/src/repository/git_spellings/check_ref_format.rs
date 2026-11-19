//! `brit check-ref-format`: Git's reference-name validity check, answered by the exit code alone.

use gix::bstr::BStr;

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
