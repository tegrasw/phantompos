//! Barcode encoders for `GS k` (hand-written; the tables are small).
//!
//! Data rules follow the `GS k` page of the Epson reference:
//!
//! * **UPC-A** (11 or 12 digits), **EAN-13** (12 or 13), **EAN-8** (7 or 8):
//!   the check digit is computed when missing; when sent, it is printed as is
//!   and **not checked** (reference). Guard and centre bars are added.
//! * **UPC-E**: 6, 7, 8, 11 or 12 digits. The number system character must be
//!   0 (NSC 0 is added for 6 digits). 11/12 digits are zero-suppressed with
//!   the reference table; data that does not fit the table is invalid. The
//!   check digit is computed (6, 7, 11 digits) or taken as sent (8, 12).
//! * **CODE39**: `0-9 A-Z SP $ % * + - . /`. A leading/trailing `*` is the
//!   start/stop character, otherwise they are added. No check digit.
//!   Deviation: a `*` inside the data makes the barcode invalid (the printer
//!   would end the barcode there and print the rest as text).
//! * **ITF**: digits, at least 2. Function A drops the last digit of an odd
//!   count (reference); function B requires an even `n`. No check digit.
//! * **CODABAR**: `0-9 $ + - . / :` between start/stop characters `A-D` /
//!   `a-d` that the host must send. No check digit.
//! * **CODE93**: bytes 0-127 (full ASCII via the shift characters); the two
//!   check characters and start/stop are added.
//! * **CODE128**: bytes 0-127; the data starts with a code set selection
//!   (`{A`, `{B`, `{C`). Escapes: `{A {B {C` change the code set, `{S` is
//!   SHIFT, `{1`-`{4` are FNC1-FNC4, `{{` is `{` (code set B). In code set C
//!   every byte is one value 0-99 (the reference table). The check character
//!   is added.
//!
//! The output is a list of bar/space runs. Multi-level symbologies (UPC, EAN,
//! CODE93, CODE128) use module counts; binary-level ones (CODE39, ITF,
//! CODABAR) use narrow/wide elements whose widths come from `GS w`.

use crate::command::{BarcodeFunction, BarcodeProblem, Symbology};

/// Width of one bar or space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Width {
    /// `k` modules of a multi-level barcode.
    Modules(u8),
    /// Thin element of a binary-level barcode.
    Narrow,
    /// Thick element of a binary-level barcode.
    Wide,
}

/// An encoded barcode: alternating bar/space runs (the first one is a bar)
/// and the HRI text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Encoded {
    pub runs: Vec<Width>,
    pub hri: String,
}

impl Encoded {
    /// Bar colours of each dot of one row, for module width `module`
    /// (`GS w`, 2-6).
    pub fn row(&self, module: u32) -> Vec<bool> {
        let wide = wide_width(module);
        let mut row = Vec::new();
        for (index, run) in self.runs.iter().enumerate() {
            let dots = match run {
                Width::Modules(k) => module.saturating_mul(u32::from(*k)),
                Width::Narrow => module,
                Width::Wide => wide,
            };
            let black = index % 2 == 0;
            row.extend(std::iter::repeat_n(black, dots as usize));
        }
        row
    }
}

/// Thick element width of binary-level barcodes for `GS w n`: the Epson
/// table gives 0.625 / 1.000 / 1.250 / 1.625 / 2.000 mm at 203 dpi (0.706 /
/// 1.129 / 1.411 / 1.834 / 2.258 mm at 180 dpi), i.e. 5, 8, 10, 13 and 16 dots.
fn wide_width(module: u32) -> u32 {
    match module {
        0..=2 => 5,
        3 => 8,
        4 => 10,
        5 => 13,
        _ => 16,
    }
}

/// Checks `data` against the rules of `symbology` without keeping the result.
pub(crate) fn validate(
    symbology: Symbology,
    function: BarcodeFunction,
    data: &[u8],
) -> Result<(), BarcodeProblem> {
    encode(symbology, function, data).map(|_| ())
}

/// Barcode data for the log: printable ASCII as is, other bytes as `\xNN`.
pub(crate) fn display_data(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len());
    for &b in data {
        match b {
            b'\\' => out.push_str("\\\\"),
            0x20..=0x7E => out.push(char::from(b)),
            _ => out.push_str(&format!("\\x{b:02X}")),
        }
    }
    out
}

/// Encodes a barcode, or says why it cannot be printed.
pub(crate) fn encode(
    symbology: Symbology,
    function: BarcodeFunction,
    data: &[u8],
) -> Result<Encoded, BarcodeProblem> {
    match symbology {
        Symbology::UpcA => upc_a(data),
        Symbology::UpcE => upc_e(data),
        Symbology::Ean13 => ean13(data),
        Symbology::Ean8 => ean8(data),
        Symbology::Code39 => code39(data),
        Symbology::Itf => itf(function, data),
        Symbology::Codabar => codabar(data),
        Symbology::Code93 => code93(data),
        Symbology::Code128 => code128(data),
        Symbology::Gs1128
        | Symbology::Gs1DataBarOmnidirectional
        | Symbology::Gs1DataBarTruncated
        | Symbology::Gs1DataBarLimited
        | Symbology::Gs1DataBarExpanded
        | Symbology::Code128Auto => Err(BarcodeProblem::UnsupportedSymbology),
    }
}

/// Collects runs, merging adjacent ones of the same colour.
#[derive(Default)]
struct Runs(Vec<Width>);

impl Runs {
    /// Appends modules from a bit pattern (`1` = bar), most significant of
    /// `len` bits first.
    fn bits(&mut self, pattern: u32, len: u32) {
        for i in (0..len).rev() {
            self.module((pattern >> i) & 1 == 1);
        }
    }

    /// Appends one module of the given colour.
    fn module(&mut self, black: bool) {
        let next_is_black = self.0.len().is_multiple_of(2);
        if black == next_is_black {
            self.0.push(Width::Modules(1));
        } else if let Some(Width::Modules(k)) = self.0.last_mut() {
            *k = k.saturating_add(1);
        } else {
            // A leading space: cannot happen (every symbol starts with a bar).
            self.0.push(Width::Modules(0));
            self.0.push(Width::Modules(1));
        }
    }

    /// Appends runs given as module counts, starting with a bar.
    fn widths(&mut self, widths: &[u8]) {
        for (i, &w) in widths.iter().enumerate() {
            let black = i % 2 == 0;
            for _ in 0..w {
                self.module(black);
            }
        }
    }

    /// Appends binary-level elements (`b`/`s` narrow, `B`/`S` wide), the
    /// first of which is a bar.
    fn elements(&mut self, pattern: &[u8]) {
        for &e in pattern {
            self.0.push(if e.is_ascii_uppercase() {
                Width::Wide
            } else {
                Width::Narrow
            });
        }
    }

    fn done(self, hri: String) -> Encoded {
        Encoded { runs: self.0, hri }
    }
}

fn digits(data: &[u8]) -> Result<Vec<u8>, BarcodeProblem> {
    data.iter()
        .map(|&b| {
            if b.is_ascii_digit() {
                Ok(b.wrapping_sub(b'0'))
            } else {
                Err(BarcodeProblem::InvalidCharacter)
            }
        })
        .collect()
}

/// Modulo-10 check digit of UPC/EAN: weights 3, 1, 3, ... from the right.
pub(crate) fn check_digit(body: &[u8]) -> u8 {
    let sum: u32 = body
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| u32::from(d).saturating_mul(if i % 2 == 0 { 3 } else { 1 }))
        .sum();
    // sum % 10 is 0-9, so the result is 0-9.
    u8::try_from(10_u32.saturating_sub(sum % 10) % 10).unwrap_or(0)
}

fn hri_digits(digits: &[u8]) -> String {
    digits
        .iter()
        .map(|&d| char::from(b'0'.saturating_add(d)))
        .collect()
}

/// L (odd parity) codes of EAN/UPC digits, 7 modules each.
const EAN_L: [u32; 10] = [
    0b0001101, 0b0011001, 0b0010011, 0b0111101, 0b0100011, 0b0110001, 0b0101111, 0b0111011,
    0b0110111, 0b0001011,
];

/// EAN-13 parity of the left half by first digit (bit set = G code).
const EAN13_PARITY: [u8; 10] = [
    0b000000, 0b001011, 0b001101, 0b001110, 0b010011, 0b011001, 0b011100, 0b010101, 0b010110,
    0b011010,
];

/// UPC-E parity for number system 0, by check digit (bit set = G code).
const UPCE_PARITY: [u8; 10] = [
    0b111000, 0b110100, 0b110010, 0b110001, 0b101100, 0b100110, 0b100011, 0b101010, 0b101001,
    0b100101,
];

fn ean_l(d: u8) -> u32 {
    EAN_L.get(usize::from(d)).copied().unwrap_or(0)
}

fn ean_r(d: u8) -> u32 {
    !ean_l(d) & 0x7F
}

/// G code: the R code read backwards.
fn ean_g(d: u8) -> u32 {
    let r = ean_r(d);
    (0..7).fold(0, |acc, i| (acc << 1) | ((r >> i) & 1))
}

/// Left half digits with a parity mask (bit 5 = first digit), then the right
/// half, between the guard and centre bars of EAN-13 / EAN-8 / UPC-A.
fn ean_symbol(left: &[u8], parity: u8, right: &[u8], hri: String) -> Encoded {
    let mut runs = Runs::default();
    runs.bits(0b101, 3);
    let count = left.len();
    for (i, &d) in left.iter().enumerate() {
        let shift = count.saturating_sub(1).saturating_sub(i);
        let g = (parity >> shift) & 1 == 1;
        runs.bits(if g { ean_g(d) } else { ean_l(d) }, 7);
    }
    runs.bits(0b01010, 5);
    for &d in right {
        runs.bits(ean_r(d), 7);
    }
    runs.bits(0b101, 3);
    runs.done(hri)
}

/// Digits with the check digit appended (`len` without it) or kept as sent
/// (`len + 1`).
fn with_check(data: &[u8], len: usize) -> Result<Vec<u8>, BarcodeProblem> {
    let mut d = digits(data)?;
    if d.len() == len {
        d.push(check_digit(&d));
    } else if d.len() != len.saturating_add(1) {
        return Err(BarcodeProblem::InvalidLength);
    }
    Ok(d)
}

fn length_ok(data: &[u8], lengths: &[usize]) -> Result<(), BarcodeProblem> {
    if lengths.contains(&data.len()) {
        Ok(())
    } else {
        Err(BarcodeProblem::InvalidLength)
    }
}

fn ean13(data: &[u8]) -> Result<Encoded, BarcodeProblem> {
    length_ok(data, &[12, 13])?;
    let d = with_check(data, 12)?;
    let (first, rest) = d.split_first().ok_or(BarcodeProblem::InvalidLength)?;
    let (left, right) = rest.split_at(6.min(rest.len()));
    let parity = EAN13_PARITY.get(usize::from(*first)).copied().unwrap_or(0);
    Ok(ean_symbol(left, parity, right, hri_digits(&d)))
}

fn upc_a(data: &[u8]) -> Result<Encoded, BarcodeProblem> {
    length_ok(data, &[11, 12])?;
    let d = with_check(data, 11)?;
    let (left, right) = d.split_at(6.min(d.len()));
    Ok(ean_symbol(left, 0, right, hri_digits(&d)))
}

fn ean8(data: &[u8]) -> Result<Encoded, BarcodeProblem> {
    length_ok(data, &[7, 8])?;
    let d = with_check(data, 7)?;
    let (left, right) = d.split_at(4.min(d.len()));
    Ok(ean_symbol(left, 0, right, hri_digits(&d)))
}

/// UPC-A body (NSC 0 + 10 digits) of a zero-suppressed UPC-E code.
fn upc_e_expand(short: &[u8]) -> Vec<u8> {
    let g = |i: usize| short.get(i).copied().unwrap_or(0);
    let (d1, d2, d3, d4, d5, d6) = (g(0), g(1), g(2), g(3), g(4), g(5));
    let body: [u8; 10] = match d6 {
        0..=2 => [d1, d2, d6, 0, 0, 0, 0, d3, d4, d5],
        3 => [d1, d2, d3, 0, 0, 0, 0, 0, d4, d5],
        4 => [d1, d2, d3, d4, 0, 0, 0, 0, 0, d5],
        _ => [d1, d2, d3, d4, d5, 0, 0, 0, 0, d6],
    };
    let mut out = vec![0];
    out.extend_from_slice(&body);
    out
}

/// Zero suppression of a UPC-A body (`d2...d11` of the reference table).
fn upc_e_compress(body: &[u8]) -> Result<Vec<u8>, BarcodeProblem> {
    let g = |i: usize| body.get(i).copied().unwrap_or(0);
    // body[0] is d2 of the reference, body[9] is d11.
    let (d2, d3, d4, d5, d6) = (g(0), g(1), g(2), g(3), g(4));
    let (d7, d8, d9, d10, d11) = (g(5), g(6), g(7), g(8), g(9));
    let short = if d4 <= 2 && d5 == 0 && d6 == 0 && d7 == 0 && d8 == 0 {
        [d2, d3, d9, d10, d11, d4]
    } else if d5 == 0 && d6 == 0 && d7 == 0 && d8 == 0 && d9 == 0 {
        [d2, d3, d4, d10, d11, 3]
    } else if d6 == 0 && d7 == 0 && d8 == 0 && d9 == 0 && d10 == 0 {
        [d2, d3, d4, d5, d11, 4]
    } else if d7 == 0 && d8 == 0 && d9 == 0 && d10 == 0 && d11 >= 5 {
        [d2, d3, d4, d5, d6, d11]
    } else {
        return Err(BarcodeProblem::NotCompressible);
    };
    Ok(short.to_vec())
}

fn upc_e(data: &[u8]) -> Result<Encoded, BarcodeProblem> {
    length_ok(data, &[6, 7, 8, 11, 12])?;
    let d = digits(data)?;
    if d.len() != 6 && d.first() != Some(&0) {
        return Err(BarcodeProblem::InvalidNumberSystem);
    }
    let rest = d.get(1..).unwrap_or(&[]);
    let (short, given_check) = match d.len() {
        6 => (d.clone(), None),
        7 => (rest.to_vec(), None),
        8 => (rest.get(..6).unwrap_or(&[]).to_vec(), rest.get(6).copied()),
        11 => (upc_e_compress(rest)?, None),
        _ => (
            upc_e_compress(rest.get(..10).unwrap_or(&[]))?,
            rest.get(10).copied(),
        ),
    };
    let check = given_check.unwrap_or_else(|| check_digit(&upc_e_expand(&short)));
    let parity = UPCE_PARITY.get(usize::from(check)).copied().unwrap_or(0);
    let mut runs = Runs::default();
    runs.bits(0b101, 3);
    for (i, &digit) in short.iter().enumerate() {
        let g = (parity >> 5usize.saturating_sub(i)) & 1 == 1;
        runs.bits(if g { ean_g(digit) } else { ean_l(digit) }, 7);
    }
    runs.bits(0b010101, 6);
    let mut hri = vec![0];
    hri.extend_from_slice(&short);
    hri.push(check);
    Ok(runs.done(hri_digits(&hri)))
}

/// CODE39 characters and their elements (`b`/`s` narrow, `B`/`S` wide).
const CODE39: [(u8, &[u8; 9]); 44] = [
    (b'0', b"bsbSBsBsb"),
    (b'1', b"BsbSbsbsB"),
    (b'2', b"bsBSbsbsB"),
    (b'3', b"BsBSbsbsb"),
    (b'4', b"bsbSBsbsB"),
    (b'5', b"BsbSBsbsb"),
    (b'6', b"bsBSBsbsb"),
    (b'7', b"bsbSbsBsB"),
    (b'8', b"BsbSbsBsb"),
    (b'9', b"bsBSbsBsb"),
    (b'A', b"BsbsbSbsB"),
    (b'B', b"bsBsbSbsB"),
    (b'C', b"BsBsbSbsb"),
    (b'D', b"bsbsBSbsB"),
    (b'E', b"BsbsBSbsb"),
    (b'F', b"bsBsBSbsb"),
    (b'G', b"bsbsbSBsB"),
    (b'H', b"BsbsbSBsb"),
    (b'I', b"bsBsbSBsb"),
    (b'J', b"bsbsBSBsb"),
    (b'K', b"BsbsbsbSB"),
    (b'L', b"bsBsbsbSB"),
    (b'M', b"BsBsbsbSb"),
    (b'N', b"bsbsBsbSB"),
    (b'O', b"BsbsBsbSb"),
    (b'P', b"bsBsBsbSb"),
    (b'Q', b"bsbsbsBSB"),
    (b'R', b"BsbsbsBSb"),
    (b'S', b"bsBsbsBSb"),
    (b'T', b"bsbsBsBSb"),
    (b'U', b"BSbsbsbsB"),
    (b'V', b"bSBsbsbsB"),
    (b'W', b"BSBsbsbsb"),
    (b'X', b"bSbsBsbsB"),
    (b'Y', b"BSbsBsbsb"),
    (b'Z', b"bSBsBsbsb"),
    (b'-', b"bSbsbsBsB"),
    (b'.', b"BSbsbsBsb"),
    (b' ', b"bSBsbsBsb"),
    (b'*', b"bSbsBsBsb"),
    (b'$', b"bSbSbSbsb"),
    (b'/', b"bSbSbsbSb"),
    (b'+', b"bSbsbSbSb"),
    (b'%', b"bsbSbSbSb"),
];

fn code39_pattern(c: u8) -> Option<&'static [u8; 9]> {
    CODE39.iter().find(|(ch, _)| *ch == c).map(|(_, p)| *p)
}

fn code39(data: &[u8]) -> Result<Encoded, BarcodeProblem> {
    if data.is_empty() || data.len() > 255 {
        return Err(BarcodeProblem::InvalidLength);
    }
    let inner = data.strip_prefix(b"*").unwrap_or(data);
    let inner = inner.strip_suffix(b"*").unwrap_or(inner);
    let mut runs = Runs::default();
    let mut symbol = Vec::with_capacity(inner.len().saturating_add(2));
    symbol.push(b'*');
    for &c in inner {
        if c == b'*' || code39_pattern(c).is_none() {
            return Err(BarcodeProblem::InvalidCharacter);
        }
        symbol.push(c);
    }
    symbol.push(b'*');
    for (i, &c) in symbol.iter().enumerate() {
        if i > 0 {
            // Inter-character gap: one narrow space.
            runs.0.push(Width::Narrow);
        }
        runs.elements(code39_pattern(c).map_or(&[][..], |p| &p[..]));
    }
    let hri = symbol.iter().map(|&b| char::from(b)).collect();
    Ok(runs.done(hri))
}

/// ITF digit patterns (5 elements; `n` narrow, `w` wide).
const ITF: [&[u8; 5]; 10] = [
    b"nnwwn", b"wnnnw", b"nwnnw", b"wwnnn", b"nnwnw", b"wnwnn", b"nwwnn", b"nnnww", b"wnnwn",
    b"nwnwn",
];

fn itf(function: BarcodeFunction, data: &[u8]) -> Result<Encoded, BarcodeProblem> {
    let mut d = digits(data)?;
    match function {
        // "When an odd number of data is processed ... the printer ignores
        // the last received data."
        BarcodeFunction::A => {
            if d.len() % 2 == 1 {
                d.pop();
            }
        }
        BarcodeFunction::B => {
            if d.len() % 2 == 1 {
                return Err(BarcodeProblem::InvalidLength);
            }
        }
    }
    if d.len() < 2 {
        return Err(BarcodeProblem::InvalidLength);
    }
    let mut runs = Runs::default();
    runs.elements(b"bsbs");
    for &[a, b] in d.as_chunks::<2>().0 {
        let bars = ITF.get(usize::from(a)).map_or(&[][..], |p| &p[..]);
        let spaces = ITF.get(usize::from(b)).map_or(&[][..], |p| &p[..]);
        for (bar, space) in bars.iter().zip(spaces) {
            runs.0.push(if *bar == b'w' {
                Width::Wide
            } else {
                Width::Narrow
            });
            runs.0.push(if *space == b'w' {
                Width::Wide
            } else {
                Width::Narrow
            });
        }
    }
    runs.elements(b"Bsb");
    Ok(runs.done(hri_digits(&d)))
}

/// CODABAR characters (7 elements each).
const CODABAR: [(u8, &[u8; 7]); 20] = [
    (b'0', b"bsbsbSB"),
    (b'1', b"bsbsBSb"),
    (b'2', b"bsbSbsB"),
    (b'3', b"BSbsbsb"),
    (b'4', b"bsBsbSb"),
    (b'5', b"BsbsbSb"),
    (b'6', b"bSbsbsB"),
    (b'7', b"bSbsBsb"),
    (b'8', b"bSBsbsb"),
    (b'9', b"BsbSbsb"),
    (b'-', b"bsbSBsb"),
    (b'$', b"bsBSbsb"),
    (b':', b"BsbsBsB"),
    (b'/', b"BsBsbsB"),
    (b'.', b"BsBsBsb"),
    (b'+', b"bsBsBsB"),
    (b'A', b"bsBSbSb"),
    (b'B', b"bSbSbsB"),
    (b'C', b"bsbSbSB"),
    (b'D', b"bsbSBSb"),
];

fn codabar_pattern(c: u8) -> Option<&'static [u8; 7]> {
    let c = c.to_ascii_uppercase();
    CODABAR.iter().find(|(ch, _)| *ch == c).map(|(_, p)| *p)
}

fn is_codabar_start_stop(c: u8) -> bool {
    matches!(c, b'A'..=b'D' | b'a'..=b'd')
}

fn codabar(data: &[u8]) -> Result<Encoded, BarcodeProblem> {
    if data.len() < 2 || data.len() > 255 {
        return Err(BarcodeProblem::InvalidLength);
    }
    let (Some(&first), Some(&last)) = (data.first(), data.last()) else {
        return Err(BarcodeProblem::InvalidLength);
    };
    if !is_codabar_start_stop(first) || !is_codabar_start_stop(last) {
        return Err(BarcodeProblem::InvalidStartStop);
    }
    let inner = data.get(1..data.len().saturating_sub(1)).unwrap_or(&[]);
    for &c in inner {
        if is_codabar_start_stop(c) {
            return Err(BarcodeProblem::InvalidStartStop);
        }
        if codabar_pattern(c).is_none() {
            return Err(BarcodeProblem::InvalidCharacter);
        }
    }
    let mut runs = Runs::default();
    for (i, &c) in data.iter().enumerate() {
        if i > 0 {
            runs.0.push(Width::Narrow);
        }
        runs.elements(codabar_pattern(c).map_or(&[][..], |p| &p[..]));
    }
    let hri = data.iter().map(|&b| char::from(b)).collect();
    Ok(runs.done(hri))
}

/// CODE93 character values 0-46 as bar/space module widths (6 runs, 9
/// modules). 0-9, A-Z, `-`, `.`, SP, `$`, `/`, `+`, `%`, then the shift
/// characters ($) (%) (/) (+) at 43-46.
const CODE93: [[u8; 6]; 47] = [
    [1, 3, 1, 1, 1, 2],
    [1, 1, 1, 2, 1, 3],
    [1, 1, 1, 3, 1, 2],
    [1, 1, 1, 4, 1, 1],
    [1, 2, 1, 1, 1, 3],
    [1, 2, 1, 2, 1, 2],
    [1, 2, 1, 3, 1, 1],
    [1, 1, 1, 1, 1, 4],
    [1, 3, 1, 2, 1, 1],
    [1, 4, 1, 1, 1, 1],
    [2, 1, 1, 1, 1, 3],
    [2, 1, 1, 2, 1, 2],
    [2, 1, 1, 3, 1, 1],
    [2, 2, 1, 1, 1, 2],
    [2, 2, 1, 2, 1, 1],
    [2, 3, 1, 1, 1, 1],
    [1, 1, 2, 1, 1, 3],
    [1, 1, 2, 2, 1, 2],
    [1, 1, 2, 3, 1, 1],
    [1, 2, 2, 1, 1, 2],
    [1, 3, 2, 1, 1, 1],
    [1, 1, 1, 1, 2, 3],
    [1, 1, 1, 2, 2, 2],
    [1, 1, 1, 3, 2, 1],
    [1, 2, 1, 1, 2, 2],
    [1, 3, 1, 1, 2, 1],
    [2, 1, 2, 1, 1, 2],
    [2, 1, 2, 2, 1, 1],
    [2, 1, 1, 1, 2, 2],
    [2, 1, 1, 2, 2, 1],
    [2, 2, 1, 1, 2, 1],
    [2, 2, 2, 1, 1, 1],
    [1, 1, 2, 1, 2, 2],
    [1, 1, 2, 2, 2, 1],
    [1, 2, 2, 1, 2, 1],
    [1, 2, 3, 1, 1, 1],
    [1, 2, 1, 1, 3, 1],
    [3, 1, 1, 1, 1, 2],
    [3, 1, 1, 2, 1, 1],
    [3, 2, 1, 1, 1, 1],
    [1, 1, 2, 1, 3, 1],
    [1, 1, 3, 1, 2, 1],
    [2, 1, 1, 1, 3, 1],
    [1, 2, 1, 2, 2, 1],
    [3, 1, 2, 1, 1, 1],
    [3, 1, 1, 1, 2, 1],
    [1, 2, 2, 2, 1, 1],
];

const CODE93_START_STOP: [u8; 6] = [1, 1, 1, 1, 4, 1];
const CODE93_BASIC: &[u8; 43] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ-. $/+%";
/// Shift characters ($) (%) (/) (+).
const C93_DOLLAR: u8 = 43;
const C93_PERCENT: u8 = 44;
const C93_SLASH: u8 = 45;
const C93_PLUS: u8 = 46;

fn code93_basic(c: u8) -> Option<u8> {
    CODE93_BASIC
        .iter()
        .position(|&b| b == c)
        .and_then(|p| u8::try_from(p).ok())
}

/// Value of the letter `c` (A-Z) in CODE93.
fn c93_letter(c: u8) -> u8 {
    10_u8.saturating_add(c.saturating_sub(b'A'))
}

/// Full-ASCII CODE93 values of one byte (0-127).
fn code93_values(c: u8) -> Option<Vec<u8>> {
    if let Some(v) = code93_basic(c) {
        return Some(vec![v]);
    }
    let pair = |shift: u8, letter: u8| Some(vec![shift, c93_letter(letter)]);
    match c {
        0 => pair(C93_PERCENT, b'U'),
        1..=26 => pair(C93_DOLLAR, b'A'.saturating_add(c.saturating_sub(1))),
        27..=31 => pair(C93_PERCENT, b'A'.saturating_add(c.saturating_sub(27))),
        b'!'..=b',' => pair(C93_SLASH, b'A'.saturating_add(c.saturating_sub(b'!'))),
        b':' => pair(C93_SLASH, b'Z'),
        b';'..=b'?' => pair(C93_PERCENT, b'F'.saturating_add(c.saturating_sub(b';'))),
        b'@' => pair(C93_PERCENT, b'V'),
        b'['..=b'_' => pair(C93_PERCENT, b'K'.saturating_add(c.saturating_sub(b'['))),
        b'`' => pair(C93_PERCENT, b'W'),
        b'a'..=b'z' => pair(C93_PLUS, b'A'.saturating_add(c.saturating_sub(b'a'))),
        b'{'..=0x7F => pair(C93_PERCENT, b'P'.saturating_add(c.saturating_sub(b'{'))),
        _ => None,
    }
}

/// CODE93 check character over `values` with weights 1..=`max` from the
/// right.
fn code93_check(values: &[u8], max: usize) -> u8 {
    let sum: usize = values
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &v)| {
            usize::from(v).saturating_mul(i.checked_rem(max).unwrap_or(0).saturating_add(1))
        })
        .sum();
    u8::try_from(sum % 47).unwrap_or(0)
}

fn printable_hri(c: u8) -> char {
    if (0x20..0x7F).contains(&c) {
        char::from(c)
    } else {
        ' '
    }
}

fn code93(data: &[u8]) -> Result<Encoded, BarcodeProblem> {
    if data.is_empty() || data.len() > 255 {
        return Err(BarcodeProblem::InvalidLength);
    }
    let mut values = Vec::new();
    for &c in data {
        values.extend(code93_values(c).ok_or(BarcodeProblem::InvalidCharacter)?);
    }
    let c = code93_check(&values, 20);
    values.push(c);
    let k = code93_check(&values, 15);
    values.push(k);
    let mut runs = Runs::default();
    runs.widths(&CODE93_START_STOP);
    for v in &values {
        runs.widths(CODE93.get(usize::from(*v)).map_or(&[][..], |w| &w[..]));
    }
    runs.widths(&CODE93_START_STOP);
    // Termination bar.
    runs.widths(&[1]);
    // Deviation: the start/stop squares and the shift marks the reference
    // shows in the HRI are not drawn; control characters print as spaces.
    let hri = data.iter().map(|&b| printable_hri(b)).collect();
    Ok(runs.done(hri))
}

/// CODE128 symbol values 0-105 as module widths (bar, space, ...).
const CODE128: [[u8; 6]; 106] = [
    [2, 1, 2, 2, 2, 2],
    [2, 2, 2, 1, 2, 2],
    [2, 2, 2, 2, 2, 1],
    [1, 2, 1, 2, 2, 3],
    [1, 2, 1, 3, 2, 2],
    [1, 3, 1, 2, 2, 2],
    [1, 2, 2, 2, 1, 3],
    [1, 2, 2, 3, 1, 2],
    [1, 3, 2, 2, 1, 2],
    [2, 2, 1, 2, 1, 3],
    [2, 2, 1, 3, 1, 2],
    [2, 3, 1, 2, 1, 2],
    [1, 1, 2, 2, 3, 2],
    [1, 2, 2, 1, 3, 2],
    [1, 2, 2, 2, 3, 1],
    [1, 1, 3, 2, 2, 2],
    [1, 2, 3, 1, 2, 2],
    [1, 2, 3, 2, 2, 1],
    [2, 2, 3, 2, 1, 1],
    [2, 2, 1, 1, 3, 2],
    [2, 2, 1, 2, 3, 1],
    [2, 1, 3, 2, 1, 2],
    [2, 2, 3, 1, 1, 2],
    [3, 1, 2, 1, 3, 1],
    [3, 1, 1, 2, 2, 2],
    [3, 2, 1, 1, 2, 2],
    [3, 2, 1, 2, 2, 1],
    [3, 1, 2, 2, 1, 2],
    [3, 2, 2, 1, 1, 2],
    [3, 2, 2, 2, 1, 1],
    [2, 1, 2, 1, 2, 3],
    [2, 1, 2, 3, 2, 1],
    [2, 3, 2, 1, 2, 1],
    [1, 1, 1, 3, 2, 3],
    [1, 3, 1, 1, 2, 3],
    [1, 3, 1, 3, 2, 1],
    [1, 1, 2, 3, 1, 3],
    [1, 3, 2, 1, 1, 3],
    [1, 3, 2, 3, 1, 1],
    [2, 1, 1, 3, 1, 3],
    [2, 3, 1, 1, 1, 3],
    [2, 3, 1, 3, 1, 1],
    [1, 1, 2, 1, 3, 3],
    [1, 1, 2, 3, 3, 1],
    [1, 3, 2, 1, 3, 1],
    [1, 1, 3, 1, 2, 3],
    [1, 1, 3, 3, 2, 1],
    [1, 3, 3, 1, 2, 1],
    [3, 1, 3, 1, 2, 1],
    [2, 1, 1, 3, 3, 1],
    [2, 3, 1, 1, 3, 1],
    [2, 1, 3, 1, 1, 3],
    [2, 1, 3, 3, 1, 1],
    [2, 1, 3, 1, 3, 1],
    [3, 1, 1, 1, 2, 3],
    [3, 1, 1, 3, 2, 1],
    [3, 3, 1, 1, 2, 1],
    [3, 1, 2, 1, 1, 3],
    [3, 1, 2, 3, 1, 1],
    [3, 3, 2, 1, 1, 1],
    [3, 1, 4, 1, 1, 1],
    [2, 2, 1, 4, 1, 1],
    [4, 3, 1, 1, 1, 1],
    [1, 1, 1, 2, 2, 4],
    [1, 1, 1, 4, 2, 2],
    [1, 2, 1, 1, 2, 4],
    [1, 2, 1, 4, 2, 1],
    [1, 4, 1, 1, 2, 2],
    [1, 4, 1, 2, 2, 1],
    [1, 1, 2, 2, 1, 4],
    [1, 1, 2, 4, 1, 2],
    [1, 2, 2, 1, 1, 4],
    [1, 2, 2, 4, 1, 1],
    [1, 4, 2, 1, 1, 2],
    [1, 4, 2, 2, 1, 1],
    [2, 4, 1, 2, 1, 1],
    [2, 2, 1, 1, 1, 4],
    [4, 1, 3, 1, 1, 1],
    [2, 4, 1, 1, 1, 2],
    [1, 3, 4, 1, 1, 1],
    [1, 1, 1, 2, 4, 2],
    [1, 2, 1, 1, 4, 2],
    [1, 2, 1, 2, 4, 1],
    [1, 1, 4, 2, 1, 2],
    [1, 2, 4, 1, 1, 2],
    [1, 2, 4, 2, 1, 1],
    [4, 1, 1, 2, 1, 2],
    [4, 2, 1, 1, 1, 2],
    [4, 2, 1, 2, 1, 1],
    [2, 1, 2, 1, 4, 1],
    [2, 1, 4, 1, 2, 1],
    [4, 1, 2, 1, 2, 1],
    [1, 1, 1, 1, 4, 3],
    [1, 1, 1, 3, 4, 1],
    [1, 3, 1, 1, 4, 1],
    [1, 1, 4, 1, 1, 3],
    [1, 1, 4, 3, 1, 1],
    [4, 1, 1, 1, 1, 3],
    [4, 1, 1, 3, 1, 1],
    [1, 1, 3, 1, 4, 1],
    [1, 1, 4, 1, 3, 1],
    [3, 1, 1, 1, 4, 1],
    [4, 1, 1, 1, 3, 1],
    [2, 1, 1, 4, 1, 2],
    [2, 1, 1, 2, 1, 4],
    [2, 1, 1, 2, 3, 2],
];

const CODE128_STOP: [u8; 7] = [2, 3, 3, 1, 1, 1, 2];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodeSet {
    A,
    B,
    C,
}

/// Value of a data byte in code set A (00h-5Fh) or B (20h-7Fh).
fn code128_value(set: CodeSet, c: u8) -> Option<u8> {
    match (set, c) {
        (CodeSet::A, 0x00..=0x1F) => Some(c.saturating_add(64)),
        (CodeSet::A, 0x20..=0x5F) | (CodeSet::B, 0x20..=0x7F) => Some(c.saturating_sub(0x20)),
        _ => None,
    }
}

fn code128(data: &[u8]) -> Result<Encoded, BarcodeProblem> {
    if data.len() < 2 || data.len() > 255 {
        return Err(BarcodeProblem::InvalidLength);
    }
    let mut set = match (data.first(), data.get(1)) {
        (Some(b'{'), Some(b'A')) => CodeSet::A,
        (Some(b'{'), Some(b'B')) => CodeSet::B,
        (Some(b'{'), Some(b'C')) => CodeSet::C,
        _ => return Err(BarcodeProblem::InvalidCharacter),
    };
    let start = match set {
        CodeSet::A => 103,
        CodeSet::B => 104,
        CodeSet::C => 105,
    };
    let mut values: Vec<u8> = vec![start];
    let mut hri = String::new();
    let mut bytes = data.get(2..).unwrap_or(&[]).iter().copied();
    let invalid = BarcodeProblem::InvalidCharacter;
    while let Some(c) = bytes.next() {
        if c == b'{' {
            let escape = bytes.next().ok_or(invalid)?;
            let value = match (escape, set) {
                (b'A', CodeSet::B | CodeSet::C) => {
                    set = CodeSet::A;
                    101
                }
                (b'B', CodeSet::A | CodeSet::C) => {
                    set = CodeSet::B;
                    100
                }
                (b'C', CodeSet::A | CodeSet::B) => {
                    set = CodeSet::C;
                    99
                }
                (b'S', CodeSet::A | CodeSet::B) => {
                    // SHIFT: the next byte is in the other of A/B.
                    let other = if set == CodeSet::A {
                        CodeSet::B
                    } else {
                        CodeSet::A
                    };
                    let next = bytes.next().ok_or(invalid)?;
                    let shifted = code128_value(other, next).ok_or(invalid)?;
                    values.push(98);
                    hri.push(printable_hri(next));
                    shifted
                }
                (b'1', _) => {
                    hri.push(' ');
                    102
                }
                (b'2', CodeSet::A | CodeSet::B) => {
                    hri.push(' ');
                    97
                }
                (b'3', CodeSet::A | CodeSet::B) => {
                    hri.push(' ');
                    96
                }
                (b'4', CodeSet::A) => {
                    hri.push(' ');
                    101
                }
                (b'4', CodeSet::B) => {
                    hri.push(' ');
                    100
                }
                (b'{', CodeSet::B) => {
                    hri.push('{');
                    91
                }
                _ => return Err(invalid),
            };
            values.push(value);
            continue;
        }
        let value = match set {
            CodeSet::C => {
                if c > 99 {
                    return Err(invalid);
                }
                hri.push_str(&format!("{c:02}"));
                c
            }
            CodeSet::A | CodeSet::B => {
                hri.push(printable_hri(c));
                code128_value(set, c).ok_or(invalid)?
            }
        };
        values.push(value);
    }
    if values.len() < 2 {
        return Err(BarcodeProblem::InvalidLength);
    }
    let sum = values
        .iter()
        .enumerate()
        .map(|(i, &v)| usize::from(v).saturating_mul(i.max(1)))
        .fold(0_usize, usize::saturating_add);
    values.push(u8::try_from(sum % 103).unwrap_or(0));
    let mut runs = Runs::default();
    for v in &values {
        runs.widths(CODE128.get(usize::from(*v)).map_or(&[][..], |w| &w[..]));
    }
    runs.widths(&CODE128_STOP);
    Ok(runs.done(hri))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Modules of a multi-level barcode as a `1`/`0` string.
    fn modules(e: &Encoded) -> String {
        e.row(1)
            .into_iter()
            .map(|b| if b { '1' } else { '0' })
            .collect()
    }

    #[test]
    fn check_digits() {
        assert_eq!(check_digit(&[4, 0, 0, 6, 3, 8, 1, 3, 3, 3, 9, 3]), 1);
        assert_eq!(check_digit(&[7, 8, 9, 1, 2, 3, 4, 5, 6, 7, 8, 9]), 5);
        assert_eq!(check_digit(&[9, 6, 3, 8, 5, 0, 7]), 4); // EAN-8 96385074
        assert_eq!(check_digit(&[0, 3, 6, 0, 0, 0, 2, 9, 1, 4, 5]), 2); // UPC-A
    }

    #[test]
    fn ean13_reference_pattern() {
        // 4006381333931: first digit 4 -> parity LGLLGG.
        let e = encode(Symbology::Ean13, BarcodeFunction::B, b"400638133393").unwrap();
        assert_eq!(e.hri, "4006381333931");
        let expected = concat!(
            "101", "0001101", // 0 L
            "0100111", // 0 G
            "0101111", // 6 L
            "0111101", // 3 L
            "0001001", // 8 G
            "0110011", // 1 G
            "01010", "1000010", // 3 R
            "1000010", // 3 R
            "1000010", // 3 R
            "1110100", // 9 R
            "1000010", // 3 R
            "1100110", // 1 R
            "101"
        );
        assert_eq!(modules(&e), expected);
        assert_eq!(modules(&e).len(), 95);
    }

    #[test]
    fn ean13_check_digit_is_not_checked() {
        let e = encode(Symbology::Ean13, BarcodeFunction::A, b"4006381333930").unwrap();
        assert_eq!(e.hri, "4006381333930");
    }

    #[test]
    fn upc_e_compression_and_expansion() {
        // 01234565 is the UPC-E form of UPC-A 012345000065.
        let from_a = encode(Symbology::UpcE, BarcodeFunction::A, b"01234500006").unwrap();
        let from_e = encode(Symbology::UpcE, BarcodeFunction::A, b"123456").unwrap();
        assert_eq!(from_a.hri, "01234565");
        assert_eq!(from_e.hri, "01234565");
        assert_eq!(from_a, from_e);
        assert_eq!(modules(&from_e).len(), 51);
        assert_eq!(
            encode(Symbology::UpcE, BarcodeFunction::A, b"01234567890"),
            Err(BarcodeProblem::NotCompressible)
        );
        assert_eq!(
            encode(Symbology::UpcE, BarcodeFunction::A, b"1234567"),
            Err(BarcodeProblem::InvalidNumberSystem)
        );
    }

    #[test]
    fn code128_checksum_examples() {
        // "PJJ123C" in code set B: 104 + 48x1 + 42x2 + 42x3 + 17x4 + 18x5 +
        // 19x6 + 35x7 = 879, and 879 % 103 = 55.
        let e = encode(Symbology::Code128, BarcodeFunction::B, b"{BPJJ123C").unwrap();
        let m = modules(&e);
        // Start B, 7 data, check, stop: 9 x 11 + 13 modules.
        assert_eq!(m.len(), 9 * 11 + 13);
        let check = &m[8 * 11..9 * 11];
        let expected: String = CODE128[55]
            .iter()
            .enumerate()
            .flat_map(|(i, &w)| std::iter::repeat_n(if i % 2 == 0 { '1' } else { '0' }, w.into()))
            .collect();
        assert_eq!(check, expected);
        assert_eq!(e.hri, "PJJ123C");
        // Code set C takes binary values 0-99.
        let c = encode(Symbology::Code128, BarcodeFunction::B, b"{C\x0c\x22\x38").unwrap();
        assert_eq!(c.hri, "123456");
        assert_eq!(
            encode(Symbology::Code128, BarcodeFunction::B, b"{C\x64"),
            Err(BarcodeProblem::InvalidCharacter)
        );
        assert_eq!(
            encode(Symbology::Code128, BarcodeFunction::B, b"AB"),
            Err(BarcodeProblem::InvalidCharacter)
        );
    }

    #[test]
    fn code128_table_widths() {
        for w in CODE128 {
            assert_eq!(w.iter().map(|&x| u32::from(x)).sum::<u32>(), 11);
        }
        for w in CODE93 {
            assert_eq!(w.iter().map(|&x| u32::from(x)).sum::<u32>(), 9);
        }
    }

    #[test]
    fn code93_checks() {
        // "TEST93" (T=29 E=14 S=28 T=29 9=9 3=3), weights 6..1 from the left:
        // sum C = 29*6 + 14*5 + 28*4 + 29*3 + 9*2 + 3*1 = 464 -> 464 % 47 = 41.
        let mut values = vec![29, 14, 28, 29, 9, 3];
        let c = code93_check(&values, 20);
        assert_eq!(c, 41);
        values.push(c);
        // K = (29*7 + 14*6 + 28*5 + 29*4 + 9*3 + 3*2 + 41*1) % 47 = 617 % 47 = 6.
        assert_eq!(code93_check(&values, 15), 6);
        let e = encode(Symbology::Code93, BarcodeFunction::B, b"TEST93").unwrap();
        // start + 6 + C + K + stop = 10 x 9 modules, + termination bar.
        assert_eq!(modules(&e).len(), 10 * 9 + 1);
        // Lowercase uses the (+) shift: two values per character.
        let lower = encode(Symbology::Code93, BarcodeFunction::B, b"a").unwrap();
        assert_eq!(modules(&lower).len(), 6 * 9 + 1);
    }

    #[test]
    fn binary_level_rules() {
        assert!(encode(Symbology::Code39, BarcodeFunction::A, b"ABC-12").is_ok());
        assert_eq!(
            encode(Symbology::Code39, BarcodeFunction::A, b"AB*C"),
            Err(BarcodeProblem::InvalidCharacter)
        );
        assert_eq!(
            encode(Symbology::Code39, BarcodeFunction::A, b"abc"),
            Err(BarcodeProblem::InvalidCharacter)
        );
        let itf = encode(Symbology::Itf, BarcodeFunction::A, b"12345").unwrap();
        assert_eq!(itf.hri, "1234");
        assert_eq!(
            encode(Symbology::Itf, BarcodeFunction::B, b"12345"),
            Err(BarcodeProblem::InvalidLength)
        );
        assert!(encode(Symbology::Codabar, BarcodeFunction::A, b"A40156B").is_ok());
        assert_eq!(
            encode(Symbology::Codabar, BarcodeFunction::A, b"40156"),
            Err(BarcodeProblem::InvalidStartStop)
        );
        assert_eq!(
            encode(Symbology::Gs1128, BarcodeFunction::B, b"{A1"),
            Err(BarcodeProblem::UnsupportedSymbology)
        );
    }

    #[test]
    fn display_escapes_non_printable() {
        assert_eq!(display_data(b"{C\x0c\\"), "{C\\x0C\\\\");
    }
}
