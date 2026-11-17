//! Golden fixtures: every `tests/fixtures/<name>.bin` must match `<name>.json`.
//!
//! The JSON holds `{ "paper", "commands", "receipts", "drawerKicks" }`. Run with
//! `UPDATE_FIXTURES=1` to (re)write the JSON files after reviewing the change.

mod common;

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
        let (commands, output) = common::run_whole(&input, paper);
        let actual = json!({
            "paper": paper,
            "commands": commands,
            "receipts": output.receipts,
            "drawerKicks": output.drawer_kicks,
        });
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
