mod guard_label;
mod main;
mod mutation;
/// The status a Git-shaped ending asks for, when a command answers through its exit code.
pub use gitoxide_core::repository::git_spellings::{Exit as GitExit, exit_status};
pub use gitoxide_core::repository::switch::MutationWait;
pub use main::change_dirs;
pub use main::configure_mutation_guards;
pub use main::main;
pub use main::run;
pub use main::run_with_command;
pub use mutation::MutationGuards;

/// Return Git's normalized process arguments for the composed public CLI.
pub fn args_os() -> impl Iterator<Item = std::ffi::OsString> {
    gix::env::args_os()
}

#[path = "progress.rs"]
mod progress_impl;
pub use progress_impl::show_progress;

mod options;
pub use options::Args;
