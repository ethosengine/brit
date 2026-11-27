//! Typed refusals that the Brit CLI can report as feedback claims.

/// Why Brit declined an operation.
#[derive(Clone, Copy, Debug)]
pub enum FeedbackCode {
    CherryPickRange,
    CherryPickList,
    CherryPickSingle,
    CherryPickContinue,
    CherryPickAbort,
    CherryPickSkip,
    CherryPickStatus,
    PushForceIfIncludes,
    CommitSigning,
    CommitPaths,
    CommitState,
    CommitCleanup,
    CommitSparseIndex,
    TagCleanup,
    TagEmptyMessage,
    ShowOutputEncoding,
    ShowCommitEncoding,
    ShowCombinedDiff,
    ShowRange,
    DiffCombined,
    DiffPathspec,
    DiffSubmoduleFormat,
    DiffSubmoduleIgnore,
    DiffAttributes,
    DiffExternal,
    PushDeletion,
    PushForce,
    PushUnverifiedFastForward,
    MergeDetachedHead,
    CheckoutNonLocalHead,
    SwitchNonBranchTarget,
}

/// A classified refusal with the existing human-facing diagnostic.
#[derive(Debug)]
pub struct FeedbackRefusal {
    pub code: FeedbackCode,
    message: String,
}

impl std::fmt::Display for FeedbackRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for FeedbackRefusal {}

impl FeedbackCode {
    pub(crate) fn unsupported(self, message: impl Into<String>) -> gix::error::ClassificationMarker {
        gix::error::tag(self.because(message), gix::error::Class::Unsupported)
    }

    pub(crate) fn because(self, message: impl Into<String>) -> FeedbackRefusal {
        FeedbackRefusal {
            code: self,
            message: message.into(),
        }
    }
}
