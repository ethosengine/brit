//! The public context boundary preserves the evaluator's input, output and failure status.

#[cfg(unix)]
mod unix {
    use cli_journey::support::runner::{brit_bin, BritInvocation};
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn standalone_context_delegates_without_reinterpreting_evidence() {
        let fixture = tempfile::tempdir().expect("isolated standalone directory");
        let epr = fixture.path().join("epr");
        std::fs::write(
            &epr,
            b"#!/bin/sh\nprintf '%s\\n' \"$@\"\nprintf 'unresolved review\\n' >&2\nexit 7\n",
        )
        .expect("fake installed evaluator");
        std::fs::set_permissions(&epr, std::fs::Permissions::from_mode(0o755)).expect("executable evaluator");
        let result = BritInvocation::new(brit_bin().expect("public executable"))
            .args(["context", "feature with spaces.md", "--json", "--root", "."])
            .env("PATH", fixture.path())
            .current_dir(fixture.path())
            .run()
            .expect("invoke standalone context");
        assert_eq!(result.status.code(), Some(7));
        assert_eq!(
            result.stdout,
            "flow\ncontext\nfeature with spaces.md\n--json\n--root\n.\n"
        );
        assert_eq!(result.stderr, "unresolved review\n");
    }
}

use cli_journey::support::runner::{brit_bin, BritInvocation};

#[test]
fn unavailable_evaluator_refuses_with_installation_action() {
    let fixture = tempfile::tempdir().expect("empty PATH");
    let result = BritInvocation::new(brit_bin().expect("public executable"))
        .args(["context", "feature.md"])
        .env("PATH", fixture.path())
        .current_dir(fixture.path())
        .run()
        .expect("invoke public executable");
    assert!(!result.status.success());
    assert!(result.stderr.contains("install epr"));
    assert!(result.stdout.is_empty());
}
