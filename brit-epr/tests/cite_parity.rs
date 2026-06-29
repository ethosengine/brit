//! Verdict-label parity with the parent cite oracle on the fixture corpus.
//! Skips (returns) when the oracle (parent monorepo) is not on disk.
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

#[test]
fn brit_verdicts_match_oracle() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cite_corpus");
    let py = Command::new("python3")
        .arg(dir.join("oracle.py"))
        .arg(&dir)
        .output()
        .unwrap();
    if py.status.code() == Some(3) {
        eprintln!("oracle absent; skipping parity");
        return;
    }
    assert!(py.status.success(), "oracle: {}", String::from_utf8_lossy(&py.stderr));
    let oracle: BTreeMap<String, String> = serde_json::from_slice(&py.stdout).unwrap();

    let idx = brit_epr::engine::cite::SlugIndex::build(std::slice::from_ref(&dir)).unwrap();
    let mut brit = BTreeMap::new();
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_none_or(|x| x != "md") {
            continue;
        }
        let c = std::fs::read_to_string(&p).unwrap();
        let (fm, _) = brit_epr::engine::frontmatter::split_frontmatter(&c);
        let Some(fm) = fm else { continue };
        let id = brit_epr::engine::cite::extract_id(fm).unwrap_or_default();
        for edge in brit_epr::engine::cite::extract_cites(fm) {
            let v = format!("{:?}", brit_epr::engine::verdict::verdict(&edge, &idx)).to_lowercase();
            brit.insert(format!("{id}|{}", edge.ref_), v);
        }
    }
    assert_eq!(brit, oracle, "verdict-label divergence brit (left) vs oracle (right)");
}
