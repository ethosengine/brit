//! Git's spellings of ref resolution and listing, with Git's output shapes for the forms agents use.
//!
//! Each module answers one Git command name. Brit's own verbs (`revision resolve`, `remote refs`,
//! `branch list`) stay the native interface; these exist so that a guessed Git spelling works and
//! points back to them.
//!
//! ### Known differences shared by these spellings
//!
//! - A relative `GIT_DIR` is printed absolute by `rev-parse --git-dir`.
//! - Short ref names are always strict, as if `core.warnAmbiguousRefs` were true: with a tag and a
//!   branch both named `x` and the setting false, Git may print `x` where Brit prints `tags/x`.
//! - Ref names that are not UTF-8 are shortened, matched and printed lossily.
//! - `ls-remote` needs a local repository even for an explicit path or URL.
//! - Brit's global `-r <dir>` starts discovery at `<dir>` and may find an ancestor; Git's
//!   `--git-dir=<dir>` requires `<dir>` itself to be a Git directory. `GIT_DIR` overrides `-r`.
//! - `ls-remote` may wait on a configured credential helper; see its module for how a caller avoids it.

pub mod check_ref_format;
pub mod for_each_ref;
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
pub mod ls_remote;
pub mod rev_parse;

/// A Git-shaped ending: its diagnostic, if any, was already written, and the process should exit with
/// this status instead of reporting an error. Returned as the error of a spelling that answers
/// through its exit code, so that buffered output is still forwarded before the process ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exit(pub i32);

impl std::fmt::Display for Exit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "exit status {}", self.0)
    }
}

impl std::error::Error for Exit {}

/// End like Git with `code`.
pub fn exit(code: i32) -> gix::Error {
    gix::Error::from_error(Exit(code))
}

/// The status a Git-shaped ending in `error` asks for, if it is one.
pub fn exit_status(error: &(dyn std::error::Error + 'static)) -> Option<i32> {
    error.downcast_ref::<Exit>().map(|exit| exit.0).or_else(|| {
        error
            .downcast_ref::<gix::Error>()
            .and_then(|error| error.downcast_any_ref::<Exit>())
            .map(|exit| exit.0)
    })
}

/// Git's `shorten_unambiguous_ref()` in its default strict mode (`core.warnAmbiguousRefs`): the
/// shortest name that no other lookup rule resolves, where `exists` answers whether a full ref name exists.
pub(crate) fn shorten_unambiguous(full_name: &str, exists: impl Fn(&str) -> bool) -> String {
    const RULES: [(&str, &str); 6] = [
        ("", ""),
        ("refs/", ""),
        ("refs/tags/", ""),
        ("refs/heads/", ""),
        ("refs/remotes/", ""),
        ("refs/remotes/", "/HEAD"),
    ];
    for matched in (1..RULES.len()).rev() {
        let (prefix, suffix) = RULES[matched];
        let Some(short) = full_name
            .strip_prefix(prefix)
            .and_then(|rest| rest.strip_suffix(suffix))
            .filter(|short| !short.is_empty())
        else {
            continue;
        };
        let ambiguous = RULES
            .iter()
            .enumerate()
            .filter(|(rule, _)| *rule != matched)
            .any(|(_, (prefix, suffix))| exists(&format!("{prefix}{short}{suffix}")));
        if !ambiguous {
            return short.to_owned();
        }
    }
    full_name.to_owned()
}

#[cfg(test)]
mod tests {
    use super::shorten_unambiguous;

    #[test]
    fn shortening_follows_git_rules_and_ambiguity() {
        let none = |_: &str| false;
        assert_eq!(shorten_unambiguous("refs/heads/main", none), "main");
        assert_eq!(shorten_unambiguous("refs/tags/v1", none), "v1");
        assert_eq!(shorten_unambiguous("refs/remotes/origin/dev", none), "origin/dev");
        assert_eq!(shorten_unambiguous("refs/remotes/origin/HEAD", none), "origin");
        assert_eq!(
            shorten_unambiguous("refs/notes/brit/build/x", none),
            "notes/brit/build/x"
        );
        assert_eq!(shorten_unambiguous("HEAD", none), "HEAD");
        let both = |name: &str| name == "refs/tags/main" || name == "refs/heads/main";
        assert_eq!(shorten_unambiguous("refs/heads/main", both), "heads/main");
        assert_eq!(shorten_unambiguous("refs/tags/main", both), "tags/main");
    }
}
