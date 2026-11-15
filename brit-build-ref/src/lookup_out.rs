//! Printing a verified read without moving the contracts other tools parse.

use brit_epr::elohim::{
    attestation::note::{NoteBody, VersionedNote},
    refs::{HeldReason, Lookup},
};

/// Print a lookup's answer: a verified note's view ([`VersionedNote::view`]) as pretty JSON on
/// stdout. Otherwise `miss` is the first stderr line — the line other tools parse, unchanged
/// whether nothing is stored, what is stored failed verification, or what is stored is held —
/// followed by the reason when a stored record was refused or held. Exit 0 either way: a lookup
/// that git answered is an answer.
pub fn print<B: NoteBody>(lookup: Lookup<VersionedNote<B>>, miss: &str) -> anyhow::Result<()> {
    match lookup {
        // Emitted from the typed note, never from the stored bytes.
        Lookup::Verified(note) => println!("{}", serde_json::to_string_pretty(&note.view()?)?),
        Lookup::Absent => eprintln!("{miss}"),
        Lookup::Unverifiable { at, reason } => {
            eprintln!("{miss}");
            unverifiable(&at, &reason);
        }
        Lookup::Held { at, reason } => {
            eprintln!("{miss}");
            held(&at, reason);
        }
    }
    Ok(())
}

/// The one stderr line for a stored record that failed verification.
pub fn unverifiable(at: &str, reason: &str) {
    eprintln!("unverifiable: {at} {}", reason.replace('\n', " "));
}

/// The one stderr line for a stored record this reader holds: framed in a note epoch or
/// signature scheme it does not implement. Never a row, never a reuse candidate, and not an
/// `unverifiable:` line — it is neither authentic nor forged here.
pub fn held(at: &str, reason: HeldReason) {
    eprintln!("held: {at} {reason}");
}
