use std::process::ExitCode;

fn main() -> ExitCode {
    brit_epr::diagnostic::install_panic_hook("brit-rebase-retained", env!("CARGO_PKG_VERSION"));
    gix::utils::stream::Budget::share_process_quota();
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    // Only `extract` writes anything but its report to disk.
    let extracting = arguments.first().is_some_and(|action| action == "extract");
    match gitoxide::rebase_retained::run(arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // The reader closed stdout early: end quietly, never with a panic or a report.
            if let Some(code) = brit_epr::diagnostic::exit_for(error.as_ref(), !extracting) {
                return code;
            }
            brit_epr::diagnostic::emit(brit_epr::diagnostic::report(error.as_ref(), Some(error.backtrace())));
            ExitCode::FAILURE
        }
    }
}
