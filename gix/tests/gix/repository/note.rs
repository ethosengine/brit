use gix_testtools::{Result, TestResult};

#[test]
fn visiting_notes_preserves_bytes_order_and_callback_errors_above_buffer_policy() -> TestResult {
    let (repo, _tmp) = crate::util::basic_rw_repo()?;
    let target = repo.write_blob(b"annotated")?.detach();
    let large = vec![b'n'; 64 * 1024];
    repo.notes()?.replace("first", target, &large)?;
    repo.notes()?.replace("second", target, b"second\0note")?;
    // Reopen so a write buffer already large enough cannot satisfy later reads.
    let repo = gix::open_opts(
        repo.git_dir(),
        gix::open::Options::isolated().config_overrides(["gitoxide.objects.allocLimit=1024"]),
    )?;

    let mut notes = repo.notes()?.with_refs(["refs/notes/first", "refs/notes/second"])?;
    assert!(
        notes.get(target).is_err(),
        "the owned-blob API retains its explicit allocation limit"
    );
    let mut found = Vec::new();
    notes.for_each(target, |reference, bytes| {
        found.push((reference.to_string(), bytes.to_vec()));
        Ok(())
    })?;
    assert_eq!(
        found,
        [
            ("refs/notes/first".into(), large),
            ("refs/notes/second".into(), b"second\0note".to_vec())
        ],
        "bounded visits preserve the configured order and exact bytes"
    );
    let mut calls = 0;
    let error = notes
        .for_each(target, |_, _| {
            calls += 1;
            Err(gix::error::message("visitor stopped").into())
        })
        .expect_err("callback failure stops visitation");
    assert_eq!(calls, 1, "a failed visitor never receives a later note");
    assert!(
        error.to_string().contains("visitor stopped"),
        "the callback diagnostic is preserved"
    );
    Ok(())
}

#[test]
fn visiting_notes_maps_large_tree_metadata_and_rejects_non_blob_notes() -> TestResult {
    use gix::objs::{
        Tree,
        tree::{Entry, EntryKind},
    };
    use gix::refs::transaction::PreviousValue;

    let (repo, _tmp) = crate::util::basic_rw_repo()?;
    let target = repo.write_blob(b"annotated")?.detach();
    let note_blob_id = repo.write_blob(b"note bytes")?.detach();
    let mut tree = Tree {
        entries: vec![Entry {
            mode: EntryKind::Blob.into(),
            filename: target.to_string().into(),
            oid: note_blob_id,
        }],
    };
    tree.entries.extend((0..128).map(|index| Entry {
        mode: EntryKind::Blob.into(),
        filename: format!("non-note-{index:04}").into(),
        oid: note_blob_id,
    }));
    tree.entries.sort();
    let root_tree_id = repo.write_object(&tree)?.detach();
    repo.reference(
        "refs/notes/large-tree",
        root_tree_id,
        PreviousValue::MustNotExist,
        "test notes",
    )?;
    tree.entries[0].oid = root_tree_id;
    let corrupt_tree_id = repo.write_object(&tree)?.detach();
    repo.reference(
        "refs/notes/wrong-kind",
        corrupt_tree_id,
        PreviousValue::MustNotExist,
        "test wrong kind",
    )?;
    let repo = gix::open_opts(
        repo.git_dir(),
        gix::open::Options::isolated().config_overrides(["gitoxide.objects.allocLimit=1024"]),
    )?;
    assert!(
        repo.find_object(root_tree_id).is_err(),
        "the tree independently exceeds the buffering policy"
    );
    let mut calls = 0;
    repo.notes()?
        .with_refs(["refs/notes/large-tree"])?
        .for_each(target, |_, bytes| {
            assert_eq!(bytes, b"note bytes", "large metadata still resolves the original note");
            calls += 1;
            Ok(())
        })?;
    assert_eq!(calls, 1);
    let error = repo
        .notes()?
        .with_refs(["refs/notes/wrong-kind"])?
        .for_each(target, |_, _| {
            panic!("a tree masquerading as a note must never reach the visitor")
        })
        .expect_err("note object kind is validated even when mapped");
    assert!(error.to_string().contains("to be a blob"));
    Ok(())
}

#[test]
fn query_and_mutate_a_configured_notes_ref() -> TestResult {
    let (mut repo, _tmp) = crate::util::basic_rw_repo()?;
    let mut config = repo.config_snapshot_mut();
    config.set_value(&gix::config::tree::Core::NOTES_REF, "refs/notes/review")?;
    config.commit()?;

    assert!(
        repo.try_find_reference("refs/notes/review")?.is_none(),
        "configuring a notes reference does not create it"
    );
    let target = repo.write_blob(b"annotated")?;
    let mut notes = repo.notes()?;
    assert_eq!(
        notes.default_ref().map(ToString::to_string).as_deref(),
        Some("refs/notes/review"),
        "core.notesRef configures the default notes reference"
    );
    assert_eq!(
        notes.refs().map(ToString::to_string).collect::<Vec<_>>(),
        ["refs/notes/review"],
        "the default reference is initially the only selected notes reference"
    );
    assert!(notes.get(target)?.is_empty(), "the target initially has no notes");

    assert_eq!(
        notes.replace("review", target, b"first")?,
        None,
        "adding the first note does not replace an existing note"
    );
    assert!(
        repo.try_find_reference("refs/notes/review")?.is_some(),
        "adding a note creates its previously absent notes reference"
    );
    let found = notes.get(target)?;
    assert_eq!(found.len(), 1, "the target has exactly one note after insertion");
    assert_eq!(
        found[0].reference, "refs/notes/review",
        "the note is found through the configured notes reference"
    );
    assert_eq!(found[0].blob.data, b"first", "the inserted note data is returned");
    drop(found);

    let previous = notes
        .replace("review", target, b"second")?
        .expect("the first note is replaced");
    assert_eq!(
        repo.find_blob(previous)?.data,
        b"first",
        "replacement returns the previous note object"
    );
    assert_eq!(
        notes.remove("review", target)?,
        Some(repo.write_blob(b"second")?),
        "removal returns the repository-attached ID of the replacement note"
    );
    assert!(notes.get(target)?.is_empty(), "the target has no notes after removal");
    assert!(
        repo.try_find_reference("refs/notes/review")?.is_some(),
        "removing the last note preserves the notes reference"
    );
    Ok(())
}

#[test]
fn mutations_follow_symbolic_references_to_their_direct_target() -> TestResult {
    use gix::refs::{
        FullName, Target, TargetRef,
        transaction::{PreviousValue, RefEdit},
    };

    fn full_name(name: &str) -> FullName {
        name.try_into().expect("test reference names are valid")
    }

    fn create_symbolic_ref(repo: &gix::Repository, name: &str, target: &str) -> Result {
        repo.edit_reference(RefEdit::update(
            full_name(name),
            Target::Symbolic(full_name(target)),
            PreviousValue::MustNotExist,
            "create symbolic notes reference",
        ))?;
        Ok(())
    }

    fn assert_symbolic_target(repo: &gix::Repository, name: &str, target: &str) -> Result {
        assert_eq!(
            repo.find_reference(name)?.target(),
            TargetRef::Symbolic(target.try_into()?),
            "{name} remains symbolic and points to its original target"
        );
        Ok(())
    }

    let (repo, _tmp) = crate::util::basic_rw_repo()?;
    let annotated_blob_id = repo.write_blob(b"annotated")?;
    let direct = "refs/notes/direct";
    let inner_alias = "refs/notes/inner-alias";
    let outer_alias = "refs/notes/outer-alias";

    repo.notes()?.replace(direct, annotated_blob_id, b"first")?;

    let initial_commit_id = repo.find_reference(direct)?.id();
    create_symbolic_ref(&repo, inner_alias, direct)?;
    create_symbolic_ref(&repo, outer_alias, inner_alias)?;

    let mut notes = repo.notes()?.with_refs([outer_alias, direct])?;
    let found = notes.get(annotated_blob_id)?;
    assert_eq!(
        found.len(),
        2,
        "the alias and its direct target both provide the note, it's what Git does"
    );
    assert!(
        found.iter().all(|note| note.blob.data == b"first"),
        "both selected references initially cache the same notes tree"
    );
    drop(found);

    let previous_note_blob_id = notes
        .replace(outer_alias, annotated_blob_id, b"second")?
        .expect("the existing note is replaced through the symbolic chain");
    assert_eq!(
        repo.find_blob(previous_note_blob_id)?.data,
        b"first",
        "replacement returns the previous note"
    );
    assert_symbolic_target(&repo, outer_alias, inner_alias)?;
    assert_symbolic_target(&repo, inner_alias, direct)?;

    let replacement_commit_id = repo.find_reference(direct)?.id();
    assert_ne!(
        replacement_commit_id, initial_commit_id,
        "replacement advances the ultimate direct target"
    );
    assert_eq!(
        repo.find_commit(replacement_commit_id)?
            .parent_ids()
            .collect::<Vec<_>>(),
        [initial_commit_id],
        "the replacement commit parents the previous direct target"
    );
    let found = notes.get(annotated_blob_id)?;
    assert_eq!(found.len(), 2, "both selected references still provide the note");
    assert!(
        found.iter().all(|note| note.blob.data == b"second"),
        "cached alias and direct-target roots both observe the replacement"
    );
    drop(found);

    assert_eq!(
        notes.remove(outer_alias, annotated_blob_id)?,
        Some(repo.write_blob(b"second")?),
        "removal through the symbolic chain returns the repository-attached note ID"
    );
    assert_symbolic_target(&repo, outer_alias, inner_alias)?;
    assert_symbolic_target(&repo, inner_alias, direct)?;

    let removal_commit_id = repo.find_reference(direct)?.id();
    assert_ne!(
        removal_commit_id, replacement_commit_id,
        "removal advances the ultimate direct target"
    );
    assert_eq!(
        repo.find_commit(removal_commit_id)?.parent_ids().collect::<Vec<_>>(),
        [replacement_commit_id],
        "the removal commit parents the previous direct target"
    );
    assert!(
        notes.get(annotated_blob_id)?.is_empty(),
        "cached alias and direct-target roots both observe the removal"
    );
    Ok(())
}

#[test]
fn mutations_reject_non_commit_notes_ref_targets() -> TestResult {
    use gix::refs::transaction::PreviousValue;

    let (repo, _tmp) = crate::util::basic_rw_repo()?;
    let annotated_blob_id = repo.write_blob(b"annotated")?;
    let valid_notes_ref = "refs/notes/valid";
    repo.notes()?.replace(valid_notes_ref, annotated_blob_id, b"note")?;

    let valid_notes_commit_id = repo.find_reference(valid_notes_ref)?.id();
    let notes_tree_id = repo.find_commit(valid_notes_commit_id)?.tree_id()?;
    let notes_tag_id = repo
        .tag(
            "notes-history",
            valid_notes_commit_id,
            gix::object::Kind::Commit,
            None,
            "tagged notes history",
            PreviousValue::MustNotExist,
        )?
        .id();

    let tree_ref = "refs/notes/tree";
    repo.reference(
        tree_ref,
        notes_tree_id,
        PreviousValue::MustNotExist,
        "create malformed tree-based notes reference",
    )?;
    repo.notes()?
        .replace(tree_ref, annotated_blob_id, b"replacement")
        .expect_err("a notes tree cannot be used as a commit parent");
    assert_eq!(
        repo.find_reference(tree_ref)?.id(),
        notes_tree_id,
        "failed replacement leaves the tree-based notes reference unchanged"
    );

    let tag_ref = "refs/notes/tag";
    repo.reference(
        tag_ref,
        notes_tag_id,
        PreviousValue::MustNotExist,
        "create malformed tag-based notes reference",
    )?;
    repo.notes()?
        .remove(tag_ref, annotated_blob_id)
        .expect_err("an annotated tag cannot be used as a commit parent");
    assert_eq!(
        repo.find_reference(tag_ref)?.id(),
        notes_tag_id,
        "failed removal leaves the tag-based notes reference unchanged"
    );
    Ok(())
}

#[test]
fn custom_commit_message_is_used_for_mutations() -> TestResult {
    let (repo, _tmp) = crate::util::basic_rw_repo()?;
    let annotated_blob_id = repo.write_blob(b"annotated")?;
    let mut notes = repo.notes()?.with_commit_message("custom notes update");

    notes.replace("review", annotated_blob_id, b"note")?;
    {
        let commit = repo.find_reference("refs/notes/review")?.peel_to_commit()?;
        assert_eq!(
            commit.message_raw()?,
            "custom notes update",
            "replacement uses the configured commit message"
        );
    }

    notes.remove("review", annotated_blob_id)?;
    let commit = repo.find_reference("refs/notes/review")?.peel_to_commit()?;
    assert_eq!(
        commit.message_raw()?,
        "custom notes update",
        "removal uses the configured commit message"
    );
    Ok(())
}

#[test]
fn query_and_mutate_multiple_notes_refs() -> TestResult {
    let (repo, _tmp) = crate::util::basic_rw_repo()?;
    let target = repo.write_blob(b"annotated")?;
    let notes_refs = ["refs/notes/review", "refs/notes/security"];
    let mut notes = repo.notes()?.with_refs(notes_refs)?;
    assert_eq!(
        notes.refs().map(ToString::to_string).collect::<Vec<_>>(),
        notes_refs,
        "explicitly selected notes references are available in lookup order"
    );

    for name in notes_refs {
        assert!(
            repo.try_find_reference(name)?.is_none(),
            "selecting {name} does not create it"
        );
    }

    assert_eq!(
        notes.replace("review", target, b"review note")?,
        None,
        "adding the review note creates a new mapping"
    );
    assert_eq!(
        notes.replace("notes/security", target, b"security note")?,
        None,
        "adding the security note creates an independent mapping"
    );

    for name in notes_refs {
        assert!(
            repo.try_find_reference(name)?.is_some(),
            "writing a note auto-creates {name}"
        );
    }

    let unmatched = repo.notes()?.with_refs(["/refs/notes/*", "refs/notes/*/"])?;
    assert_eq!(
        unmatched.refs().count(),
        0,
        "reference names have neither a leading nor trailing slash"
    );

    notes = notes.with_refs(["refs/notes/revie?", "refs/notes/[s]ecurity"])?;
    assert_eq!(
        notes.refs().map(ToString::to_string).collect::<Vec<_>>(),
        notes_refs,
        "question-mark and bracket globs expand without requiring an asterisk"
    );

    let found = notes.get(target)?;
    assert_eq!(found.len(), 2, "one note is returned from each selected reference");
    assert_eq!(
        found[0].reference, "refs/notes/review",
        "the first note follows the selected reference order"
    );
    assert_eq!(found[0].blob.data, b"review note", "the review note data is returned");
    assert_eq!(
        found[1].reference, "refs/notes/security",
        "the second note follows the selected reference order"
    );
    assert_eq!(
        found[1].blob.data, b"security note",
        "the security note data is returned"
    );
    Ok(())
}

#[test]
fn add_to_an_exact_fully_qualified_reference() -> TestResult {
    let (repo, _tmp) = crate::util::basic_rw_repo()?;
    let target = repo.write_blob(b"annotated")?.detach();
    let reference: gix::refs::FullName = "refs/worktree/tix/notes".try_into()?;
    let mut notes = repo.notes()?;

    assert_eq!(
        notes.replace_at_ref(reference.as_ref(), target, b"[commit]\n\ttodo = true\n")?,
        None
    );
    let mut exact = repo.notes()?.with_refs([reference.as_bstr()])?;
    assert_eq!(
        exact.get(target)?.first().map(|note| note.blob.data.as_slice()),
        Some(b"[commit]\n\ttodo = true\n".as_slice()),
        "the exact worktree-local ref stores the note"
    );
    assert!(
        repo.try_find_reference("refs/notes/refs/worktree/tix/notes")?.is_none(),
        "exact writes do not apply notes shorthand"
    );
    Ok(())
}
