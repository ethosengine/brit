fn main() {
    if let Err(error) = gitoxide::rebase_retained::run(std::env::args_os().skip(1)) {
        eprintln!("brit-rebase-retained: {error:#}");
        std::process::exit(1);
    }
}
