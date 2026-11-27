//! The published feedback vocabulary. Keep superseded declarations in this table.

use gitoxide::plumbing::FeedbackCode;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy)]
pub(crate) struct Claim(&'static Declaration);

impl Claim {
    pub fn id(self) -> &'static str {
        self.0.id
    }

    pub fn is_command(self) -> bool {
        self.id().starts_with("brit/command:")
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Declaration {
    pub id: &'static str,
    asserts: &'static str,
    contradicted_by: &'static str,
    validity_horizon: &'static str,
    supersedes: &'static [&'static str],
    clears: &'static [&'static str],
    polarity: &'static str,
    severity: &'static str,
    weight: u8,
    ceiling: &'static str,
    retire_when: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cure: Option<&'static str>,
}

const DEFAULT: Declaration = Declaration {
    id: "",
    asserts: "",
    contradicted_by: "",
    validity_horizon: "P14D",
    supersedes: &[],
    clears: &[],
    polarity: "negative",
    severity: "warn",
    weight: 1,
    ceiling: "device",
    retire_when: None,
    cure: None,
};
const RELEASE_CURRENT: &[&str] = &["release-current:brit"];

macro_rules! command {
    ($param:literal) => {
        Declaration {
            id: concat!("brit/command:", $param),
            asserts: concat!("Brit handled a command recorded as ", $param, "."),
            contradicted_by: "The invocation was not handled as this command.",
            polarity: "positive",
            severity: "info",
            weight: 0,
            ..DEFAULT
        }
    };
}

macro_rules! gap {
    ($param:literal, $asserts:literal, $contradicted_by:literal, $clears:expr) => {
        Declaration {
            id: concat!("brit/capability-absent:", $param),
            asserts: $asserts,
            contradicted_by: $contradicted_by,
            clears: $clears,
            ..DEFAULT
        }
    };
}

macro_rules! refused {
    ($param:literal, $asserts:literal, $contradicted_by:literal) => {
        Declaration {
            id: concat!("brit/refused:", $param),
            asserts: $asserts,
            contradicted_by: $contradicted_by,
            severity: "info",
            weight: 0,
            ..DEFAULT
        }
    };
}

macro_rules! recorder {
    ($param:literal, $asserts:literal, $contradicted_by:literal, $cure:expr) => {
        Declaration {
            id: concat!("epr/recorder-unavailable:", $param),
            asserts: $asserts,
            contradicted_by: $contradicted_by,
            cure: $cure,
            ..DEFAULT
        }
    };
}

pub(crate) const DECLARATIONS: &[Declaration] = &[
    command!("status"),
    command!("log"),
    command!("build"),
    command!("snapshot"),
    command!("context"),
    command!("integrate"),
    command!("init"),
    command!("clone"),
    command!("fetch"),
    command!("push"),
    command!("pull"),
    command!("add"),
    command!("commit"),
    command!("diff"),
    command!("show"),
    command!("branch"),
    command!("switch"),
    command!("merge"),
    command!("rebase"),
    command!("restore"),
    command!("tag"),
    command!("remote"),
    command!("submodule"),
    command!("other"),
    command!("surface"),
    command!("unknown"),
    gap!(
        "reset",
        "Brit directed the requested reset operation to Git.",
        "Brit performs the requested reset operation natively.",
        RELEASE_CURRENT
    ),
    gap!(
        "rm",
        "Brit directed the requested rm operation to Git.",
        "Brit performs the requested rm operation natively.",
        RELEASE_CURRENT
    ),
    gap!(
        "mv",
        "Brit directed the requested mv operation to Git.",
        "Brit performs the requested mv operation natively.",
        RELEASE_CURRENT
    ),
    gap!(
        "stash",
        "Brit directed the requested stash operation to Git.",
        "Brit performs the requested stash operation natively.",
        RELEASE_CURRENT
    ),
    gap!(
        "revert",
        "Brit directed the requested revert operation to Git.",
        "Brit performs the requested revert operation natively.",
        RELEASE_CURRENT
    ),
    gap!(
        "checkout",
        "Brit directed the requested checkout operation to Git.",
        "Brit performs the requested checkout operation natively.",
        RELEASE_CURRENT
    ),
    gap!(
        "notes",
        "Brit directed the requested notes operation to Git.",
        "Brit performs the requested notes operation natively.",
        RELEASE_CURRENT
    ),
    gap!(
        "submodule.init",
        "Brit directed the requested submodule.init operation to Git.",
        "Brit performs the requested submodule.init operation natively.",
        RELEASE_CURRENT
    ),
    gap!(
        "submodule.sync",
        "Brit directed the requested submodule.sync operation to Git.",
        "Brit performs the requested submodule.sync operation natively.",
        RELEASE_CURRENT
    ),
    gap!(
        "submodule.update",
        "Brit directed the requested submodule.update operation to Git.",
        "Brit performs the requested submodule.update operation natively.",
        RELEASE_CURRENT
    ),
    gap!(
        "unknown",
        "Brit could not recognize the requested subcommand.",
        "Brit recognizes the requested subcommand.",
        RELEASE_CURRENT
    ),
    gap!(
        "cherry-pick.range",
        "Brit cannot select a cherry-pick range on this platform.",
        "Brit can select a cherry-pick range on this platform.",
        &[]
    ),
    gap!(
        "cherry-pick.list",
        "Brit cannot select a list of cherry-pick revisions on this platform.",
        "Brit can select a list of cherry-pick revisions on this platform.",
        &[]
    ),
    gap!(
        "cherry-pick.single",
        "Brit cannot cherry-pick one revision on this platform.",
        "Brit can cherry-pick one revision on this platform.",
        &[]
    ),
    gap!(
        "cherry-pick.continue",
        "Brit cannot continue a cherry-pick on this platform.",
        "Brit can continue a cherry-pick on this platform.",
        &[]
    ),
    gap!(
        "cherry-pick.abort",
        "Brit cannot abort a cherry-pick on this platform.",
        "Brit can abort a cherry-pick on this platform.",
        &[]
    ),
    gap!(
        "cherry-pick.skip",
        "Brit cannot skip a cherry-pick step on this platform.",
        "Brit can skip a cherry-pick step on this platform.",
        &[]
    ),
    gap!(
        "cherry-pick.status",
        "Brit cannot report cherry-pick status on this platform.",
        "Brit can report cherry-pick status on this platform.",
        &[]
    ),
    gap!(
        "push.force-if-includes",
        "Brit cannot enforce force-if-includes for push.",
        "Brit can enforce force-if-includes for push.",
        RELEASE_CURRENT
    ),
    gap!(
        "commit.signing",
        "Brit cannot create a commit when signing is required.",
        "Brit can create a commit when signing is required.",
        RELEASE_CURRENT
    ),
    gap!(
        "commit.paths",
        "Brit cannot commit only the requested paths.",
        "Brit can commit only the requested paths.",
        RELEASE_CURRENT
    ),
    gap!(
        "commit.state",
        "Brit cannot commit during this repository operation.",
        "Brit can commit during this repository operation.",
        RELEASE_CURRENT
    ),
    gap!(
        "commit.cleanup",
        "Brit cannot clean a commit message with an automatic or empty comment prefix.",
        "Brit can clean a commit message with an automatic or empty comment prefix.",
        RELEASE_CURRENT
    ),
    gap!(
        "commit.sparse-index",
        "Brit cannot commit from a sparse index.",
        "Brit can commit from a sparse index.",
        RELEASE_CURRENT
    ),
    gap!(
        "tag.cleanup",
        "Brit cannot clean a tag message with an automatic or empty comment prefix.",
        "Brit can clean a tag message with an automatic or empty comment prefix.",
        RELEASE_CURRENT
    ),
    gap!(
        "tag.empty-message",
        "Brit cannot create an annotated tag with an empty message.",
        "Brit can create an annotated tag with an empty message.",
        RELEASE_CURRENT
    ),
    gap!(
        "show.output-encoding",
        "Brit cannot show the object in the configured output encoding.",
        "Brit can show the object in the configured output encoding.",
        RELEASE_CURRENT
    ),
    gap!(
        "show.commit-encoding",
        "Brit cannot decode the commit in its declared encoding.",
        "Brit can decode the commit in its declared encoding.",
        RELEASE_CURRENT
    ),
    gap!(
        "show.combined-diff",
        "Brit cannot show the requested combined diff.",
        "Brit can show the requested combined diff.",
        RELEASE_CURRENT
    ),
    gap!(
        "show.range",
        "Brit cannot show the requested revision range.",
        "Brit can show the requested revision range.",
        RELEASE_CURRENT
    ),
    gap!(
        "diff.combined-diff",
        "Brit cannot compare more than two revisions.",
        "Brit can compare more than two revisions.",
        RELEASE_CURRENT
    ),
    refused!(
        "push.deletion",
        "The selected push policy disallows deletion.",
        "The selected push policy authorizes deletion."
    ),
    refused!(
        "push.force",
        "The selected push policy disallows force options.",
        "The selected push policy authorizes force options."
    ),
    refused!(
        "push.unverified-fast-forward",
        "Brit refused a push whose fast-forward ancestry it could not verify.",
        "Brit verified fast-forward ancestry for the same push."
    ),
    refused!(
        "merge.detached-head",
        "Native fast-forward merge requires a checked-out branch as its source.",
        "Native fast-forward merge permits the same detached source."
    ),
    refused!(
        "checkout.non-local-head",
        "Native checkout requires an attached HEAD to name a local branch.",
        "Native checkout permits the same non-local HEAD attachment."
    ),
    refused!(
        "switch.non-branch-target",
        "Native switch requires an existing local branch as its target.",
        "Native switch permits the same non-branch target."
    ),
    recorder!(
        "not-found",
        "Brit could not find the EPR recorder.",
        "Brit can find the EPR recorder.",
        Some("install epr")
    ),
    recorder!(
        "timeout",
        "The EPR recorder missed its capture deadline.",
        "The EPR recorder completes capture within the deadline.",
        None
    ),
    recorder!(
        "refused",
        "The EPR recorder exited unsuccessfully during capture.",
        "The EPR recorder completes capture successfully.",
        None
    ),
    recorder!(
        "spawn-failed",
        "Brit could not start or supervise the EPR recorder.",
        "Brit starts and supervises the EPR recorder successfully.",
        None
    ),
    gap!(
        "diff.pathspec",
        "Brit cannot match the requested non-literal pathspec.",
        "Brit can match the requested non-literal pathspec.",
        RELEASE_CURRENT
    ),
    gap!(
        "diff.submodule-format",
        "Brit cannot diff submodules in the configured format.",
        "Brit can diff submodules in the configured format.",
        RELEASE_CURRENT
    ),
    gap!(
        "diff.submodule-ignore",
        "Brit cannot apply the configured rules for ignoring submodule changes.",
        "Brit can apply the configured rules for ignoring submodule changes.",
        RELEASE_CURRENT
    ),
    gap!(
        "diff.attributes",
        "Brit cannot apply the requested diff attributes or text conversion.",
        "Brit can apply the requested diff attributes or text conversion.",
        RELEASE_CURRENT
    ),
    gap!(
        "diff.external",
        "Brit cannot run the configured external diff command.",
        "Brit can run the configured external diff command.",
        RELEASE_CURRENT
    ),
];

#[derive(Serialize)]
pub(super) struct Cure {
    pub(super) claim: &'static str,
    pub(super) cite: &'static str,
    pub(super) summary: &'static str,
}

pub(super) const CURES: &[Cure] = &[];

#[derive(Serialize)]
pub(super) struct Manifest {
    claims: &'static [Declaration],
    cures: &'static [Cure],
}

impl Manifest {
    pub(super) const COMPILED: Self = Self {
        claims: DECLARATIONS,
        cures: CURES,
    };
}

const fn same_bytes(lhs: &[u8], rhs: &[u8]) -> bool {
    if lhs.len() != rhs.len() {
        return false;
    }
    let mut index = 0;
    while index < lhs.len() {
        if lhs[index] != rhs[index] {
            return false;
        }
        index += 1;
    }
    true
}

const fn declared(id: &str) -> Claim {
    let mut index = 0;
    while index < DECLARATIONS.len() {
        if same_bytes(DECLARATIONS[index].id.as_bytes(), id.as_bytes()) {
            return Claim(&DECLARATIONS[index]);
        }
        index += 1;
    }
    panic!("feedback claim must be declared")
}

pub(crate) const OTHER: Claim = declared("brit/command:other");
pub(crate) const UNKNOWN_COMMAND: Claim = declared("brit/command:unknown");
pub(crate) const UNKNOWN_CAPABILITY: Claim = declared("brit/capability-absent:unknown");

pub(crate) fn command(name: &str) -> Option<Claim> {
    lookup("brit/command:", name)
}

pub(crate) fn capability(name: &str) -> Option<Claim> {
    lookup("brit/capability-absent:", name)
}

fn lookup(prefix: &str, param: &str) -> Option<Claim> {
    DECLARATIONS
        .iter()
        .find(|row| row.id.strip_prefix(prefix) == Some(param))
        .map(Claim)
}

/// Every typed code [`form_claim`] can return. A cure is observable only for one of these;
/// extend this list together with `form_claim` when a new form becomes observable.
#[cfg(test)]
const FORM_CODES: [FeedbackCode; 7] = [
    FeedbackCode::CherryPickContinue,
    FeedbackCode::CherryPickAbort,
    FeedbackCode::CherryPickSkip,
    FeedbackCode::CherryPickStatus,
    FeedbackCode::CherryPickRange,
    FeedbackCode::CherryPickSingle,
    FeedbackCode::CherryPickList,
];

pub(super) fn form_claim(matches: &clap::ArgMatches) -> Option<Claim> {
    use gix::bstr::{BString, ByteSlice};

    let pick = matches.subcommand_matches("cherry-pick")?;
    // Match the same forms as the typed cherry-pick refusals.
    for (flag, code) in [
        ("continue_", FeedbackCode::CherryPickContinue),
        ("abort", FeedbackCode::CherryPickAbort),
        ("skip", FeedbackCode::CherryPickSkip),
        ("status", FeedbackCode::CherryPickStatus),
    ] {
        if pick.get_flag(flag) {
            return Some(refusal(code));
        }
    }
    let mut revisions = pick.get_many::<BString>("revisions")?;
    let code = match revisions.len() {
        0 => return None,
        1 if revisions.next()?.contains_str("..") => FeedbackCode::CherryPickRange,
        1 => FeedbackCode::CherryPickSingle,
        _ => FeedbackCode::CherryPickList,
    };
    Some(refusal(code))
}

pub(crate) fn refusal(code: FeedbackCode) -> Claim {
    match code {
        FeedbackCode::CherryPickRange => const { declared("brit/capability-absent:cherry-pick.range") },
        FeedbackCode::CherryPickList => const { declared("brit/capability-absent:cherry-pick.list") },
        FeedbackCode::CherryPickSingle => const { declared("brit/capability-absent:cherry-pick.single") },
        FeedbackCode::CherryPickContinue => const { declared("brit/capability-absent:cherry-pick.continue") },
        FeedbackCode::CherryPickAbort => const { declared("brit/capability-absent:cherry-pick.abort") },
        FeedbackCode::CherryPickSkip => const { declared("brit/capability-absent:cherry-pick.skip") },
        FeedbackCode::CherryPickStatus => const { declared("brit/capability-absent:cherry-pick.status") },
        FeedbackCode::PushForceIfIncludes => const { declared("brit/capability-absent:push.force-if-includes") },
        FeedbackCode::CommitSigning => const { declared("brit/capability-absent:commit.signing") },
        FeedbackCode::CommitPaths => const { declared("brit/capability-absent:commit.paths") },
        FeedbackCode::CommitState => const { declared("brit/capability-absent:commit.state") },
        FeedbackCode::CommitCleanup => const { declared("brit/capability-absent:commit.cleanup") },
        FeedbackCode::CommitSparseIndex => const { declared("brit/capability-absent:commit.sparse-index") },
        FeedbackCode::TagCleanup => const { declared("brit/capability-absent:tag.cleanup") },
        FeedbackCode::TagEmptyMessage => const { declared("brit/capability-absent:tag.empty-message") },
        FeedbackCode::ShowOutputEncoding => const { declared("brit/capability-absent:show.output-encoding") },
        FeedbackCode::ShowCommitEncoding => const { declared("brit/capability-absent:show.commit-encoding") },
        FeedbackCode::ShowCombinedDiff => const { declared("brit/capability-absent:show.combined-diff") },
        FeedbackCode::ShowRange => const { declared("brit/capability-absent:show.range") },
        FeedbackCode::DiffCombined => const { declared("brit/capability-absent:diff.combined-diff") },
        FeedbackCode::DiffPathspec => const { declared("brit/capability-absent:diff.pathspec") },
        FeedbackCode::DiffSubmoduleFormat => const { declared("brit/capability-absent:diff.submodule-format") },
        FeedbackCode::DiffSubmoduleIgnore => const { declared("brit/capability-absent:diff.submodule-ignore") },
        FeedbackCode::DiffAttributes => const { declared("brit/capability-absent:diff.attributes") },
        FeedbackCode::DiffExternal => const { declared("brit/capability-absent:diff.external") },
        FeedbackCode::PushDeletion => const { declared("brit/refused:push.deletion") },
        FeedbackCode::PushForce => const { declared("brit/refused:push.force") },
        FeedbackCode::PushUnverifiedFastForward => const { declared("brit/refused:push.unverified-fast-forward") },
        FeedbackCode::MergeDetachedHead => const { declared("brit/refused:merge.detached-head") },
        FeedbackCode::CheckoutNonLocalHead => const { declared("brit/refused:checkout.non-local-head") },
        FeedbackCode::SwitchNonBranchTarget => const { declared("brit/refused:switch.non-branch-target") },
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum RecorderCause {
    NotFound,
    Timeout,
    Refused,
    SpawnFailed,
}

impl RecorderCause {
    pub fn claim(self) -> Claim {
        match self {
            Self::NotFound => const { declared("epr/recorder-unavailable:not-found") },
            Self::Timeout => const { declared("epr/recorder-unavailable:timeout") },
            Self::Refused => const { declared("epr/recorder-unavailable:refused") },
            Self::SpawnFailed => const { declared("epr/recorder-unavailable:spawn-failed") },
        }
    }

    pub fn as_str(self) -> &'static str {
        self.claim()
            .id()
            .strip_prefix("epr/recorder-unavailable:")
            .expect("recorder claim prefix")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn slug(value: &str, dotted: bool) -> bool {
        !value.is_empty()
            && value.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || dotted && byte == b'.'
            })
    }

    #[test]
    fn claims_have_unique_ids_and_valid_declarations() {
        assert!(!DECLARATIONS.is_empty(), "Brit declares its compiled claims");
        let mut ids = HashSet::new();
        for row in DECLARATIONS {
            let (producer, tail) = row.id.split_once('/').expect("producer/name");
            let (name, param) = tail
                .split_once(':')
                .map_or((tail, None), |(name, param)| (name, Some(param)));
            assert!(slug(producer, false) && slug(name, false) && param.is_none_or(|param| slug(param, true)));
            assert!(ids.insert(row.id), "{} appears once", row.id);
            assert!(matches!(row.polarity, "negative" | "positive"));
            assert!(matches!(
                row.severity,
                "trace" | "debug" | "info" | "warn" | "error" | "fatal"
            ));
            assert!(matches!(row.ceiling, "device" | "steward"));
            assert!(
                iso_day_duration(row.validity_horizon),
                "{} has an ISO 8601 day duration",
                row.id
            );
            assert!(
                row.retire_when.is_none(),
                "{} has no retirement condition in this release",
                row.id
            );
            if row.id.starts_with("brit/command:") {
                assert_eq!(
                    (row.polarity, row.severity, row.weight, row.ceiling),
                    ("positive", "info", 0, "device")
                );
            } else if row.id.starts_with("brit/refused:") {
                assert_eq!(
                    (row.polarity, row.severity, row.weight, row.ceiling),
                    ("negative", "info", 0, "device")
                );
                assert!(row.clears.is_empty(), "{} declares an intentional refusal", row.id);
            } else {
                assert_eq!((row.polarity, row.severity), ("negative", "warn"));
            }
            assert!(row.asserts.ends_with('.') && row.contradicted_by.ends_with('.'));
            assert!(row.cure.is_none_or(|cure| !cure.trim().is_empty()));
            for clear in row.clears {
                let has_pair = |value: &str| {
                    value
                        .split_once('@')
                        .is_some_and(|(name, version)| !name.is_empty() && !version.is_empty())
                };
                assert!(
                    match clear.split_once(':') {
                        None => *clear == "observed",
                        Some(("release-current" | "path", value)) => !value.is_empty(),
                        Some(("release-at-least" | "bound", value)) => has_pair(value),
                        _ => false,
                    },
                    "unknown clearing condition {clear}"
                );
            }
            for previous in row.supersedes {
                assert_ne!(row.id, *previous);
                assert!(DECLARATIONS.iter().any(|candidate| candidate.id == *previous));
            }
        }
    }

    #[test]
    fn cures_reference_declared_claims() {
        for cure in CURES {
            assert!(
                DECLARATIONS.iter().any(|row| row.id == cure.claim),
                "undeclared cure claim {}",
                cure.claim
            );
            assert!(!cure.cite.trim().is_empty(), "{} requires a citation", cure.claim);
            assert!(!cure.summary.trim().is_empty(), "{} requires a summary", cure.claim);
            assert!(
                FORM_CODES.iter().any(|code| refusal(*code).id() == cure.claim),
                "{} has no successful form that `form_claim` maps to it, so its passage would never be observed",
                cure.claim
            );
        }
    }

    /// `P<n>D` with a positive day count: the only duration form this table declares.
    fn iso_day_duration(value: &str) -> bool {
        value
            .strip_prefix('P')
            .and_then(|rest| rest.strip_suffix('D'))
            .is_some_and(|days| !days.is_empty() && days.bytes().all(|b| b.is_ascii_digit()) && days != "0")
    }

    #[test]
    fn claims_subcommand_exports_the_compiled_table() -> gix_testtools::TestResult {
        let mut out = Vec::new();
        super::super::write_claims(&mut out, true)?;
        let expected = serde_json::json!({ "claims": DECLARATIONS, "cures": CURES });
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&out)?,
            expected,
            "JSON contains the complete claim and cure declarations"
        );
        Ok(())
    }

    #[test]
    fn claim_ids_are_append_only() {
        // Append new IDs only. Keep superseded IDs here and in the declaration table.
        let published = [
            "brit/command:status",
            "brit/command:log",
            "brit/command:build",
            "brit/command:snapshot",
            "brit/command:context",
            "brit/command:integrate",
            "brit/command:init",
            "brit/command:clone",
            "brit/command:fetch",
            "brit/command:push",
            "brit/command:pull",
            "brit/command:add",
            "brit/command:commit",
            "brit/command:diff",
            "brit/command:show",
            "brit/command:branch",
            "brit/command:switch",
            "brit/command:merge",
            "brit/command:rebase",
            "brit/command:restore",
            "brit/command:tag",
            "brit/command:remote",
            "brit/command:submodule",
            "brit/command:other",
            "brit/command:surface",
            "brit/command:unknown",
            "brit/capability-absent:reset",
            "brit/capability-absent:rm",
            "brit/capability-absent:mv",
            "brit/capability-absent:stash",
            "brit/capability-absent:revert",
            "brit/capability-absent:checkout",
            "brit/capability-absent:notes",
            "brit/capability-absent:submodule.init",
            "brit/capability-absent:submodule.sync",
            "brit/capability-absent:submodule.update",
            "brit/capability-absent:unknown",
            "brit/capability-absent:cherry-pick.range",
            "brit/capability-absent:cherry-pick.list",
            "brit/capability-absent:cherry-pick.single",
            "brit/capability-absent:cherry-pick.continue",
            "brit/capability-absent:cherry-pick.abort",
            "brit/capability-absent:cherry-pick.skip",
            "brit/capability-absent:cherry-pick.status",
            "brit/capability-absent:push.force-if-includes",
            "brit/capability-absent:commit.signing",
            "brit/capability-absent:commit.paths",
            "brit/capability-absent:commit.state",
            "brit/capability-absent:commit.cleanup",
            "brit/capability-absent:commit.sparse-index",
            "brit/capability-absent:tag.cleanup",
            "brit/capability-absent:tag.empty-message",
            "brit/capability-absent:show.output-encoding",
            "brit/capability-absent:show.commit-encoding",
            "brit/capability-absent:show.combined-diff",
            "brit/capability-absent:show.range",
            "brit/capability-absent:diff.combined-diff",
            "brit/refused:push.deletion",
            "brit/refused:push.force",
            "brit/refused:push.unverified-fast-forward",
            "brit/refused:merge.detached-head",
            "brit/refused:checkout.non-local-head",
            "brit/refused:switch.non-branch-target",
            "epr/recorder-unavailable:not-found",
            "epr/recorder-unavailable:timeout",
            "epr/recorder-unavailable:refused",
            "epr/recorder-unavailable:spawn-failed",
            "brit/capability-absent:diff.pathspec",
            "brit/capability-absent:diff.submodule-format",
            "brit/capability-absent:diff.submodule-ignore",
            "brit/capability-absent:diff.attributes",
            "brit/capability-absent:diff.external",
        ];
        let actual: Vec<_> = DECLARATIONS.iter().map(|row| row.id).collect();
        assert_eq!(
            actual, published,
            "append new declarations to the snapshot in table order"
        );
    }
}
