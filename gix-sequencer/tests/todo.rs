use gix_sequencer::{Command, MappingLedger, Todo};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

fn ids() -> Vec<String> {
    ["1", "2", "3", "4"].into_iter().map(|digit| digit.repeat(40)).collect()
}

#[test]
fn every_selected_command_and_alias_has_identical_consumption() -> Result {
    let ids = ids();
    for (long, short, expected) in [
        ("pick", "p", Command::Pick),
        ("reword", "r", Command::Reword),
        ("edit", "e", Command::Edit),
        ("squash", "s", Command::Squash),
        ("fixup", "f", Command::Fixup),
        ("drop", "d", Command::Drop),
    ] {
        for spelling in [long, short] {
            let bytes = format!("pick {}\n{spelling} {} displayed subject\n", ids[0], ids[1]);
            let todo = Todo::parse(bytes.as_bytes(), &ids[..2])?;
            assert_eq!(todo.entries[1].command, expected);
            assert_eq!(todo.entries[1].commit.as_deref(), Some(ids[1].as_str()));
        }
    }
    for spelling in ["break", "b"] {
        let todo = Todo::parse(format!("{spelling}\npick {}\n", ids[0]).as_bytes(), &ids[..1])?;
        assert_eq!(todo.entries[0].command, Command::Break);
        assert_eq!(todo.entries[0].commit, None);
    }
    Ok(())
}

#[test]
fn raw_subjects_comments_blank_lines_and_full_sha256_ids_are_preserved() -> Result {
    let id = "a".repeat(64);
    let mut bytes = format!("# arbitrary comment\n\n  pick {id} subject ").into_bytes();
    bytes.extend_from_slice(b"\xff\xfe\n\t# final comment\n");
    let todo = Todo::parse(&bytes, std::slice::from_ref(&id))?;
    assert_eq!(todo.bytes, bytes);
    assert_eq!(todo.entries.len(), 1);
    assert_eq!(todo.entries[0].commit.as_deref(), Some(id.as_str()));
    Ok(())
}

#[test]
fn invalid_or_incomplete_todos_never_silently_consume_inputs() {
    let ids = ids();
    for bytes in [
        String::new(),
        format!("pick {}\n", ids[0]),
        format!("pick {}\npick {}\n", ids[0], ids[0]),
        format!("pick {}\npick {}\n", ids[0], ids[2]),
        format!("pick {}\npick deadbee\n", ids[0]),
        format!("pick {}\nexec {}\n", ids[0], ids[1]),
        format!("pick {}\nfixup -C {}\n", ids[0], ids[1]),
        format!("break {}\npick {}\npick {}\n", ids[2], ids[0], ids[1]),
        format!("pick\npick {}\npick {}\n", ids[0], ids[1]),
    ] {
        assert!(Todo::parse(bytes.as_bytes(), &ids[..2]).is_err(), "reject {bytes:?}");
    }
}

#[test]
fn group_target_survives_drop_and_break_but_neither_can_establish_one() -> Result {
    let ids = ids();
    for prefix in [String::new(), "break\n".to_owned(), format!("drop {}\n", ids[0])] {
        let remaining = if prefix.starts_with("drop") {
            &ids[..2]
        } else {
            &ids[1..2]
        };
        assert!(Todo::parse(format!("{prefix}squash {}\n", ids[1]).as_bytes(), remaining).is_err());
    }
    let todo = Todo::parse(
        format!("pick {}\ndrop {}\nbreak\nfixup {}\n", ids[0], ids[1], ids[2]).as_bytes(),
        &ids[..3],
    )?;
    assert_eq!(todo.entries.last().expect("fixup entry").command, Command::Fixup);
    assert!(Todo::parse_suffix(format!("squash {}\n", ids[0]).as_bytes(), &ids[..1], false).is_err());
    Todo::parse_suffix(format!("squash {}\n", ids[0]).as_bytes(), &ids[..1], true)?;
    Ok(())
}

#[test]
fn suffix_replacement_freezes_completed_entries_and_requires_every_remaining_input() -> Result {
    let ids = ids();
    let original = Todo::parse(
        format!("pick {}\npick {}\npick {}\n", ids[0], ids[1], ids[2]).as_bytes(),
        &ids[..3],
    )?;
    let replacement = original.replace_suffix(1, format!("edit {}\ndrop {}\n", ids[2], ids[1]).as_bytes(), true)?;
    assert_eq!(replacement.entries[0], original.entries[0]);
    assert_eq!(replacement.entries[1].command, Command::Edit);
    assert_eq!(replacement.entries[1].commit.as_deref(), Some(ids[2].as_str()));
    assert!(
        original
            .replace_suffix(
                1,
                format!("pick {}\npick {}\npick {}\n", ids[0], ids[1], ids[2]).as_bytes(),
                true
            )
            .is_err()
    );
    assert!(
        original
            .replace_suffix(1, format!("pick {}\n", ids[1]).as_bytes(), true)
            .is_err()
    );
    assert!(original.replace_suffix(4, b"", true).is_err());
    assert_eq!(
        original.entries[1].command,
        Command::Pick,
        "rejected replacements cannot mutate accepted todo"
    );
    Ok(())
}

#[test]
fn mapping_edges_keep_original_order_split_order_and_zero_result_drops() -> Result {
    let ids = ids();
    let mut ledger = MappingLedger::new(ids[..3].to_vec())?;
    ledger.record(&ids[2], &["a".repeat(40), "b".repeat(40)])?;
    ledger.record(&ids[0], &["c".repeat(40)])?;
    ledger.record(&ids[1], &[])?;
    assert_eq!(
        ledger.edges(),
        vec![
            (ids[0].clone(), "c".repeat(40)),
            (ids[2].clone(), "a".repeat(40)),
            (ids[2].clone(), "b".repeat(40))
        ]
    );
    assert!(ledger.record(&ids[0], &["d".repeat(40)]).is_err());
    assert!(ledger.record(&ids[3], &[]).is_err());
    Ok(())
}

#[test]
fn group_replacement_rewrites_every_constituent_and_tracks_superseded_objects() -> Result {
    let ids = ids();
    let mut ledger = MappingLedger::new(ids[..3].to_vec())?;
    let first = "a".repeat(40);
    let second = "b".repeat(40);
    let final_id = "c".repeat(40);
    ledger.record(&ids[0], std::slice::from_ref(&first))?;
    ledger.replace_group(&ids[..2], std::slice::from_ref(&second))?;
    let before = ledger.edges();
    assert!(
        ledger
            .replace_group(&ids[1..3], std::slice::from_ref(&final_id))
            .is_err(),
        "partial shared group cannot leave stale mapping edges"
    );
    assert_eq!(ledger.edges(), before);
    ledger.replace_group(&ids[..3], std::slice::from_ref(&final_id))?;
    assert_eq!(
        ledger.edges(),
        ids[..3]
            .iter()
            .map(|old| (old.clone(), final_id.clone()))
            .collect::<Vec<_>>()
    );
    assert!(ledger.superseded().contains(&first));
    assert!(ledger.superseded().contains(&second));
    Ok(())
}

#[test]
fn mapping_refuses_duplicate_origins_unrelated_groups_and_multiple_new_inputs() -> Result {
    let ids = ids();
    assert!(MappingLedger::new(vec![ids[0].clone(), ids[0].clone()]).is_err());
    let mut ledger = MappingLedger::new(ids.clone())?;
    ledger.record(&ids[0], &["a".repeat(40)])?;
    ledger.record(&ids[1], &["b".repeat(40)])?;
    assert!(ledger.replace_group(&ids[..2], &["c".repeat(40)]).is_err());
    assert!(
        ledger
            .replace_group(&[ids[0].clone(), ids[2].clone(), ids[3].clone()], &["c".repeat(40)])
            .is_err()
    );
    assert_eq!(ledger.edges().len(), 2);
    Ok(())
}
