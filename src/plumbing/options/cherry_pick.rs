use gix::bstr::BString;

#[derive(Debug, clap::Args)]
#[command(group(clap::ArgGroup::new("action").required(true).args([
    "revisions", "continue_", "abort", "skip", "status"
])))]
pub struct Platform {
    /// Commits in application order, or A..B ranges expanded oldest first.
    #[arg(value_name = "COMMIT|RANGE", value_parser = crate::shared::AsBString)]
    revisions: Vec<BString>,
    /// Commit a staged resolution and apply the remaining commits.
    #[arg(long = "continue")]
    continue_: bool,
    /// Restore the branch, index and worktree from before the sequence.
    #[arg(long)]
    abort: bool,
    /// Discard the current input and apply the remaining commits.
    #[arg(long)]
    skip: bool,
    /// Inspect the active cherry-pick sequence.
    #[arg(long)]
    status: bool,
}

impl Platform {
    pub fn action(self) -> gitoxide_core::repository::replay::command::Action {
        use gitoxide_core::repository::replay::command::Action;
        if self.continue_ {
            Action::Continue
        } else if self.abort {
            Action::Abort
        } else if self.skip {
            Action::Skip
        } else if self.status {
            Action::Status
        } else {
            Action::StartPick(self.revisions)
        }
    }
}
