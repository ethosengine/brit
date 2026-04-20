//! Self-tests for the MockRemote helper.

use cli_journey::support::mock_remote::MockRemote;

#[test]
fn mock_remote_url_is_file_scheme() {
    let remote = MockRemote::new("upstream").expect("init");
    let url = remote.url();
    assert!(url.starts_with("file://"), "url: {url}");
    assert!(url.contains(".git"), "url contains .git: {url}");
}

#[test]
fn mock_remote_is_a_bare_repo() {
    let remote = MockRemote::new("upstream").expect("init");
    // Bare repos don't have a .git/ directory; the dir IS the repo
    assert!(remote.path().exists());
    assert!(remote.path().join("HEAD").exists(), "bare repo has HEAD at top level");
}

#[test]
fn local_can_clone_from_mock_remote() {
    let upstream = MockRemote::new("upstream").expect("upstream init");
    let local_temp = tempfile::Builder::new()
        .prefix("brit-test-clone-")
        .tempdir()
        .expect("mktemp");
    let local_path = local_temp.path().join("clone");

    // git clone <url> <local_path>
    let status = std::process::Command::new("git")
        .args(["clone", "-q", &upstream.url()])
        .arg(&local_path)
        .status()
        .expect("git clone");
    assert!(status.success(), "clone succeeded");
    assert!(local_path.join(".git").exists(), ".git in clone");
}
