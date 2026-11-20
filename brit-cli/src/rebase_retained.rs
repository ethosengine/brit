fn main() {
    gix::utils::stream::Budget::share_process_quota();
    if let Err(error) = gitoxide::rebase_retained::run(std::env::args_os().skip(1)) {
        brit_epr::diagnostic::emit(brit_epr::diagnostic::report(error.as_ref(), Some(error.backtrace())));
        std::process::exit(1);
    }
}
