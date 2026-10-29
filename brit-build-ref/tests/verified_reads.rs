//! Verified reads through the `brit-build-ref` CLI: a stored record is returned only when it
//! decodes strictly, its signature verifies against its own `agentId`, and it binds to the ref it
//! was found under. An unverifiable record is reported on stderr and never printed as a result;
//! a failed git read exits non-zero instead of reading as absence.
//!
//! `BRIT_BUILD_REF_BIN` selects the binary under test (default: this package's build).

use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

use brit_epr::{
    elohim::attestation::{build::BuildAttestationContentNode, covenant::CovenantClauseKind},
    engine::{cid::BritCid, content_node::ContentNode, signing::AgentKey},
};
use serde_json::Value;

const STEP: &str = "gate:elohim-storage";
const STEP_REF: &str = "refs/notes/brit/build/gate%3Aelohim-storage";

fn bin() -> PathBuf {
    std::env::var_os("BRIT_BUILD_REF_BIN")
        .filter(|v| !v.is_empty())
        .map_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_brit-build-ref")), PathBuf::from)
}

fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .expect("git output is UTF-8")
        .trim()
        .to_string()
}

fn git_stdin(repo: &Path, args: &[&str], input: &[u8]) -> String {
    use std::io::Write;
    let mut child = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("git spawns");
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(input)
        .expect("stdin accepts input");
    let out = child.wait_with_output().expect("git exits");
    assert!(out.status.success(), "git {args:?}");
    String::from_utf8(out.stdout)
        .expect("git output is UTF-8")
        .trim()
        .to_string()
}

fn repo() -> tempfile::TempDir {
    let tmp = tempfile::TempDir::new().expect("temporary repository");
    git(tmp.path(), &["init", "-q", "--initial-branch=main"]);
    git(tmp.path(), &["commit", "-q", "--allow-empty", "-m", "init"]);
    tmp
}

fn commit(repo: &Path, message: &str) -> String {
    git(repo, &["commit", "-q", "--allow-empty", "-m", message]);
    git(repo, &["rev-parse", "HEAD"])
}

fn brit(repo: &Path, args: &[&str]) -> Output {
    Command::new(bin())
        .arg("--repo")
        .arg(repo)
        .args(args)
        .output()
        .expect("brit-build-ref runs")
}

fn stdout(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).expect("stdout is UTF-8")
}

fn stderr(out: &Output) -> String {
    String::from_utf8(out.stderr.clone()).expect("stderr is UTF-8")
}

fn put_build(repo: &Path, step: &str, inputs: &str, success: bool, rev: &str) {
    let manifest = BritCid::compute_raw(b"manifest").to_string();
    let output = BritCid::compute_raw(b"output").to_string();
    let out = brit(
        repo,
        &[
            "build",
            "put",
            "--step",
            step,
            "--manifest",
            &manifest,
            "--output",
            &output,
            "--inputs-hash",
            inputs,
            "--success",
            if success { "true" } else { "false" },
            "--commit",
            rev,
        ],
    );
    assert!(out.status.success(), "build put: {}", stderr(&out));
}

/// The note on `commit` under `notes_ref`, as stored.
fn note(repo: &Path, notes_ref: &str, commit: &str) -> Value {
    serde_json::from_str(&git(repo, &["notes", "--ref", notes_ref, "show", commit])).expect("note is JSON")
}

/// Replace (or create) the note on `commit` with exactly `bytes`.
fn write_note_bytes(repo: &Path, notes_ref: &str, commit: &str, bytes: &[u8]) {
    let blob = git_stdin(repo, &["hash-object", "-w", "--stdin"], bytes);
    git(repo, &["notes", "--ref", notes_ref, "add", "-f", "-C", &blob, commit]);
}

fn write_note(repo: &Path, notes_ref: &str, commit: &str, value: &Value) {
    write_note_bytes(
        repo,
        notes_ref,
        commit,
        &serde_json::to_vec(value).expect("JSON encodes"),
    );
}

fn find(repo: &Path, step: &str, inputs: &str) -> (Vec<Value>, String) {
    let out = brit(repo, &["build", "find", "--step", step, "--inputs-hash", inputs]);
    assert!(out.status.success(), "find exits 0: {}", stderr(&out));
    let rows: Value = serde_json::from_str(&stdout(&out)).expect("find prints one JSON document");
    (rows.as_array().expect("find prints an array").clone(), stderr(&out))
}

fn unverifiable_lines(err: &str) -> Vec<&str> {
    err.lines().filter(|l| l.starts_with("unverifiable: ")).collect()
}

/// A signed build node for `step`, by `key`.
fn signed_build(key: &AgentKey, step: &str, inputs: &str, success: bool) -> BuildAttestationContentNode {
    let mut node = BuildAttestationContentNode {
        manifest_cid: BritCid::compute_raw(b"manifest"),
        step_name: step.to_string(),
        inputs_hash: inputs.to_string(),
        output_cid: BritCid::compute_raw(b"output"),
        agent_id: key.agent_id(),
        hardware_profile: serde_json::json!({"arch": "x86_64"}),
        build_duration_ms: 7,
        built_at: "2026-10-29T00:00:00Z".to_string(),
        success,
        signature: String::new(),
    };
    node.signature = key.sign(&node.canonical_bytes().expect("node encodes"));
    node
}

// ─── build find ──────────────────────────────────────────────────────────────

#[test]
fn find_drops_a_note_whose_success_was_flipped() {
    let tmp = repo();
    let red = commit(tmp.path(), "red");
    put_build(tmp.path(), STEP, "closure-1", false, &red);
    let mut v = note(tmp.path(), STEP_REF, &red);
    v["success"] = Value::Bool(true);
    write_note(tmp.path(), STEP_REF, &red, &v);

    let (rows, err) = find(tmp.path(), STEP, "closure-1");
    assert!(rows.is_empty(), "a flipped verdict is never returned: {rows:?}");
    let lines = unverifiable_lines(&err);
    assert_eq!(lines.len(), 1, "{err}");
    assert!(lines[0].starts_with(&format!("unverifiable: {red} ")), "{err}");
}

#[test]
fn find_drops_a_note_copied_under_another_step() {
    let tmp = repo();
    let c = commit(tmp.path(), "c");
    put_build(tmp.path(), "gate:other", "closure-1", true, &c);
    let other = note(tmp.path(), "refs/notes/brit/build/gate%3Aother", &c);
    write_note(tmp.path(), STEP_REF, &c, &other);

    let (rows, err) = find(tmp.path(), STEP, "closure-1");
    assert!(
        rows.is_empty(),
        "a note bound to another step is not this step's: {rows:?}"
    );
    assert_eq!(unverifiable_lines(&err).len(), 1, "{err}");
    let (rows, err) = find(tmp.path(), "gate:other", "closure-1");
    assert_eq!(rows.len(), 1, "the original still reads where it was written: {err}");
}

#[test]
fn find_drops_a_note_with_an_appended_field() {
    let tmp = repo();
    let c = commit(tmp.path(), "c");
    put_build(tmp.path(), STEP, "closure-1", true, &c);
    let mut v = note(tmp.path(), STEP_REF, &c);
    v["admitted"] = Value::Bool(true);
    write_note(tmp.path(), STEP_REF, &c, &v);

    let (rows, err) = find(tmp.path(), STEP, "closure-1");
    assert!(rows.is_empty(), "an unknown field is not decoded away: {rows:?}");
    assert_eq!(unverifiable_lines(&err).len(), 1, "{err}");
}

#[test]
fn find_returns_a_foreign_key_note_under_its_own_agent_id_and_no_standing() {
    let tmp = repo();
    let c = commit(tmp.path(), "c");
    let keys = tempfile::TempDir::new().expect("foreign key dir");
    let foreign = AgentKey::generate(&keys.path().join("foreign")).expect("foreign key");
    let node = signed_build(&foreign, STEP, "closure-1", true);
    write_note(
        tmp.path(),
        STEP_REF,
        &c,
        &serde_json::to_value(&node).expect("node encodes"),
    );

    let (rows, err) = find(tmp.path(), STEP, "closure-1");
    assert!(unverifiable_lines(&err).is_empty(), "{err}");
    assert_eq!(rows.len(), 1);
    let row = rows[0].as_object().expect("row is an object");
    let mut row_keys: Vec<&str> = row.keys().map(String::as_str).collect();
    row_keys.sort_unstable();
    assert_eq!(
        row_keys,
        ["attestation", "commit"],
        "no verdict flag rides beside the row"
    );
    assert_eq!(row["commit"], c.as_str());
    let att = row["attestation"].as_object().expect("attestation is an object");
    assert_eq!(
        att["agentId"],
        foreign.agent_id().as_str(),
        "provenance is the signer's own key"
    );
    let mut keys: Vec<&str> = att.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "agentId",
            "buildDurationMs",
            "builtAt",
            "hardwareProfile",
            "inputsHash",
            "manifestCid",
            "outputCid",
            "signature",
            "stepName",
            "success"
        ],
        "exactly the signed fields: brit emits no trusted/admitted/standing flag"
    );
}

#[test]
fn find_returns_a_valid_red_attestation() {
    let tmp = repo();
    let c = commit(tmp.path(), "c");
    put_build(tmp.path(), STEP, "closure-1", false, &c);
    let (rows, err) = find(tmp.path(), STEP, "closure-1");
    assert!(unverifiable_lines(&err).is_empty(), "{err}");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["attestation"]["success"], false);
}

#[test]
fn find_keeps_valid_rows_beside_a_dropped_one_and_ignores_other_closures() {
    let tmp = repo();
    let a = commit(tmp.path(), "a");
    put_build(tmp.path(), STEP, "closure-1", true, &a);
    let b = commit(tmp.path(), "b");
    put_build(tmp.path(), STEP, "closure-2", true, &b);
    let t = commit(tmp.path(), "t");
    write_note_bytes(tmp.path(), STEP_REF, &t, b"not json at all");

    let (rows, err) = find(tmp.path(), STEP, "closure-1");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["commit"], a.as_str());
    let lines = unverifiable_lines(&err);
    assert_eq!(
        lines.len(),
        1,
        "a valid note for another closure is not unverifiable: {err}"
    );
    assert!(lines[0].starts_with(&format!("unverifiable: {t} ")), "{err}");
}

#[test]
fn find_drops_a_note_whose_signature_is_not_hex_and_keeps_its_siblings() {
    let tmp = repo();
    let a = commit(tmp.path(), "a");
    put_build(tmp.path(), STEP, "closure-1", true, &a);
    let z = commit(tmp.path(), "z");
    let mut v = note(tmp.path(), STEP_REF, &a);
    v["signature"] = Value::String("zz".to_string());
    write_note(tmp.path(), STEP_REF, &z, &v);

    let (rows, err) = find(tmp.path(), STEP, "closure-1");
    assert_eq!(rows.len(), 1, "the valid note survives its sibling: {err}");
    assert_eq!(rows[0]["commit"], a.as_str());
    let lines = unverifiable_lines(&err);
    assert_eq!(lines.len(), 1, "{err}");
    assert!(lines[0].starts_with(&format!("unverifiable: {z} ")), "{err}");
    assert!(lines[0].contains("signature"), "{err}");
}

#[test]
fn find_drops_a_note_signed_by_one_key_and_claiming_another() {
    let tmp = repo();
    let a = commit(tmp.path(), "a");
    put_build(tmp.path(), STEP, "closure-1", true, &a);
    let keys = tempfile::TempDir::new().expect("key dir");
    let signer = AgentKey::generate(&keys.path().join("signer")).expect("signer key");
    let claimed = AgentKey::generate(&keys.path().join("claimed")).expect("claimed key");
    let mut v = serde_json::to_value(signed_build(&signer, STEP, "closure-1", true)).expect("node encodes");
    v["agentId"] = Value::String(claimed.agent_id());
    let i = commit(tmp.path(), "i");
    write_note(tmp.path(), STEP_REF, &i, &v);

    let (rows, err) = find(tmp.path(), STEP, "closure-1");
    assert_eq!(rows.len(), 1, "{err}");
    assert_eq!(rows[0]["commit"], a.as_str());
    let lines = unverifiable_lines(&err);
    assert_eq!(lines.len(), 1, "{err}");
    assert!(lines[0].starts_with(&format!("unverifiable: {i} ")), "{err}");
    assert!(lines[0].contains("signature"), "{err}");
}

#[test]
fn find_on_a_step_without_notes_is_an_empty_answer() {
    let tmp = repo();
    let (rows, err) = find(tmp.path(), "no-such-step", "closure-1");
    assert!(rows.is_empty());
    assert!(err.is_empty(), "{err}");
}

#[test]
fn find_fails_when_git_cannot_read_the_notes() {
    let tmp = repo();
    let blob = git_stdin(tmp.path(), &["hash-object", "-w", "--stdin"], b"not a notes commit");
    git(tmp.path(), &["update-ref", STEP_REF, &blob]);
    let out = brit(
        tmp.path(),
        &["build", "find", "--step", STEP, "--inputs-hash", "closure-1"],
    );
    assert!(
        !out.status.success(),
        "a failed notes read is not an empty answer: stdout={}",
        stdout(&out)
    );
    assert!(stdout(&out).trim().is_empty(), "no result document on failure");
}

// ─── build get ───────────────────────────────────────────────────────────────

#[test]
fn get_miss_keeps_the_miss_line_and_exit_zero() {
    let tmp = repo();
    let out = brit(tmp.path(), &["build", "get", "--step", STEP]);
    assert!(out.status.success());
    assert!(stdout(&out).is_empty());
    assert_eq!(
        stderr(&out).lines().next(),
        Some(format!("no build ref found for step={STEP} commit=HEAD").as_str())
    );
}

#[test]
fn get_returns_a_verified_note() {
    let tmp = repo();
    put_build(tmp.path(), STEP, "closure-1", true, "HEAD");
    let out = brit(tmp.path(), &["build", "get", "--step", STEP]);
    assert!(out.status.success(), "{}", stderr(&out));
    let v: Value = serde_json::from_str(&stdout(&out)).expect("get prints JSON");
    assert_eq!(v["stepName"], STEP);
    assert_eq!(v["inputsHash"], "closure-1");
}

#[test]
fn get_unverifiable_reads_as_the_miss_line_then_the_reason() {
    let tmp = repo();
    let head = git(tmp.path(), &["rev-parse", "HEAD"]);
    put_build(tmp.path(), STEP, "closure-1", false, "HEAD");
    let mut v = note(tmp.path(), STEP_REF, &head);
    v["success"] = Value::Bool(true);
    write_note(tmp.path(), STEP_REF, &head, &v);

    let out = brit(tmp.path(), &["build", "get", "--step", STEP]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        stdout(&out).is_empty(),
        "a tampered note is never printed: {}",
        stdout(&out)
    );
    let err = stderr(&out);
    let mut lines = err.lines();
    assert_eq!(
        lines.next(),
        Some(format!("no build ref found for step={STEP} commit=HEAD").as_str())
    );
    assert!(
        lines
            .next()
            .is_some_and(|l| l.starts_with(&format!("unverifiable: {head} "))),
        "{err}"
    );
}

#[test]
fn get_fails_when_git_cannot_read_the_notes() {
    let tmp = repo();
    let blob = git_stdin(tmp.path(), &["hash-object", "-w", "--stdin"], b"not a notes commit");
    git(tmp.path(), &["update-ref", STEP_REF, &blob]);
    let out = brit(tmp.path(), &["build", "get", "--step", STEP]);
    assert!(
        !out.status.success(),
        "a failed notes read is not a miss: {}",
        stderr(&out)
    );
}

// ─── deploy / validate get ───────────────────────────────────────────────────

#[test]
fn deploy_and_validate_get_drop_a_tampered_record() {
    let tmp = repo();
    let artifact = BritCid::compute_raw(b"artifact").to_string();
    let out = brit(
        tmp.path(),
        &[
            "deploy",
            "put",
            "--step",
            STEP,
            "--env",
            "staging",
            "--artifact",
            &artifact,
            "--endpoint",
            "https://x",
            "--health-check-epr",
            "epr:x/health",
        ],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    let out = brit(
        tmp.path(),
        &[
            "validate",
            "put",
            "--step",
            STEP,
            "--check",
            "lint@v1",
            "--artifact",
            &artifact,
            "--result",
            "fail",
        ],
    );
    assert!(out.status.success(), "{}", stderr(&out));

    let deploy = brit(tmp.path(), &["deploy", "get", "--step", STEP, "--env", "staging"]);
    assert!(deploy.status.success());
    let v: Value = serde_json::from_str(&stdout(&deploy)).expect("deploy get prints JSON");
    assert_eq!(v["environmentLabel"], "staging");
    let validate = brit(tmp.path(), &["validate", "get", "--step", STEP, "--check", "lint@v1"]);
    assert!(validate.status.success());
    let v: Value = serde_json::from_str(&stdout(&validate)).expect("validate get prints JSON");
    assert_eq!(v["result"], "fail", "a valid failing check is still returned");

    for (ref_name, field, value, args) in [
        (
            "refs/notes/brit/deploy/gate%3Aelohim-storage/staging",
            "healthStatus",
            "unreachable",
            vec!["deploy", "get", "--step", STEP, "--env", "staging"],
        ),
        (
            "refs/notes/brit/validate/gate%3Aelohim-storage/lint%40v1",
            "result",
            "pass",
            vec!["validate", "get", "--step", STEP, "--check", "lint@v1"],
        ),
    ] {
        let mut v: Value = serde_json::from_str(&git(tmp.path(), &["cat-file", "-p", ref_name])).expect("blob is JSON");
        v[field] = Value::String(value.to_string());
        let blob = git_stdin(
            tmp.path(),
            &["hash-object", "-w", "--stdin"],
            &serde_json::to_vec(&v).expect("JSON encodes"),
        );
        git(tmp.path(), &["update-ref", ref_name, &blob]);
        let out = brit(tmp.path(), &args);
        assert!(out.status.success(), "{}", stderr(&out));
        assert!(
            stdout(&out).is_empty(),
            "{ref_name}: tampered record printed: {}",
            stdout(&out)
        );
        let err = stderr(&out);
        assert!(err.lines().next().is_some_and(|l| l.starts_with("no ")), "{err}");
        assert_eq!(unverifiable_lines(&err).len(), 1, "{err}");
    }
}

/// A valid deploy or validation record filed under a ref it was not signed for does not read as
/// that ref's answer: deploy binds step and environment, validation binds its check.
#[test]
fn deploy_and_validate_get_drop_a_record_filed_under_another_ref() {
    let tmp = repo();
    let artifact = BritCid::compute_raw(b"artifact").to_string();
    let out = brit(
        tmp.path(),
        &[
            "deploy",
            "put",
            "--step",
            STEP,
            "--env",
            "staging",
            "--artifact",
            &artifact,
            "--endpoint",
            "https://x",
            "--health-check-epr",
            "epr:x/health",
        ],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    let out = brit(
        tmp.path(),
        &[
            "validate",
            "put",
            "--step",
            STEP,
            "--check",
            "lint@v1",
            "--artifact",
            &artifact,
            "--result",
            "pass",
        ],
    );
    assert!(out.status.success(), "{}", stderr(&out));

    let deploy_oid = git(
        tmp.path(),
        &["rev-parse", "refs/notes/brit/deploy/gate%3Aelohim-storage/staging"],
    );
    let validate_oid = git(
        tmp.path(),
        &["rev-parse", "refs/notes/brit/validate/gate%3Aelohim-storage/lint%40v1"],
    );
    git(
        tmp.path(),
        &[
            "update-ref",
            "refs/notes/brit/deploy/gate%3Aelohim-storage/prod",
            &deploy_oid,
        ],
    );
    git(
        tmp.path(),
        &["update-ref", "refs/notes/brit/deploy/other-step/staging", &deploy_oid],
    );
    git(
        tmp.path(),
        &[
            "update-ref",
            "refs/notes/brit/validate/gate%3Aelohim-storage/other",
            &validate_oid,
        ],
    );

    for args in [
        vec!["deploy", "get", "--step", STEP, "--env", "prod"],
        vec!["deploy", "get", "--step", "other-step", "--env", "staging"],
        vec!["validate", "get", "--step", STEP, "--check", "other"],
    ] {
        let out = brit(tmp.path(), &args);
        assert!(out.status.success(), "{args:?}: {}", stderr(&out));
        assert!(stdout(&out).is_empty(), "{args:?}: printed {}", stdout(&out));
        let err = stderr(&out);
        assert!(
            err.lines().next().is_some_and(|l| l.starts_with("no ")),
            "{args:?}: {err}"
        );
        assert_eq!(unverifiable_lines(&err).len(), 1, "{args:?}: {err}");
    }
}

// ─── covenant ────────────────────────────────────────────────────────────────

fn put_clause(repo: &Path, session: &str, quote: &str) -> Value {
    let out = brit(
        repo,
        &[
            "covenant",
            "put",
            "--session",
            session,
            "--kind",
            CovenantClauseKind::AffirmsStewardOfRecord.as_str(),
            "--party",
            "human:t",
            "--quote",
            quote,
            "--output-json",
        ],
    );
    assert!(out.status.success(), "covenant put: {}", stderr(&out));
    serde_json::from_str(&stdout(&out)).expect("covenant put prints JSON")
}

fn write_ref_json(repo: &Path, ref_name: &str, value: &Value) {
    let blob = git_stdin(
        repo,
        &["hash-object", "-w", "--stdin"],
        &serde_json::to_vec(value).expect("JSON encodes"),
    );
    git(repo, &["update-ref", ref_name, &blob]);
}

#[test]
fn covenant_reads_drop_records_that_do_not_verify_or_bind() {
    let tmp = repo();
    let good = put_clause(tmp.path(), "s1", "yes");
    let other = put_clause(tmp.path(), "s2", "also yes");
    let good_cid = good["cid"].as_str().expect("cid").to_string();
    let other_cid = other["cid"].as_str().expect("cid").to_string();

    // A valid record filed under a CID that is not its clause's.
    let mut wrong_cid = good.clone();
    wrong_cid["cid"] = Value::String(other_cid.clone());
    write_ref_json(
        tmp.path(),
        &format!("refs/notes/brit/covenant/s1/{other_cid}"),
        &wrong_cid,
    );
    // A clause whose words changed after signing, under its own CID.
    let mut tampered = good.clone();
    tampered["clause"]["quote"] = Value::String("no".into());
    let tampered_cid = BritCid::compute(b"tampered").to_string();
    tampered["cid"] = Value::String(tampered_cid.clone());
    write_ref_json(
        tmp.path(),
        &format!("refs/notes/brit/covenant/s1/{tampered_cid}"),
        &tampered,
    );

    let out = brit(tmp.path(), &["covenant", "list", "--session", "s1", "--output-json"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let rows: Value = serde_json::from_str(&stdout(&out)).expect("list prints JSON");
    let rows = rows.as_array().expect("list prints an array");
    assert_eq!(rows.len(), 1, "only the verified clause: {rows:?}");
    assert_eq!(rows[0]["cid"], good_cid.as_str());
    assert_eq!(
        rows[0], good,
        "the row is the stored record, emitted from its typed form"
    );
    assert_eq!(unverifiable_lines(&stderr(&out)).len(), 2, "{}", stderr(&out));

    let show = brit(
        tmp.path(),
        &["covenant", "show", "--cid", &tampered_cid, "--output-json"],
    );
    assert!(!show.status.success(), "a tampered clause is not shown");
    assert!(stdout(&show).trim().is_empty());
    let show = brit(tmp.path(), &["covenant", "show", "--cid", &good_cid, "--output-json"]);
    assert!(show.status.success(), "{}", stderr(&show));
}

#[test]
fn covenant_list_drops_a_clause_filed_under_another_session() {
    let tmp = repo();
    let other = put_clause(tmp.path(), "s2", "mine");
    let cid = other["cid"].as_str().expect("cid").to_string();
    write_ref_json(tmp.path(), &format!("refs/notes/brit/covenant/s1/{cid}"), &other);
    let out = brit(tmp.path(), &["covenant", "list", "--session", "s1", "--output-json"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out).trim(), "[]");
    assert_eq!(unverifiable_lines(&stderr(&out)).len(), 1, "{}", stderr(&out));
}

// ─── reach is gone ───────────────────────────────────────────────────────────

#[test]
fn reach_is_a_usage_error() {
    let tmp = repo();
    for verb in ["compute", "get"] {
        let out = brit(tmp.path(), &["reach", verb, "--step", STEP]);
        assert_eq!(out.status.code(), Some(2), "reach {verb}: {}", stderr(&out));
        assert!(stdout(&out).is_empty());
    }
    let refs = git(tmp.path(), &["for-each-ref", "refs/notes/brit/reach/"]);
    assert!(refs.is_empty(), "nothing was stamped: {refs}");
}
