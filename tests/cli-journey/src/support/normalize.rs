//! Normalizer — redact variable bits of CLI output for stable snapshots.
//!
//! Replacements applied (in order):
//!   1. ANSI escape codes → stripped
//!   2. Tempdir paths → <TMPDIR>/...
//!   3. RFC 3339 timestamps → <TIMESTAMP>
//!   4. Git SHAs (40-char and 7-char hex) → <SHA>
//!      (with optional allowlist for "stable" SHAs that flow through verbatim)

use std::collections::HashSet;

use regex::Regex;

pub struct Normalizer {
    ansi_re: Regex,
    posix_tempdir_re: Regex,
    macos_tempdir_re: Regex,
    rfc3339_re: Regex,
    clock_time_re: Regex,
    duration_re: Regex,
    throughput_re: Regex,
    sha40_re: Regex,
    sha7_re: Regex,
    stable_shas: HashSet<String>,
}

impl Default for Normalizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Normalizer {
    pub fn new() -> Self {
        Self {
            ansi_re: Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap(),
            // /tmp/anything-up-to-next-space-or-end OR /tmp/path/with/segments
            // brit-test-XXX or brit-mockremote-XXX prefixes
            posix_tempdir_re: Regex::new(
                r"/tmp/(?:brit-test-|brit-mockremote-|brit-)[A-Za-z0-9_\-.]+",
            )
            .unwrap(),
            // macOS: /var/folders/XX/YYYY/T/brit-test-...
            macos_tempdir_re: Regex::new(
                r"/var/folders/[A-Za-z0-9_]+/[A-Za-z0-9_]+/T/(?:brit-test-|brit-)[A-Za-z0-9_\-.]+",
            )
            .unwrap(),
            rfc3339_re: Regex::new(
                r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:[+-]\d{2}:\d{2}|Z)",
            )
            .unwrap(),
            // Wall-clock HH:MM:SS at the start of progress lines (gitoxide).
            // E.g. " 23:19:01 indexing done ..." → " <CLOCK> indexing done ..."
            clock_time_re: Regex::new(r"\b\d{2}:\d{2}:\d{2}\b").unwrap(),
            // Duration: "0.06s", "0.00s", "1.23s" — common in gitoxide progress lines.
            duration_re: Regex::new(r"\b\d+\.\d{2}s\b").unwrap(),
            // Throughput: "15.4k objects/s", "1.3MB/s", "450B/s", etc.
            // Pattern: number (optional unit prefix) + (objects|files|B)/s
            throughput_re: Regex::new(
                r"\b\d+(?:\.\d+)?[kMG]?(?:B| objects| files)/s\b",
            )
            .unwrap(),
            sha40_re: Regex::new(r"\b[0-9a-f]{40}\b").unwrap(),
            sha7_re: Regex::new(r"\b[0-9a-f]{7,12}\b").unwrap(),
            stable_shas: HashSet::new(),
        }
    }

    /// Mark a SHA (40-char or 7-char) as "stable" — flows through verbatim
    /// instead of being redacted. Use for fixed-content commits whose SHAs
    /// are known to be deterministic via `set-static-git-environment`.
    pub fn add_stable_sha(&mut self, sha: &str) {
        self.stable_shas.insert(sha.to_string());
        // Also add the 7-char abbreviation, which is what `git log --oneline` uses
        if sha.len() >= 7 {
            self.stable_shas.insert(sha[..7].to_string());
        }
    }

    /// Apply all normalizations. Order matters (ANSI first to avoid eating
    /// other patterns; SHAs last since they may contain hex digits that
    /// would otherwise look like nothing).
    pub fn normalize(&self, text: &str) -> String {
        let mut out = self.ansi_re.replace_all(text, "").into_owned();
        out = self.macos_tempdir_re.replace_all(&out, "<TMPDIR>").into_owned();
        out = self.posix_tempdir_re.replace_all(&out, "<TMPDIR>").into_owned();
        out = self.rfc3339_re.replace_all(&out, "<TIMESTAMP>").into_owned();
        out = self.clock_time_re.replace_all(&out, "<CLOCK>").into_owned();
        out = self.throughput_re.replace_all(&out, "<RATE>").into_owned();
        out = self.duration_re.replace_all(&out, "<DUR>").into_owned();
        // SHA redaction: only redact ones not in the stable set.
        out = self
            .sha40_re
            .replace_all(&out, |caps: &regex::Captures<'_>| {
                let m = caps.get(0).unwrap().as_str();
                if self.stable_shas.contains(m) {
                    m.to_string()
                } else {
                    "<SHA>".to_string()
                }
            })
            .into_owned();
        out = self
            .sha7_re
            .replace_all(&out, |caps: &regex::Captures<'_>| {
                let m = caps.get(0).unwrap().as_str();
                if self.stable_shas.contains(m) {
                    m.to_string()
                } else {
                    "<SHA>".to_string()
                }
            })
            .into_owned();
        out
    }
}
