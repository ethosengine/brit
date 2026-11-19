//! Git's spellings of ref resolution and listing, with Git's output shapes for the forms agents use.
//!
//! Each module answers one Git command name. Brit's own verbs (`revision resolve`, `remote refs`,
//! `branch list`) stay the native interface; these exist so that a guessed Git spelling works and
//! points back to them.

pub mod for_each_ref;
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
pub mod ls_remote;
pub mod rev_parse;

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
