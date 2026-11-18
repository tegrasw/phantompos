//! Random input never panics, loses or duplicates bytes, and is independent of
//! how it is split.

mod common;

use common::Rng;
use phantompos_core::{
    Command, DecodedCommand, MAX_PAYLOAD, PaperProfile, Parser, ParserConfig, ReceiptBuilder,
    SKIP_CHUNK,
};

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

/// Random binary-payload commands (images, QR Codes, barcodes, graphics)
/// with random declared lengths, mixed with text and line feeds.
fn binary_biased(rng: &mut Rng) -> Vec<u8> {
    let mut out = Vec::new();
    let count = 1 + rng.below(12);
    for _ in 0..count {
        let small = |rng: &mut Rng| rng.below(6) as u8;
        match rng.below(14) {
            0 => {
                // GS v 0 with a small (sometimes wrong) payload.
                let (w, h) = (rng.below(4) as u8, rng.below(6) as u8);
                out.extend_from_slice(&[0x1d, b'v', b'0', rng.below(5) as u8, w, 0, h, 0]);
                let len = usize::from(w) * usize::from(h);
                let len = if rng.below(4) == 0 {
                    rng.below(len + 3)
                } else {
                    len
                };
                out.extend((0..len).map(|_| rng.byte()));
            }
            1 => {
                // GS v 0 declaring a random (often huge) size.
                out.extend_from_slice(&[0x1d, b'v', b'0', 0]);
                out.extend((0..4).map(|_| rng.byte()));
            }
            2 => {
                // QR store with random data, then print.
                let len = rng.below(120);
                let total = len + 3;
                out.extend_from_slice(&[0x1d, b'(', b'k', total as u8, 0, 49, 80, 48]);
                out.extend((0..len).map(|_| rng.byte()));
                out.extend_from_slice(b"\x1d(k\x03\x001Q0");
            }
            3 => {
                // QR setting with random parameter and length.
                let pl = rng.below(5) as u8;
                out.extend_from_slice(&[
                    0x1d,
                    b'(',
                    b'k',
                    pl,
                    0,
                    49,
                    [65, 67, 69, 81, 82][rng.below(5)],
                ]);
                out.extend((0..pl.saturating_sub(2)).map(|_| 48 + small(rng)));
            }
            4 => {
                // GS ( k / GS ( L / ESC ( / FS ( with a random length.
                let name: &[u8] =
                    [&b"\x1d(k"[..], b"\x1d(L", b"\x1b(A", b"\x1c(C", b"\x1d(E"][rng.below(5)];
                out.extend_from_slice(name);
                out.push(rng.byte());
                out.push(if rng.below(4) == 0 { rng.byte() } else { 0 });
            }
            5 => {
                // GS 8 L with a random 4-byte length.
                out.extend_from_slice(b"\x1d8L");
                out.extend((0..4).map(|_| if rng.below(2) == 0 { rng.byte() } else { 0 }));
            }
            6 => {
                // ESC * / GS * with random sizes.
                if rng.below(2) == 0 {
                    out.extend_from_slice(&[
                        0x1b,
                        b'*',
                        [0, 1, 32, 33, 7][rng.below(5)],
                        rng.byte(),
                        0,
                    ]);
                } else {
                    out.extend_from_slice(&[0x1d, b'*', small(rng), small(rng)]);
                }
            }
            7 => {
                // GS k function A with digits (often valid) and a NUL.
                out.extend_from_slice(&[0x1d, b'k', rng.below(7) as u8]);
                let len = rng.below(16);
                out.extend((0..len).map(|_| b'0' + rng.below(10) as u8));
                if rng.below(8) != 0 {
                    out.push(0);
                }
            }
            8 => {
                // GS k function B with random bytes.
                let len = rng.below(20) as u8;
                out.extend_from_slice(&[0x1d, b'k', 65 + rng.below(16) as u8, len]);
                out.extend((0..len).map(|_| rng.byte() & 0x7f));
            }
            9 => {
                // CODE128 with escapes.
                let body: Vec<u8> = (0..rng.below(12))
                    .map(|_| [b'{', b'A', b'B', b'C', b'S', b'1', b'4', b'x', 5, 99][rng.below(10)])
                    .collect();
                out.extend_from_slice(&[
                    0x1d,
                    b'k',
                    73,
                    (body.len() + 2) as u8,
                    b'{',
                    b"ABC"[rng.below(3)],
                ]);
                out.extend_from_slice(&body);
            }
            10 => {
                // Barcode / QR settings.
                let code = b"hwHf"[rng.below(4)];
                out.extend_from_slice(&[0x1d, code, rng.byte()]);
            }
            11 => out.extend_from_slice(&[0x1b, b'a', small(rng)]),
            12 => out.extend_from_slice(b"text "),
            _ => out.push(b'\n'),
        }
    }
    if rng.below(5) == 0 {
        // Cut the stream anywhere.
        let len = rng.below(out.len() + 1);
        out.truncate(len);
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

#[test]
fn binary_biased_input_never_panics() {
    let mut rng = Rng::new(0x5EED_0004);
    for _ in 0..ITERATIONS {
        let input = binary_biased(&mut rng);
        check(&input, &mut rng);
    }
}

#[test]
fn byte_by_byte_equals_whole_on_binary_input() {
    let mut rng = Rng::new(0x5EED_0005);
    for _ in 0..300 {
        let input = binary_biased(&mut rng);
        let whole = common::run_whole(&input, PaperProfile::Mm58);
        let single = common::run_chunked(&input, &mut || 1, PaperProfile::Mm58);
        assert_eq!(whole, single);
    }
}

/// Every receipt element stays inside the paper and the layout is
/// consistent: elements are contiguous and the height is their sum.
#[test]
fn image_elements_are_consistent() {
    let mut rng = Rng::new(0x5EED_0006);
    for _ in 0..ITERATIONS {
        let input = binary_biased(&mut rng);
        for paper in [PaperProfile::Mm58, PaperProfile::Mm80] {
            let (_, output) = common::run_whole(&input, paper);
            for receipt in &output.receipts {
                let mut y = 0;
                for element in &receipt.elements {
                    match element {
                        phantompos_core::Element::Line(line) => {
                            assert_eq!(line.y, y);
                            y += line.advance;
                        }
                        phantompos_core::Element::Image(image) => {
                            assert_eq!(image.y, y);
                            assert!(image.x + image.width <= receipt.width);
                            assert_eq!(image.advance, image.height);
                            let expected =
                                (image.width as usize).div_ceil(8) * image.height as usize;
                            assert_eq!(image.data.len(), expected.div_ceil(3) * 4);
                            y += image.advance;
                        }
                    }
                }
                assert_eq!(receipt.height, y);
            }
        }
    }
}

/// A huge declared image is consumed in bounded chunks; memory held by the
/// parser never exceeds the payload cap, and the result does not depend on
/// how the stream is split.
#[test]
fn oversized_payloads_use_bounded_memory() {
    // 3 MiB declared (2048 x 1536 bytes), all of it sent, then text.
    let mut input = b"\x1dv0\x00\x00\x08\x00\x06".to_vec();
    input.extend((0..3 * 1024 * 1024).map(|i| (i % 251) as u8));
    input.extend_from_slice(b"after\n\x1d8L\xff\xff\xff\x7f");
    input.extend(std::iter::repeat_n(0xAA, 200_000));

    let mut rng = Rng::new(0x5EED_0007);
    let mut parser = Parser::new(ParserConfig::default());
    let mut builder = ReceiptBuilder::new(common::receipt_config(PaperProfile::Mm80));
    let mut commands = Vec::new();
    let mut rest = &input[..];
    while !rest.is_empty() {
        let len = (1 + rng.below(70_000)).min(rest.len());
        let (chunk, tail) = rest.split_at(len);
        let batch = parser.feed(chunk);
        assert!(parser.pending_bytes() <= MAX_PAYLOAD as usize + 16);
        builder.apply_all(&batch);
        commands.extend(batch);
        rest = tail;
    }
    commands.extend(parser.finish());
    assert_lossless(&input, &commands);
    assert!(commands.iter().all(|c| c.bytes.len() <= SKIP_CHUNK));
    let skipped = commands
        .iter()
        .filter(|c| matches!(c.command, Command::SkippedData { .. }))
        .count();
    assert_eq!(skipped, 3 * 16 + 200_000 / SKIP_CHUNK);
    // The image is not drawn; the text after it is.
    let receipt = builder.finish().expect("text was printed");
    assert_eq!(receipt.elements.len(), 1);
    assert_eq!(common::run_whole(&input, PaperProfile::Mm80).0, commands);
}
