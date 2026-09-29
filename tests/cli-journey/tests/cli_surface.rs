//! Public parser census is deterministic, standalone and cannot execute work.
use cli_journey::support::runner::{brit_bin, BritInvocation};

#[test]
fn compiled_surface_includes_composed_namespaces_and_version_provenance_without_a_repository() {
    let fixture = tempfile::tempdir().unwrap();
    let binary = brit_bin().expect("fresh Brit executable");
    let invoke = || {
        BritInvocation::new(&binary)
            .args(["--cli-surface-json"])
            .current_dir(fixture.path())
            .run()
            .unwrap()
    };
    let first = invoke();
    assert!(first.status.success(), "{}", first.stderr);
    assert!(first.stderr.is_empty());
    assert_eq!(first.stdout, invoke().stdout);
    let value: serde_json::Value = serde_json::from_str(&first.stdout).unwrap();
    assert_eq!(value["schemaVersion"], 1);
    assert_eq!(value["evidenceKind"], "parser-surface");
    let version = BritInvocation::new(&binary)
        .args(["--version"])
        .current_dir(fixture.path())
        .run()
        .unwrap();
    for field in ["sourceHead", "sourceState", "frontendPresets"] {
        assert!(
            version.stdout.contains(value[field].as_str().unwrap()),
            "{field} differs from built version metadata"
        );
    }
    let commands = value["command"]["subcommands"].as_array().unwrap();
    for name in ["build", "snapshot", "context", "status", "commit"] {
        assert!(
            commands.iter().any(|command| command["name"] == name),
            "missing assembled {name}"
        );
    }
    let snapshot = commands.iter().find(|command| command["name"] == "snapshot").unwrap();
    assert!(!snapshot["subcommands"].as_array().unwrap().is_empty());
    assert!(value["command"]["options"]
        .as_array()
        .unwrap()
        .iter()
        .any(|arg| arg["long"] == "cli-surface-json"));
    assert_eq!(
        std::fs::read_dir(fixture.path()).unwrap().count(),
        0,
        "introspection must not initialize repositories or write artifacts"
    );
}

#[test]
fn malformed_introspection_never_executes_the_requested_operation() {
    let fixture = tempfile::tempdir().unwrap();
    let binary = brit_bin().expect("fresh Brit executable");
    for args in [
        vec!["--cli-surface-json", "init", "would-write"],
        vec!["init", "would-write", "--cli-surface-json"],
        vec!["-C", ".", "--cli-surface-json", "init", "would-write"],
    ] {
        let output = BritInvocation::new(&binary)
            .args(args)
            .current_dir(fixture.path())
            .run()
            .unwrap();
        assert!(!output.status.success(), "malformed invocation must refuse");
        assert_eq!(
            std::fs::read_dir(fixture.path()).unwrap().count(),
            0,
            "introspection must not dispatch init"
        );
    }
}
