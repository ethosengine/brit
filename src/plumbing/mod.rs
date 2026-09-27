mod main;
pub use main::main;
pub use main::run;
pub use main::run_with_command;

/// Return Git's normalized process arguments for the composed public CLI.
pub fn args_os() -> impl Iterator<Item = std::ffi::OsString> {
    gix::env::args_os()
}

#[path = "progress.rs"]
mod progress_impl;
pub use progress_impl::show_progress;

mod options;
pub use options::Args;
