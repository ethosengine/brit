//! Bounded native rebase interface. Deferred Git options are rejected by clap.
use gix::bstr::BString;

#[derive(Debug, clap::Args)]
#[command(group(clap::ArgGroup::new("author").multiple(false).args(["amend", "commit"])), group(clap::ArgGroup::new("action").required(true).multiple(false).args([
    "upstream", "status", "continue_", "skip", "abort", "edit_todo", "show_current_patch",
    "stage", "amend", "begin_split", "commit"
])))]
pub struct Platform {
    /// Explicit local branch, tag, remote-tracking ref or full commit OID.
    #[arg(value_parser = crate::shared::AsBString)]
    upstream: Option<BString>,
    /// Edit the complete candidate sequence before replay.
    #[arg(short = 'i', long, requires = "upstream", conflicts_with_all = ["status", "continue_", "skip", "abort", "edit_todo", "show_current_patch", "stage", "amend", "begin_split", "commit"])]
    interactive: bool,
    /// Inspect state without recovery or mutation.
    #[arg(long)]
    status: bool,
    /// Explicitly recover or continue the current operation.
    #[arg(long = "continue")]
    continue_: bool,
    #[arg(long)]
    skip: bool,
    #[arg(long)]
    abort: bool,
    #[arg(long)]
    edit_todo: bool,
    #[arg(long)]
    show_current_patch: bool,
    /// Stage explicitly named literal files or deletions.
    #[arg(long, requires = "paths")]
    stage: bool,
    /// Expand the active scope at an edit stop only.
    #[arg(long, requires = "stage")]
    expand_scope: bool,
    #[arg(long)]
    amend: bool,
    #[arg(long)]
    begin_split: bool,
    #[arg(long)]
    commit: bool,
    #[arg(short = 'm', long, value_parser = crate::shared::AsBString, conflicts_with_all = ["file", "no_edit"], requires = "author")]
    message: Option<BString>,
    #[arg(short = 'F', long, conflicts_with_all = ["message", "no_edit"], requires = "author")]
    file: Option<std::path::PathBuf>,
    #[arg(long, requires = "amend", conflicts_with_all = ["message", "file"])]
    no_edit: bool,
    /// Literal paths must follow --.
    #[arg(last = true, num_args = 1.., requires = "stage", value_parser = crate::shared::AsBString)]
    paths: Vec<BString>,
}

impl Platform {
    pub fn action(self) -> gitoxide_core::repository::replay::command::Action {
        use gitoxide_core::repository::replay::command::{Action, Message};
        if let Some(upstream) = self.upstream {
            return Action::Start {
                upstream,
                interactive: self.interactive,
            };
        }
        if self.status {
            return Action::Status;
        }
        if self.continue_ {
            return Action::Continue;
        }
        if self.skip {
            return Action::Skip;
        }
        if self.abort {
            return Action::Abort;
        }
        if self.edit_todo {
            return Action::EditTodo;
        }
        if self.show_current_patch {
            return Action::ShowCurrentPatch;
        }
        if self.stage {
            return Action::Stage {
                paths: self.paths,
                expand: self.expand_scope,
            };
        }
        if self.begin_split {
            return Action::BeginSplit;
        }
        let message = match (self.message, self.file, self.no_edit) {
            (Some(bytes), _, _) => Message::Bytes(bytes.to_vec()),
            (_, Some(path), _) => Message::File(path),
            (_, _, true) => Message::Unchanged,
            _ => Message::Editor,
        };
        if self.amend {
            Action::Amend(message)
        } else {
            Action::Commit(message)
        }
    }
}
