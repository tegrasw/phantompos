//! The reply table in `core/README.md` is generated from the code: this test
//! fails when they differ, and `UPDATE_FIXTURES=1` rewrites the table.

use phantompos_core::{Command, PrinterStatus, RealTimeStatusKind, automatic_status, status_reply};

const START: &str = "<!-- status-table:start -->";
const END: &str = "<!-- status-table:end -->";

fn states() -> Vec<(&'static str, PrinterStatus)> {
    let normal = PrinterStatus::default();
    vec![
        ("Normal", normal),
        (
            "Drawer open",
            PrinterStatus {
                drawer_open: true,
                ..normal
            },
        ),
        (
            "Paper out",
            PrinterStatus {
                paper_out: true,
                ..normal
            },
        ),
        (
            "Cover open",
            PrinterStatus {
                cover_open: true,
                ..normal
            },
        ),
        (
            "Offline",
            PrinterStatus {
                offline: true,
                ..normal
            },
        ),
        (
            "Cover open + paper out",
            PrinterStatus {
                cover_open: true,
                paper_out: true,
                ..normal
            },
        ),
        (
            "All four",
            PrinterStatus {
                paper_out: true,
                cover_open: true,
                offline: true,
                drawer_open: true,
            },
        ),
    ]
}

fn eot(n: u8, status: RealTimeStatusKind) -> Command {
    Command::RealTimeStatus {
        n,
        a: None,
        status: Some(status),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn table() -> String {
    let requests: Vec<(&str, &str, Option<Command>)> = vec![
        (
            "`DLE EOT 1` printer",
            "dle_eot.html",
            Some(eot(1, RealTimeStatusKind::Printer)),
        ),
        (
            "`DLE EOT 2` offline cause",
            "dle_eot.html",
            Some(eot(2, RealTimeStatusKind::OfflineCause)),
        ),
        (
            "`DLE EOT 3` error cause",
            "dle_eot.html",
            Some(eot(3, RealTimeStatusKind::ErrorCause)),
        ),
        (
            "`DLE EOT 4` roll paper sensor",
            "dle_eot.html",
            Some(eot(4, RealTimeStatusKind::RollPaperSensor)),
        ),
        (
            "`GS r 1` / `GS r 49` paper sensor",
            "gs_lr.html",
            Some(Command::TransmitStatus { n: 1 }),
        ),
        (
            "`GS r 2` / `GS r 50` drawer connector",
            "gs_lr.html",
            Some(Command::TransmitStatus { n: 2 }),
        ),
        (
            "ASB (`GS a`), `DLE DC4 7 1`",
            "gs_la.html, dle_dc4_fn7.html",
            None,
        ),
    ];
    let states = states();
    let mut out = String::new();
    out.push_str("| Request | Epson page |");
    for (name, _) in &states {
        out.push_str(&format!(" {name} |"));
    }
    out.push_str("\n| --- | --- |");
    for _ in &states {
        out.push_str(" --- |");
    }
    out.push('\n');
    for (label, page, command) in requests {
        out.push_str(&format!("| {label} | `{page}` |"));
        for (_, status) in &states {
            let bytes = match &command {
                Some(command) => {
                    status_reply(command, status)
                        .expect("every row is answered")
                        .bytes
                }
                None => automatic_status(status).to_vec(),
            };
            out.push_str(&format!(" `{}` |", hex(&bytes)));
        }
        out.push('\n');
    }
    out
}

#[test]
fn readme_reply_table_matches_the_code() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md");
    let readme = std::fs::read_to_string(&path).expect("README is readable");
    let start = readme.find(START).expect("table start marker") + START.len();
    let end = readme.find(END).expect("table end marker");
    let expected = format!("\n{}", table());
    if std::env::var_os("UPDATE_FIXTURES").is_some_and(|v| v != "0" && !v.is_empty()) {
        let updated = format!("{}{}{}", &readme[..start], expected, &readme[end..]);
        std::fs::write(&path, updated).expect("README is writable");
        return;
    }
    assert_eq!(
        &readme[start..end],
        expected,
        "README reply table is stale (run with UPDATE_FIXTURES=1)"
    );
}
