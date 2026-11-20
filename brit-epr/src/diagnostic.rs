//! How every brit binary prints a failure.
//!
//! A failed command ends its stderr with the error itself: `Error: <message>`, then its
//! causes in anyhow's `Caused by:` layout. `RUST_BACKTRACE` and `RUST_LIB_BACKTRACE` may make
//! a library capture a backtrace, but they never make brit print one: callers that report the
//! last stderr line must see the cause, not a stack frame. A backtrace is printed only when
//! [`BACKTRACE_ENV`] asks for one, and then before the error, so the error stays last.

use std::{
    backtrace::{Backtrace, BacktraceStatus},
    error::Error,
    fmt::{Display, Write},
};

/// Set to anything but empty or `0` to print a captured backtrace with an error.
///
/// Capture itself is still governed by Rust's `RUST_BACKTRACE`/`RUST_LIB_BACKTRACE`.
pub const BACKTRACE_ENV: &str = "BRIT_BACKTRACE";

/// `Error: <error>` and its source chain, without any backtrace.
pub fn render(error: &(dyn Error + 'static)) -> String {
    render_causes(error.to_string(), error.source())
}

/// `Error: <context>` with `error` and its source chain as the causes.
pub fn render_context(context: impl Display, error: &(dyn Error + 'static)) -> String {
    render_causes(context.to_string(), Some(error))
}

/// [`render`], preceded by `backtrace` when [`BACKTRACE_ENV`] asks for it and one was captured.
pub fn report(error: &(dyn Error + 'static), backtrace: Option<&Backtrace>) -> String {
    let rendered = render(error);
    if !backtrace_requested() {
        return rendered;
    }
    match backtrace.filter(|b| b.status() == BacktraceStatus::Captured) {
        Some(backtrace) => format!("Stack backtrace:\n{backtrace}\n{rendered}"),
        None => format!(
            "note: no backtrace was captured; set RUST_LIB_BACKTRACE=1 together with {BACKTRACE_ENV}\n{rendered}"
        ),
    }
}

/// Write `message` and a newline to stderr, ignoring failure.
///
/// `eprintln!` panics when the write fails, as it does once stderr's reader has closed (Rust ignores `SIGPIPE`, so the
/// write returns `EPIPE`): the process then exits 101 instead of its own code, and a panic inside `Drop` while
/// unwinding aborts. Reporting must never be the reason a brit binary panics.
pub fn emit(message: impl Display) {
    use std::io::Write as _;
    let _ = writeln!(std::io::stderr().lock(), "{message}");
}

/// Exit status of a read-only command whose reader closed stdout early: 141, Git's status when `SIGPIPE` kills it.
pub const CLOSED_READER_EXIT: u8 = 141;

/// `println!` that returns its I/O error instead of panicking when stdout's reader has closed.
///
/// Rust ignores `SIGPIPE`, so a write to a closed pipe returns `EPIPE`, which `println!` turns into a panic. Use as
/// `outln!(..)?;` in a command that returns `anyhow::Result` or `io::Result`, and let the binary's `main` end quietly
/// through [`exit_for`].
#[macro_export]
macro_rules! outln {
    ($($arg:tt)*) => {
        $crate::diagnostic::stdout_line(format_args!($($arg)*))
    };
}

/// Write `line` and a newline to stdout and flush, so a closed reader is seen here and not at process exit.
pub fn stdout_line(line: std::fmt::Arguments<'_>) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut out = std::io::stdout().lock();
    writeln!(out, "{line}")?;
    out.flush()
}

/// Whether `error` or any cause is an [`std::io::ErrorKind::BrokenPipe`] I/O error (matched by type, never by text).
pub fn is_broken_pipe(error: &(dyn Error + 'static)) -> bool {
    let mut next = Some(error);
    while let Some(cause) = next {
        if cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|e| e.kind() == std::io::ErrorKind::BrokenPipe)
        {
            return true;
        }
        next = cause.source();
    }
    false
}

/// The status a binary ends with for a failed run, or `None` when the failure should be reported as usual.
///
/// A closed reader is not a failure of the command: it exits quietly, with [`CLOSED_READER_EXIT`] when it was
/// read-only (as Git killed by `SIGPIPE`), and `0` when it had already done its work and only the report was lost.
pub fn exit_for(error: &(dyn Error + 'static), read_only: bool) -> Option<std::process::ExitCode> {
    is_broken_pipe(error).then(|| std::process::ExitCode::from(if read_only { CLOSED_READER_EXIT } else { 0 }))
}

/// Replace the default panic report with a short, honest one naming `binary`, `version` and the panic location.
///
/// The hook only changes what is printed (once, ignoring a closed stderr): the panic still unwinds and the process
/// still ends with Rust's panic status, 101. Set `RUST_BACKTRACE` to see the frames.
pub fn install_panic_hook(binary: &'static str, version: &'static str) {
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map_or_else(|| "an unknown location".to_string(), ToString::to_string);
        let payload = info.payload();
        let message = payload
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("(no message)");
        emit(format_args!(
            "{binary} {version}: internal error: panicked at {location}: {message}\nThis is a brit bug; please report it with the command you ran. Set RUST_BACKTRACE=1 for frames."
        ));
        if std::env::var_os("RUST_BACKTRACE").is_some_and(|v| !v.is_empty() && v != "0") {
            emit(std::backtrace::Backtrace::force_capture());
        }
    }));
}

/// Whether [`BACKTRACE_ENV`] asks for backtraces.
pub fn backtrace_requested() -> bool {
    std::env::var_os(BACKTRACE_ENV).is_some_and(|value| !value.is_empty() && value != "0")
}

/// Render `message` and its causes, one line each, with the deepest cause that carries a message last.
///
/// gix errors classify themselves with [`gix_error::ClassificationMarker`]s in their source chain. A marker has no
/// diagnostic of its own: one without a source displays only its class (such as `NotFound`), and one with a source
/// displays that source, which follows it in the chain. Markers are recognised by type, never by text, and folded into
/// the message they classify as a `(Class)` suffix, so the class stays visible without becoming the last line: a
/// source-less marker classifies the error above it, a marker with a source classifies the error below it.
fn render_causes(message: String, first_cause: Option<&(dyn Error + 'static)>) -> String {
    let mut lines: Vec<(String, Vec<String>)> = vec![(message, Vec::new())];
    let mut classes_for_next = Vec::new();
    let mut next = first_cause;
    while let Some(cause) = next {
        next = cause.source();
        if let Some(marker) = cause.downcast_ref::<gix_error::ClassificationMarker>() {
            let class = format!("{:?}", marker.class());
            if marker.source().is_some() {
                classes_for_next.push(class);
            } else {
                lines.last_mut().expect("the message is the first line").1.push(class);
            }
            continue;
        }
        lines.push((cause.to_string(), std::mem::take(&mut classes_for_next)));
    }
    lines
        .last_mut()
        .expect("the message is the first line")
        .1
        .extend(classes_for_next);
    let mut lines = lines.into_iter().map(|(text, classes)| {
        if classes.is_empty() {
            text
        } else {
            format!("{text} ({})", classes.join(", "))
        }
    });

    let mut output = format!("Error: {}", lines.next().expect("the message is the first line"));
    let causes: Vec<String> = lines.collect();
    if causes.is_empty() {
        return output;
    }
    output.push_str("\n\nCaused by:");
    let multiple = causes.len() > 1;
    let indent = if multiple { "       " } else { "    " };
    for (index, cause) in causes.iter().enumerate() {
        output.push('\n');
        if multiple {
            write!(output, "{index:>5}: ").expect("writing to a String cannot fail");
        } else {
            output.push_str(indent);
        }
        output.push_str(&cause.replace('\n', &format!("\n{indent}")));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Layer(&'static str, Option<Box<Layer>>);

    impl Display for Layer {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(self.0)
        }
    }

    impl Error for Layer {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            self.1.as_deref().map(|e| e as &(dyn Error + 'static))
        }
    }

    #[test]
    fn a_lone_error_is_one_line() {
        assert_eq!(render(&Layer("failed", None)), "Error: failed");
    }

    #[test]
    fn one_cause_is_indented_without_numbers() {
        let error = Layer("operation failed", Some(Box::new(Layer("root cause", None))));
        assert_eq!(render(&error), "Error: operation failed\n\nCaused by:\n    root cause");
    }

    #[test]
    fn several_causes_are_numbered_and_multiline_causes_stay_aligned() {
        let error = Layer(
            "operation failed",
            Some(Box::new(Layer(
                "middle",
                Some(Box::new(Layer("root\nsecond line", None))),
            ))),
        );
        assert_eq!(
            render(&error),
            "Error: operation failed\n\nCaused by:\n    0: middle\n    1: root\n       second line"
        );
    }

    #[test]
    fn context_puts_the_error_first_among_the_causes() {
        assert_eq!(
            render_context("failed to open repo at x", &Layer("not a repository", None)),
            "Error: failed to open repo at x\n\nCaused by:\n    not a repository"
        );
    }

    /// An error whose source is the class-only marker gix uses for "not found".
    #[derive(Debug)]
    struct NotFoundLeaf(&'static str);

    impl Display for NotFoundLeaf {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(self.0)
        }
    }

    impl Error for NotFoundLeaf {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(const { &gix_error::ClassificationMarker::NOT_FOUND })
        }
    }

    #[derive(Debug)]
    struct Outer(&'static str, Box<dyn Error + Send + Sync>);

    impl Display for Outer {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(self.0)
        }
    }

    impl Error for Outer {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(&*self.1)
        }
    }

    #[test]
    fn a_class_only_marker_is_folded_into_the_message_it_classifies() {
        let error = Outer(
            "couldn't parse revision",
            Box::new(NotFoundLeaf("Reference \"x\" could not be found")),
        );
        assert_eq!(
            render(&error),
            "Error: couldn't parse revision\n\nCaused by:\n    Reference \"x\" could not be found (NotFound)",
            "the last line is the deepest message, not the bare class"
        );
        assert_eq!(
            render(&NotFoundLeaf("Could not find a git repository")),
            "Error: Could not find a git repository (NotFound)",
            "a marker directly below the message classifies the message"
        );
    }

    #[test]
    fn a_tagging_marker_is_not_printed_twice() {
        let tagged = gix_error::tag(Layer("bad input", None), gix_error::Class::Validation);
        let error = Outer("operation failed", Box::new(tagged));
        assert_eq!(
            render(&error),
            "Error: operation failed\n\nCaused by:\n    bad input (Validation)",
            "the marker displays its source; the source is printed once, with the class"
        );
    }

    #[test]
    fn a_captured_backtrace_is_not_printed_unless_brit_asks() {
        if backtrace_requested() {
            return;
        }
        let backtrace = Backtrace::force_capture();
        assert_eq!(report(&Layer("failed", None), Some(&backtrace)), "Error: failed");
    }
}
