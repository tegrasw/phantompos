//! Helpers shared by the integration tests.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use phantompos_core::{
    DecodedCommand, PaperProfile, Parser, ParserConfig, Receipt, ReceiptBuilder, ReceiptConfig,
};

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
    let stem = bin.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    if stem.ends_with(".58mm") {
        PaperProfile::Mm58
    } else {
        PaperProfile::Mm80
    }
}

pub fn receipt_config(paper: PaperProfile) -> ReceiptConfig {
    ReceiptConfig {
        paper,
        cr_line_feed: false,
    }
}

/// Feeds `input` split into the given chunk lengths (cycled), building the
/// receipt incrementally, as the backend does per socket read.
pub fn run_chunked(
    input: &[u8],
    chunks: &mut dyn FnMut() -> usize,
    paper: PaperProfile,
) -> (Vec<DecodedCommand>, Receipt) {
    let mut parser = Parser::new(ParserConfig::default());
    let mut builder = ReceiptBuilder::new(receipt_config(paper));
    let mut commands = Vec::new();
    let mut rest = input;
    while !rest.is_empty() {
        let len = chunks().clamp(1, rest.len());
        let (chunk, tail) = rest.split_at(len);
        let batch = parser.feed(chunk);
        builder.apply_all(&batch);
        commands.extend(batch);
        rest = tail;
    }
    let batch = parser.finish();
    builder.apply_all(&batch);
    commands.extend(batch);
    (commands, builder.finish())
}

/// Whole input in a single `feed`.
pub fn run_whole(input: &[u8], paper: PaperProfile) -> (Vec<DecodedCommand>, Receipt) {
    let len = input.len().max(1);
    run_chunked(input, &mut || len, paper)
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
