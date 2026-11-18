//! Helpers shared by the integration tests.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use phantompos_core::{
    DecodedCommand, PaperProfile, ParserConfig, PrinterStatus, ReceiptConfig, RenderOutput, Reply,
    Session, SessionEvent, SessionOutput, StatusReply, SuppressReason,
};
use serde::Serialize;

/// Directory holding the `.bin` / `.json` pairs.
pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

/// Every `.bin` fixture, sorted by name.
pub fn bin_fixtures() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(fixtures_dir())
        .expect("fixtures directory is readable")
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "bin"))
        .collect();
    paths.sort();
    paths
}

/// Paper convention: `<name>.58mm.bin` renders on 58 mm, anything else on 80 mm.
pub fn paper_for(bin: &Path) -> PaperProfile {
    if stem_tags(bin).iter().any(|tag| tag == "58mm") {
        PaperProfile::Mm58
    } else {
        PaperProfile::Mm80
    }
}

/// Dot-separated tags after the fixture name (`name.58mm.paperOut` gives
/// `["58mm", "paperOut"]`).
fn stem_tags(bin: &Path) -> Vec<String> {
    let stem = bin.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    stem.split('.').skip(1).map(str::to_owned).collect()
}

/// State convention: a tag made of state names joined by `+`
/// (`name.coverOpen+paperOut.bin`) sets those states; no tag is the normal
/// state. Unknown names panic, so a typo cannot silently test nothing.
pub fn status_for(bin: &Path) -> PrinterStatus {
    let mut status = PrinterStatus::default();
    for tag in stem_tags(bin) {
        if tag == "58mm" {
            continue;
        }
        for name in tag.split('+') {
            match name {
                "paperOut" => status.paper_out = true,
                "coverOpen" => status.cover_open = true,
                "offline" => status.offline = true,
                "drawerOpen" => status.drawer_open = true,
                other => panic!("{}: unknown fixture tag {other:?}", bin.display()),
            }
        }
    }
    status
}

pub fn receipt_config(paper: PaperProfile) -> ReceiptConfig {
    ReceiptConfig {
        paper,
        cr_line_feed: false,
    }
}

/// A reply as recorded in the fixture JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordedReply {
    pub request: String,
    #[serde(flatten)]
    pub reply: StatusReply,
}

/// Discarded data, summed over the whole stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordedSuppressed {
    pub reason: SuppressReason,
    pub bytes: u64,
}

/// Everything a stream produced.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Run {
    pub commands: Vec<DecodedCommand>,
    pub output: RenderOutput,
    pub replies: Vec<RecordedReply>,
    pub suppressed: Option<RecordedSuppressed>,
}

impl Run {
    fn record(&mut self, out: SessionOutput, status: &mut PrinterStatus) {
        for event in out.events {
            match event {
                SessionEvent::ReceiptCompleted(receipt) => {
                    self.output.receipts.push(receipt);
                }
                SessionEvent::DrawerKick(kick) => {
                    // As the backend does: the shared state follows the kick.
                    status.drawer_open = true;
                    self.output.drawer_kicks.push(kick);
                }
                SessionEvent::Reply(Reply { request, reply }) => {
                    self.replies.push(RecordedReply { request, reply });
                }
            }
        }
        if let Some(suppressed) = out.suppressed {
            let bytes = self.suppressed.map_or(0, |s| s.bytes) + suppressed.bytes;
            self.suppressed = Some(RecordedSuppressed {
                reason: suppressed.reason,
                bytes,
            });
        }
        self.commands.extend(out.commands);
    }
}

/// Feeds `input` split into the given chunk lengths (cycled) to a
/// [`Session`] in state `status`, as the backend does per socket read. The
/// in-progress snapshot is taken after every chunk (as the backend does) to
/// check that it never disturbs the result.
pub fn run_chunked_in(
    input: &[u8],
    chunks: &mut dyn FnMut() -> usize,
    paper: PaperProfile,
    status: PrinterStatus,
) -> Run {
    let mut status = status;
    let mut session = Session::new(ParserConfig::default(), receipt_config(paper), status);
    let mut run = Run::default();
    let mut rest = input;
    while !rest.is_empty() {
        let len = chunks().clamp(1, rest.len());
        let (chunk, tail) = rest.split_at(len);
        let out = session.feed(chunk, status);
        run.record(out, &mut status);
        if let Some(update) = session.take_update() {
            assert!(update.cut.is_none() && update.height > 0, "bad update");
        }
        rest = tail;
    }
    let out = session.finish(status);
    run.record(out, &mut status);
    run
}

/// [`run_chunked_in`] in the normal state.
pub fn run_chunked(input: &[u8], chunks: &mut dyn FnMut() -> usize, paper: PaperProfile) -> Run {
    run_chunked_in(input, chunks, paper, PrinterStatus::default())
}

/// Whole input in a single `feed`.
pub fn run_whole_in(input: &[u8], paper: PaperProfile, status: PrinterStatus) -> Run {
    let len = input.len().max(1);
    run_chunked_in(input, &mut || len, paper, status)
}

/// Whole input in a single `feed`, normal state.
pub fn run_whole(input: &[u8], paper: PaperProfile) -> Run {
    run_whole_in(input, paper, PrinterStatus::default())
}

/// Small deterministic PRNG (xorshift64*), so tests need no extra crate.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform-ish value in `0..bound` (`bound > 0`).
    pub fn below(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }

    pub fn byte(&mut self) -> u8 {
        (self.next_u64() >> 56) as u8
    }
}
