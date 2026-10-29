//! Printing a verified read without moving the contracts other tools parse.

use brit_epr::elohim::refs::Lookup;

/// Print a lookup's answer: a verified node as pretty JSON on stdout. Otherwise `miss` is the
/// first stderr line — the line other tools parse, unchanged whether nothing is stored or what is
/// stored failed verification — followed by the reason when a stored record was refused. Exit 0
/// either way: a lookup that git answered is an answer.
pub fn print<T: serde::Serialize>(lookup: Lookup<T>, miss: &str) -> anyhow::Result<()> {
    match lookup {
        // Emitted from the typed node, never from the stored bytes.
        Lookup::Verified(node) => println!("{}", serde_json::to_string_pretty(&serde_json::to_value(&node)?)?),
        Lookup::Absent => eprintln!("{miss}"),
        Lookup::Unverifiable { at, reason } => {
            eprintln!("{miss}");
            unverifiable(&at, &reason);
        }
    }
    Ok(())
}

/// The one stderr line for a stored record that failed verification.
pub fn unverifiable(at: &str, reason: &str) {
    eprintln!("unverifiable: {at} {}", reason.replace('\n', " "));
}
