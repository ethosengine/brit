//! How every brit binary prints a failure.
//!
//! A failed command ends its stderr with the error itself: `Error: <message>`, then its
//! causes in anyhow's `Caused by:` layout. `RUST_BACKTRACE` and `RUST_LIB_BACKTRACE` may make
//! a library capture a backtrace, but they never make brit print one: callers that report the
//! last stderr line must see the cause, not a stack frame. A backtrace is printed only when
//! [`BACKTRACE_ENV`] asks for one, and then before the error, so the error stays last.

use std::{backtrace::Backtrace, backtrace::BacktraceStatus, error::Error, fmt::Display, fmt::Write};

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

/// Whether [`BACKTRACE_ENV`] asks for backtraces.
pub fn backtrace_requested() -> bool {
    std::env::var_os(BACKTRACE_ENV).is_some_and(|value| !value.is_empty() && value != "0")
}

fn render_causes(message: String, first_cause: Option<&(dyn Error + 'static)>) -> String {
    let mut causes = Vec::new();
    let mut next = first_cause;
    while let Some(cause) = next {
        causes.push(cause.to_string());
        next = cause.source();
    }
    let mut output = format!("Error: {message}");
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

    #[test]
    fn a_captured_backtrace_is_not_printed_unless_brit_asks() {
        if backtrace_requested() {
            return;
        }
        let backtrace = Backtrace::force_capture();
        assert_eq!(report(&Layer("failed", None), Some(&backtrace)), "Error: failed");
    }
}
