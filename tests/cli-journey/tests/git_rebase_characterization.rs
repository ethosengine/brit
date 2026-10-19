//! Stock-Git behavior evidence for the Sprint 5c design, NOT implemented Brit support.
//! No Brit executable or recovery qualification is exercised here.
#![cfg(unix)]

use cli_journey::support::test_repo::TestRepo;
use std::process::Command;

fn characterize(case: &str) -> anyhow::Result<()> {
    let repo = TestRepo::new(case)?;
    let mut command = Command::new("bash");
    gix_testtools::configure_git_environment(&mut command, repo.path());
    let output = command
        .current_dir(repo.path())
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/git-rebase-characterization.sh"
        ))
        .arg(case)
        .output()?;
    assert!(
        output.status.success(),
        "Git observation {case}:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    print!("{}", String::from_utf8_lossy(&output.stdout));
    Ok(())
}

macro_rules! cases {
    ($($name:ident),+ $(,)?) => {$ (
        #[test]
        fn $name() -> anyhow::Result<()> { characterize(stringify!($name)) }
    )+};
}

cases!(
    attached,
    detached,
    empty_and_equivalent,
    newly_empty,
    merge_history,
    reword,
    squash,
    fixup,
    reorder_drop,
    edit_amend,
    edit_split,
    break_stop,
    invalid_todo,
    failed_editors,
    dirty_neighbors,
    text,
    add_add,
    modify_delete,
    rename,
    binary,
    symlink,
    mode,
    mode_conflict,
    directory,
    skip_abort,
    hooks,
    hook_failures,
    hooks_reword,
    hooks_squash,
    hooks_conflict,
    interruption_editor,
    interruption_hook,
    external_mutations,
    linked_worktree,
    untracked_collision,
    editor_precedence,
    external_index_and_worktree,
    ignored_collision,
    failed_prepare_hook,
    noop_fastforward
);
