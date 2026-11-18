//! Golden fixtures: every `tests/fixtures/<name>.bin` must match `<name>.json`.
//!
//! The JSON holds `{ "paper", "commands", "receipts", "drawerKicks" }`, plus
//! `"status"`, `"replies"` and `"suppressed"` when they are not empty (see
//! `tests/fixtures/README.md`). Run with
//! `UPDATE_FIXTURES=1` to (re)write the JSON files after reviewing the change.

mod common;

use phantompos_core::PrinterStatus;
use serde_json::{Value, json};

#[test]
fn fixtures_match_expected_json() {
    let update = std::env::var_os("UPDATE_FIXTURES").is_some_and(|v| v != "0" && !v.is_empty());
    let bins = common::bin_fixtures();
    assert!(!bins.is_empty(), "no fixtures found");

    let mut failures = Vec::new();
    for bin in &bins {
        let input = std::fs::read(bin).expect("fixture is readable");
        let paper = common::paper_for(bin);
        let status = common::status_for(bin);
        let run = common::run_whole_in(&input, paper, status);
        let mut actual = json!({
            "paper": paper,
            "commands": run.commands,
            "receipts": run.output.receipts,
            "drawerKicks": run.output.drawer_kicks,
        });
        // Status keys only where they say something, so fixtures of the
        // normal state without replies keep their stage 2-4 shape.
        if status != PrinterStatus::default() {
            actual["status"] = json!(status);
        }
        if !run.replies.is_empty() {
            actual["replies"] = json!(run.replies);
        }
        if let Some(suppressed) = run.suppressed {
            actual["suppressed"] = json!(suppressed);
        }
        let json_path = bin.with_extension("json");
        if update {
            let mut text = serde_json::to_string_pretty(&actual).expect("serializable");
            text.push('\n');
            std::fs::write(&json_path, text).expect("fixture JSON is writable");
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&json_path) else {
            failures.push(format!(
                "{}: missing expected JSON (run with UPDATE_FIXTURES=1)",
                bin.display()
            ));
            continue;
        };
        let expected: Value = serde_json::from_str(&text).expect("fixture JSON parses");
        if expected != actual {
            failures.push(format!(
                "{}: output differs from {} (run with UPDATE_FIXTURES=1 and review the diff)",
                bin.display(),
                json_path.display()
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_json_has_a_bin() {
    let dir = common::fixtures_dir();
    for entry in std::fs::read_dir(&dir).expect("fixtures directory is readable") {
        let path = entry.expect("directory entry").path();
        if path.extension().is_some_and(|ext| ext == "json") {
            assert!(
                path.with_extension("bin").exists(),
                "{} has no matching .bin",
                path.display()
            );
        }
    }
}
