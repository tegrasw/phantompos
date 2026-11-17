//! Random input never panics, loses or duplicates bytes, and is independent of
//! how it is split.

mod common;

use common::Rng;
use phantompos_core::{DecodedCommand, PaperProfile};

const ITERATIONS: usize = 3000;

fn random_bytes(rng: &mut Rng) -> Vec<u8> {
    let len = rng.below(600);
    (0..len).map(|_| rng.byte()).collect()
}

/// Random mixes of command prefixes, known command bytes, small parameters,
/// line feeds and printable text, to reach deep parser states.
fn escpos_biased(rng: &mut Rng) -> Vec<u8> {
    const PIECES: &[&[u8]] = &[
        b"\x1b",
        b"\x1d",
        b"\x10",
        b"\x1c",
        b"\n",
        b"\r",
        b"\t",
        b"\x00",
        b"@",
        b"d",
        b"J",
        b"2",
        b"3",
        b"E",
        b"-",
        b"B",
        b"M",
        b"!",
        b"a",
        b"t",
        b"R",
        b"V",
        b"i",
        b"m",
        b"p",
        b"A",
        b"B",
        b"b",
        b"g",
        b"h",
        b"(",
        b"k",
        b"v0",
        b"0",
        b"1",
        b"2",
        b"\x80",
        b"\xff",
        b"\x87",
        b"Hello ",
        b"x",
        b"\xc6\xa6",
    ];
    let count = rng.below(200);
    let mut out = Vec::new();
    for _ in 0..count {
        if rng.below(4) == 0 {
            out.push(rng.byte());
        } else {
            out.extend_from_slice(PIECES[rng.below(PIECES.len())]);
        }
    }
    out
}

fn assert_lossless(input: &[u8], commands: &[DecodedCommand]) {
    let mut offset = 0u64;
    let mut joined = Vec::with_capacity(input.len());
    for item in commands {
        assert_eq!(item.offset, offset, "offset mismatch");
        assert!(!item.bytes.is_empty(), "empty envelope");
        offset += item.bytes.len() as u64;
        joined.extend_from_slice(&item.bytes);
    }
    assert_eq!(joined, input, "bytes lost or duplicated");
}

fn check(input: &[u8], rng: &mut Rng) {
    for paper in [PaperProfile::Mm58, PaperProfile::Mm80] {
        let whole = common::run_whole(input, paper);
        assert_lossless(input, &whole.0);
        let fragmented = common::run_chunked(input, &mut || 1 + rng.below(9), paper);
        assert_eq!(whole, fragmented, "fragmentation changed the result");
        // Receipts are never empty, and only the last one may lack a cut.
        let receipts = &whole.1.receipts;
        for (i, receipt) in receipts.iter().enumerate() {
            assert!(receipt.height > 0 && !receipt.elements.is_empty());
            assert!(receipt.cut.is_some() || i + 1 == receipts.len());
        }
        // Both outputs must serialize.
        serde_json::to_string(&whole.0).expect("commands serialize");
        serde_json::to_string(&whole.1).expect("receipts serialize");
    }
}

#[test]
fn random_bytes_never_panic() {
    let mut rng = Rng::new(0x5EED_0001);
    for _ in 0..ITERATIONS {
        let input = random_bytes(&mut rng);
        check(&input, &mut rng);
    }
}

#[test]
fn escpos_biased_input_never_panics() {
    let mut rng = Rng::new(0x5EED_0002);
    for _ in 0..ITERATIONS {
        let input = escpos_biased(&mut rng);
        check(&input, &mut rng);
    }
}

#[test]
fn byte_by_byte_equals_whole_on_biased_input() {
    let mut rng = Rng::new(0x5EED_0003);
    for _ in 0..300 {
        let input = escpos_biased(&mut rng);
        let whole = common::run_whole(&input, PaperProfile::Mm80);
        let single = common::run_chunked(&input, &mut || 1, PaperProfile::Mm80);
        assert_eq!(whole, single);
    }
}
