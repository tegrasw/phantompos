//! Incremental ESC/POS parser.
//!
//! The parser is a byte-at-a-time state machine, so input may be split at any
//! point: feeding a stream in one call or byte by byte yields the same list of
//! [`DecodedCommand`]s. Two rules make that hold:
//!
//! * a run of printable bytes is emitted as one `text` command only when a
//!   non-printable byte arrives, when the run reaches [`MAX_TEXT_RUN`] bytes, or
//!   on [`Parser::finish`] — never because a `feed` call ended;
//! * an unfinished command at the end of a `feed` call stays pending until more
//!   bytes or [`Parser::finish`] arrive.
//!
//! Commands are described by a lookup table ([`lookup`]): adding a command in a
//! later stage means adding a table entry (and, for variable-length commands
//! such as `GS ( k` or `GS v 0`, a new [`ParamLength`] variant).
//!
//! Commands with a binary payload (`GS v 0`, `GS ( k`, `GS ( L`, `GS 8 L`,
//! `ESC *`, `GS *`, `GS k`) are framed by the lengths they declare, so their
//! data never leaks into the receipt as text. They are emitted once complete;
//! an unfinished one at [`Parser::finish`] becomes `unknown` / `incomplete`
//! with its bytes. Every length comes from untrusted input, so memory is
//! bounded: a payload larger than [`MAX_PAYLOAD`] is not stored. Its command
//! is emitted with the header only, as soon as the header is complete, and
//! the payload is consumed as `skippedData` items of at most
//! [`SKIP_CHUNK`] bytes (the stream stays in sync and lossless).
//!
//! Deliberate deviations: `GS V` with an `m` outside the Epson table is taken
//! as three bytes (`GS V m`) and logged as a `cut` with `null` fields; the
//! real behaviour is undefined in the reference. `GS k` with an `m` outside
//! both function tables is taken as three bytes (`GS k m`). `GS k` function A
//! reads data up to the NUL; a NUL missing within 255 data bytes ends the
//! command after the 256th byte (the reference sets no limit; this keeps a
//! lost NUL from swallowing the rest of the stream). `ESC *` with an `m`
//! outside 0, 1, 32 and 33 is taken as its 5-byte header (the data size is
//! undefined). `GS v` followed by anything but `'0'` is taken as three bytes
//! and logged as `unknown`.

use serde::{Deserialize, Serialize};

use crate::barcode;
use crate::codepage::CodePage;
use crate::command::{
    Alignment, BarcodeFunction, Command, CutFunction, CutMode, DecodedCommand, Font, HriPosition,
    QrCodeModel, QrErrorCorrection, RasterMode, Symbology, UnderlineMode, UnknownReason,
};

const HT: u8 = 0x09;
const LF: u8 = 0x0A;
const CR: u8 = 0x0D;
const DLE: u8 = 0x10;
const ESC: u8 = 0x1B;
const FS: u8 = 0x1C;
const GS: u8 = 0x1D;

/// Longest `text` command emitted; longer runs are split deterministically.
pub const MAX_TEXT_RUN: usize = 4096;

/// Largest payload (image or symbol data) kept in memory and drawn: 1 MiB,
/// e.g. a 576-dot-wide raster image about 14 500 dots (1.8 m) tall. Larger
/// payloads are consumed without being stored (see the module docs).
pub const MAX_PAYLOAD: u64 = 1 << 20;

/// Size of the `skippedData` items an oversized payload is split into.
pub const SKIP_CHUNK: usize = 1 << 16;

/// Longest function A barcode data (`GS k m d1...dk NUL`) before the command
/// is ended without its NUL.
const MAX_BARCODE_A_DATA: usize = 255;

/// Per-connection parser settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParserConfig {
    /// Code page in effect at power on and after `ESC @` (the "code page
    /// padrão" setting).
    pub default_code_page: CodePage,
}

/// How many parameter bytes follow the command byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParamLength {
    /// A fixed number of bytes.
    Fixed(usize),
    /// `GS V`: one byte `m`, plus `n` for functions B, C and D.
    Cut,
    /// `pL pH` (2 bytes) or `p1 p2 p3 p4` (4 bytes), little endian, then
    /// that many bytes (`GS ( k`, `GS ( L`, `GS 8 L`).
    LengthPrefixed(usize),
    /// `GS v`: `'0' m xL xH yL yH`, then `(xL + xH x 256) x (yL + yH x 256)`
    /// bytes.
    Raster,
    /// `ESC *`: `m nL nH`, then `n` bytes (8-dot modes) or `3n` (24-dot).
    BitImage,
    /// `GS *`: `x y`, then `x x y x 8` bytes.
    DownloadedBitImage,
    /// `GS k`: function A up to NUL, function B `n` and `n` bytes.
    Barcode,
}

/// Where a command stands after one more parameter byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Progress {
    NeedMore,
    Done,
    /// The header is complete and declares a payload larger than
    /// [`MAX_PAYLOAD`]: emit the header now, then skip `payload` bytes.
    Oversized {
        payload: u64,
    },
}

/// Header and payload sizes of a length-framed command.
enum Frame {
    /// The header is not complete yet.
    Partial,
    /// The parameters do not describe a payload; the command ends here.
    Bare,
    Sized {
        header: usize,
        payload: u64,
    },
}

/// Little-endian value of `bytes` (at most 4 of them).
fn le(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .rev()
        .fold(0_u64, |acc, &b| acc.saturating_mul(256) | u64::from(b))
}

impl ParamLength {
    fn frame(self, params: &[u8]) -> Frame {
        let header = |len: usize| params.get(..len);
        match self {
            Self::LengthPrefixed(size) => match header(size) {
                Some(h) => Frame::Sized {
                    header: size,
                    payload: le(h),
                },
                None => Frame::Partial,
            },
            Self::Raster => {
                if params.first().is_some_and(|&b| b != b'0') {
                    return Frame::Bare;
                }
                match header(6) {
                    Some(h) => Frame::Sized {
                        header: 6,
                        payload: le(h.get(2..4).unwrap_or(&[]))
                            .saturating_mul(le(h.get(4..6).unwrap_or(&[]))),
                    },
                    None => Frame::Partial,
                }
            }
            Self::BitImage => match header(3) {
                Some(h) => {
                    let n = le(h.get(1..3).unwrap_or(&[]));
                    let payload = match h.first() {
                        Some(0 | 1) => n,
                        Some(32 | 33) => n.saturating_mul(3),
                        _ => 0,
                    };
                    Frame::Sized { header: 3, payload }
                }
                None => Frame::Partial,
            },
            Self::DownloadedBitImage => match header(2) {
                Some(h) => Frame::Sized {
                    header: 2,
                    payload: h
                        .iter()
                        .fold(8_u64, |acc, &b| acc.saturating_mul(u64::from(b))),
                },
                None => Frame::Partial,
            },
            Self::Fixed(_) | Self::Cut | Self::Barcode => Frame::Bare,
        }
    }

    /// Whether the command is complete, given the parameters read so far.
    fn progress(self, params: &[u8]) -> Progress {
        let done_if = |complete: bool| {
            if complete {
                Progress::Done
            } else {
                Progress::NeedMore
            }
        };
        match self {
            Self::Fixed(count) => done_if(params.len() >= count),
            Self::Cut => {
                let required = match params.first() {
                    Some(&m) if cut_takes_feed(m) => 2,
                    _ => 1,
                };
                done_if(params.len() >= required)
            }
            Self::Barcode => match params.first() {
                None => Progress::NeedMore,
                Some(&m) if barcode_function(m) == Some(BarcodeFunction::A) => {
                    let data = params.get(1..).unwrap_or(&[]);
                    done_if(data.last() == Some(&0) || data.len() > MAX_BARCODE_A_DATA)
                }
                Some(&m) if barcode_function(m) == Some(BarcodeFunction::B) => {
                    match params.get(1) {
                        None => Progress::NeedMore,
                        Some(&n) => done_if(params.len() >= usize::from(n).saturating_add(2)),
                    }
                }
                Some(_) => Progress::Done,
            },
            Self::LengthPrefixed(_) | Self::Raster | Self::BitImage | Self::DownloadedBitImage => {
                match self.frame(params) {
                    Frame::Partial => Progress::NeedMore,
                    Frame::Bare => Progress::Done,
                    Frame::Sized { header, payload } => {
                        if payload > MAX_PAYLOAD {
                            if params.len() >= header {
                                Progress::Oversized { payload }
                            } else {
                                Progress::NeedMore
                            }
                        } else {
                            // payload <= MAX_PAYLOAD, so it fits in usize.
                            let total = usize::try_from(payload)
                                .map_or(usize::MAX, |p| p.saturating_add(header));
                            done_if(params.len() >= total)
                        }
                    }
                }
            }
        }
    }
}

/// Table entry describing one recognized command.
#[derive(Debug, Clone, Copy)]
struct CommandSpec {
    params: ParamLength,
    /// Builds the command from its parameter bytes (exactly `params` long,
    /// or only the header for an oversized payload).
    decode: fn(&[u8]) -> Command,
    /// Bytes before the parameters: prefix + command byte (2), or prefix +
    /// two name bytes (3) for `GS ( k` and the like.
    name_len: usize,
}

const fn spec(params: usize, decode: fn(&[u8]) -> Command) -> CommandSpec {
    CommandSpec {
        params: ParamLength::Fixed(params),
        decode,
        name_len: 2,
    }
}

/// First parameter byte, or 0 if missing (cannot happen for fixed lengths).
fn n(params: &[u8]) -> u8 {
    params.first().copied().unwrap_or(0)
}

/// The command table, keyed by (prefix, command byte).
fn lookup(prefix: u8, code: u8) -> Option<CommandSpec> {
    let found = match (prefix, code) {
        (ESC, b'@') => spec(0, |_| Command::Initialize),
        (ESC, b'd') => spec(1, |p| Command::FeedLines { lines: n(p) }),
        (ESC, b'J') => spec(1, |p| Command::FeedDots { dots: n(p) }),
        (ESC, b'2') => spec(0, |_| Command::ResetLineSpacing),
        (ESC, b'3') => spec(1, |p| Command::SetLineSpacing { dots: n(p) }),
        (ESC, b'E') => spec(1, |p| Command::SetBold {
            enabled: n(p) & 1 == 1,
        }),
        (ESC, b'-') => spec(1, |p| Command::SetUnderline {
            mode: decode_underline(n(p)),
        }),
        (GS, b'B') => spec(1, |p| Command::SetReverse {
            enabled: n(p) & 1 == 1,
        }),
        (ESC, b'M') => spec(1, |p| Command::SetFont {
            font: decode_font(n(p)),
        }),
        (GS, b'!') => spec(1, |p| decode_character_size(n(p))),
        (ESC, b'!') => spec(1, |p| decode_print_mode(n(p))),
        (ESC, b'a') => spec(1, |p| Command::SetAlignment {
            alignment: decode_alignment(n(p)),
        }),
        (ESC, b't') => spec(1, |p| Command::SetCodePage {
            page: n(p),
            code_page: CodePage::from_esc_t(n(p)),
        }),
        (ESC, b'R') => spec(1, |p| Command::SetInternationalCharset { n: n(p) }),
        (GS, b'V') => framed(ParamLength::Cut, decode_cut),
        // Obsolete cuts; both are partial cuts in the Epson reference ("one
        // point left uncut" / "three points left uncut") and equal to GS V
        // function A.
        (ESC, b'i') | (ESC, b'm') => spec(0, |_| Command::Cut {
            mode: Some(CutMode::Partial),
            function: Some(CutFunction::A),
            feed: None,
        }),
        (ESC, b'p') => spec(3, decode_drawer_kick),
        (GS, b'v') => framed(ParamLength::Raster, decode_raster),
        (ESC, b'*') => framed(ParamLength::BitImage, |p| {
            let m = n(p);
            unsupported(
                format!("ESC * m={m}"),
                payload_len(ParamLength::BitImage, p),
            )
        }),
        (GS, b'*') => framed(ParamLength::DownloadedBitImage, |p| {
            unsupported(
                "GS *".into(),
                payload_len(ParamLength::DownloadedBitImage, p),
            )
        }),
        (GS, b'/') => spec(1, |p| unsupported(format!("GS / m={}", n(p)), 0)),
        (FS, b'p') => spec(2, |p| {
            let m = p.get(1).copied().unwrap_or(0);
            unsupported(format!("FS p n={} m={m}", n(p)), 0)
        }),
        (GS, b'h') => spec(1, |p| Command::SetBarcodeHeight {
            dots: (n(p) > 0).then(|| n(p)),
        }),
        (GS, b'w') => spec(1, |p| Command::SetBarcodeWidth {
            module: (2..=6).contains(&n(p)).then(|| n(p)),
        }),
        (GS, b'H') => spec(1, |p| Command::SetHriPosition {
            position: decode_hri_position(n(p)),
        }),
        (GS, b'f') => spec(1, |p| Command::SetHriFont {
            font: decode_font(n(p)),
        }),
        (GS, b'k') => framed(ParamLength::Barcode, decode_barcode),
        _ => return None,
    };
    Some(found)
}

/// Command bytes that start a three-byte command name (`GS ( k`, `GS 8 L`,
/// ...); the entry is chosen by [`lookup_long`] once the third byte arrives.
fn is_long_name(prefix: u8, code: u8) -> bool {
    matches!((prefix, code), (GS | ESC | FS, b'(') | (GS, b'8'))
}

/// Entries for three-byte command names. Every `GS (`, `ESC (` and `FS (`
/// command of the reference is framed by `pL pH`, so the ones not
/// implemented are still consumed whole (as `unknown`) instead of leaking
/// their parameters as text. `GS 8` is only defined for `GS 8 L`.
fn lookup_long(prefix: u8, code: u8, third: u8) -> Option<CommandSpec> {
    let found = match (prefix, code, third) {
        (GS, b'(', b'k') => framed(ParamLength::LengthPrefixed(2), decode_symbol),
        (GS, b'(', b'L') => framed(ParamLength::LengthPrefixed(2), |p| {
            decode_graphics("GS ( L", p, 2)
        }),
        (GS, b'8', b'L') => framed(ParamLength::LengthPrefixed(4), |p| {
            decode_graphics("GS 8 L", p, 4)
        }),
        (GS | ESC | FS, b'(', _) => framed(ParamLength::LengthPrefixed(2), |_| Command::Unknown {
            reason: UnknownReason::UnsupportedCommand,
        }),
        _ => return None,
    };
    Some(CommandSpec {
        name_len: 3,
        ..found
    })
}

const fn framed(params: ParamLength, decode: fn(&[u8]) -> Command) -> CommandSpec {
    CommandSpec {
        params,
        decode,
        name_len: 2,
    }
}

fn unsupported(command: String, length: u64) -> Command {
    Command::UnsupportedImage {
        command,
        length: u32::try_from(length).unwrap_or(u32::MAX),
    }
}

/// Number of payload bytes the header in `params` declares, for logging.
fn payload_len(kind: ParamLength, params: &[u8]) -> u64 {
    match kind.frame(params) {
        Frame::Sized { payload, .. } => payload,
        Frame::Partial | Frame::Bare => 0,
    }
}

/// `GS ( L` / `GS 8 L pL pH m fn ...`: graphics. Recognized by length only.
fn decode_graphics(name: &str, params: &[u8], size: usize) -> Command {
    let length = le(params.get(..size).unwrap_or(&[]));
    let label = match params.get(size.saturating_add(1)) {
        Some(function) => format!("{name} fn={function}"),
        None => name.to_owned(),
    };
    unsupported(label, length)
}

/// `GS ( k pL pH cn fn ...`: QR Code functions are decoded; the other
/// symbols are recognized by length only.
fn decode_symbol(params: &[u8]) -> Command {
    let length = le(params.get(..2).unwrap_or(&[]));
    let body = params.get(2..).unwrap_or(&[]);
    let (Some(&cn), Some(&function)) = (body.first(), body.get(1)) else {
        return unsupported("GS ( k".into(), length);
    };
    let args = body.get(2..).unwrap_or(&[]);
    let name = match cn {
        48 => "PDF417",
        49 => "QR Code",
        50 => "MaxiCode",
        51 => "GS1 DataBar",
        52 => "Composite Symbology",
        53 => "Aztec Code",
        54 => "DataMatrix",
        _ => "",
    };
    let fallback = || {
        let label = if name.is_empty() {
            format!("GS ( k cn={cn} fn={function}")
        } else {
            format!("GS ( k {name} fn={function}")
        };
        unsupported(label, length)
    };
    if cn != 49 {
        return fallback();
    }
    match (function, args) {
        (65, &[n1, n2]) => Command::SetQrModel {
            model: match (n1, n2) {
                (49, 0) => Some(QrCodeModel::Model1),
                (50, 0) => Some(QrCodeModel::Model2),
                (51, 0) => Some(QrCodeModel::Micro),
                _ => None,
            },
        },
        (67, &[size]) => Command::SetQrModuleSize {
            size: (1..=16).contains(&size).then_some(size),
        },
        (69, &[level]) => Command::SetQrErrorCorrection {
            level: match level {
                48 => Some(QrErrorCorrection::L),
                49 => Some(QrErrorCorrection::M),
                50 => Some(QrErrorCorrection::Q),
                51 => Some(QrErrorCorrection::H),
                _ => None,
            },
        },
        (80, [m, data @ ..]) => {
            let valid = *m == 48 && (4..=7092).contains(&length);
            Command::QrStoreData {
                length: u32::try_from(data.len()).unwrap_or(u32::MAX),
                valid,
                data: if valid { data.to_vec() } else { Vec::new() },
            }
        }
        (81, &[48]) => Command::QrPrint,
        (82, &[48]) => Command::QrRequestSize,
        _ => fallback(),
    }
}

/// `GS v 0 m xL xH yL yH d1...dk`.
fn decode_raster(params: &[u8]) -> Command {
    let (Some(b'0'), Some(header)) = (params.first(), params.get(1..6)) else {
        return Command::Unknown {
            reason: UnknownReason::UnsupportedCommand,
        };
    };
    let m = header.first().copied().unwrap_or(0);
    let width_bytes = le(header.get(1..3).unwrap_or(&[]));
    let height = le(header.get(3..5).unwrap_or(&[]));
    let to_u32 = |v: u64| u32::try_from(v).unwrap_or(u32::MAX);
    Command::RasterImage {
        mode: match m {
            0 | 48 => Some(RasterMode::Normal),
            1 | 49 => Some(RasterMode::DoubleWidth),
            2 | 50 => Some(RasterMode::DoubleHeight),
            3 | 51 => Some(RasterMode::Quadruple),
            _ => None,
        },
        width: to_u32(width_bytes.saturating_mul(8)),
        height: to_u32(height),
        bytes_expected: to_u32(width_bytes.saturating_mul(height)),
        data: params.get(6..).unwrap_or(&[]).to_vec(),
    }
}

/// `GS k m`: the function of `m`, or `None` when it is in neither table.
fn barcode_function(m: u8) -> Option<BarcodeFunction> {
    match m {
        0..=6 => Some(BarcodeFunction::A),
        65..=79 => Some(BarcodeFunction::B),
        _ => None,
    }
}

/// Barcode system of `GS k m` (function A 0-6, function B 65-79).
fn symbology(m: u8) -> Option<Symbology> {
    let index = if m >= 65 { m.saturating_sub(65) } else { m };
    Some(match index {
        0 => Symbology::UpcA,
        1 => Symbology::UpcE,
        2 => Symbology::Ean13,
        3 => Symbology::Ean8,
        4 => Symbology::Code39,
        5 => Symbology::Itf,
        6 => Symbology::Codabar,
        7 if m >= 65 => Symbology::Code93,
        8 if m >= 65 => Symbology::Code128,
        9 if m >= 65 => Symbology::Gs1128,
        10 if m >= 65 => Symbology::Gs1DataBarOmnidirectional,
        11 if m >= 65 => Symbology::Gs1DataBarTruncated,
        12 if m >= 65 => Symbology::Gs1DataBarLimited,
        13 if m >= 65 => Symbology::Gs1DataBarExpanded,
        14 if m >= 65 => Symbology::Code128Auto,
        _ => return None,
    })
}

/// `GS k m d1...dk NUL` (function A) or `GS k m n d1...dn` (function B).
fn decode_barcode(params: &[u8]) -> Command {
    let m = n(params);
    let function = barcode_function(m);
    let (raw, terminated) = match function {
        Some(BarcodeFunction::A) => {
            let data = params.get(1..).unwrap_or(&[]);
            match data.split_last() {
                Some((0, body)) => (body, true),
                _ => (data, false),
            }
        }
        Some(BarcodeFunction::B) => (params.get(2..).unwrap_or(&[]), true),
        None => (&[][..], true),
    };
    let symbology = function.and(symbology(m));
    let problem = match (symbology, function) {
        (Some(symbology), Some(function)) if terminated => {
            barcode::validate(symbology, function, raw).err()
        }
        (Some(_), _) => Some(crate::command::BarcodeProblem::MissingTerminator),
        _ => Some(crate::command::BarcodeProblem::UnknownSymbology),
    };
    Command::Barcode {
        symbology,
        function,
        data: barcode::display_data(raw),
        valid: problem.is_none(),
        problem,
        raw: raw.to_vec(),
    }
}

/// `GS H n`: 0/48 none, 1/49 above, 2/50 below, 3/51 both.
fn decode_hri_position(n: u8) -> Option<HriPosition> {
    match n {
        0 | 48 => Some(HriPosition::None),
        1 | 49 => Some(HriPosition::Above),
        2 | 50 => Some(HriPosition::Below),
        3 | 51 => Some(HriPosition::Both),
        _ => None,
    }
}

/// `GS V m`: functions B (65, 66), C (97, 98) and D (103, 104) take `n`.
fn cut_takes_feed(m: u8) -> bool {
    matches!(m, 65 | 66 | 97 | 98 | 103 | 104)
}

/// `GS V m [n]`: even `m` is a full cut, odd `m` a partial cut.
fn decode_cut(params: &[u8]) -> Command {
    let m = n(params);
    let feed = params.get(1).copied();
    let (mode, function) = match m {
        0 | 48 => (CutMode::Full, CutFunction::A),
        1 | 49 => (CutMode::Partial, CutFunction::A),
        65 => (CutMode::Full, CutFunction::B),
        66 => (CutMode::Partial, CutFunction::B),
        97 => (CutMode::Full, CutFunction::C),
        98 => (CutMode::Partial, CutFunction::C),
        103 => (CutMode::Full, CutFunction::D),
        104 => (CutMode::Partial, CutFunction::D),
        _ => {
            return Command::Cut {
                mode: None,
                function: None,
                feed: None,
            };
        }
    };
    Command::Cut {
        mode: Some(mode),
        function: Some(function),
        feed,
    }
}

/// `ESC p m t1 t2`: m = 0/48 pin 2, 1/49 pin 5; ON time t1 x 2 ms, OFF time
/// t2 x 2 ms, and "if t2 < t1, the OFF time is equal to the ON time".
fn decode_drawer_kick(params: &[u8]) -> Command {
    let pin = match n(params) {
        0 | 48 => Some(2),
        1 | 49 => Some(5),
        _ => None,
    };
    let t1 = u32::from(params.get(1).copied().unwrap_or(0));
    let t2 = u32::from(params.get(2).copied().unwrap_or(0));
    Command::DrawerKick {
        pin,
        on_ms: t1.saturating_mul(2),
        off_ms: t2.max(t1).saturating_mul(2),
    }
}

/// `ESC - n`: 0/48 off, 1/49 one dot, 2/50 two dots.
fn decode_underline(n: u8) -> Option<UnderlineMode> {
    match n {
        0 | 48 => Some(UnderlineMode::Off),
        1 | 49 => Some(UnderlineMode::OneDot),
        2 | 50 => Some(UnderlineMode::TwoDots),
        _ => None,
    }
}

/// `ESC M n`: 0-4 / 48-52 fonts A-E, 97/98 special fonts A/B.
fn decode_font(n: u8) -> Option<Font> {
    match n {
        0 | 48 => Some(Font::A),
        1 | 49 => Some(Font::B),
        2 | 50 => Some(Font::C),
        3 | 51 => Some(Font::D),
        4 | 52 => Some(Font::E),
        97 => Some(Font::SpecialA),
        98 => Some(Font::SpecialB),
        _ => None,
    }
}

/// `ESC a n`: 0/48 left, 1/49 center, 2/50 right.
fn decode_alignment(n: u8) -> Option<Alignment> {
    match n {
        0 | 48 => Some(Alignment::Left),
        1 | 49 => Some(Alignment::Center),
        2 | 50 => Some(Alignment::Right),
        _ => None,
    }
}

/// `GS ! n`: bits 0-2 height - 1, bits 4-6 width - 1; bits 3 and 7 are
/// reserved (range `0xxx0xxxb`). Out-of-range values are logged and ignored.
fn decode_character_size(n: u8) -> Command {
    if n & 0b1000_1000 != 0 {
        return Command::SetCharacterSize {
            width: None,
            height: None,
        };
    }
    Command::SetCharacterSize {
        // Masked to 0-7, so adding 1 cannot overflow.
        width: Some(((n >> 4) & 0x07).saturating_add(1)),
        height: Some((n & 0x07).saturating_add(1)),
    }
}

/// `ESC ! n`: bit 0 font (A/B), bit 3 emphasized, bit 4 double height,
/// bit 5 double width, bit 7 underline. Bits 1, 2 and 6 are reserved and
/// ignored.
fn decode_print_mode(n: u8) -> Command {
    Command::SetPrintMode {
        font: if n & 0x01 != 0 { Font::B } else { Font::A },
        bold: n & 0x08 != 0,
        double_height: n & 0x10 != 0,
        double_width: n & 0x20 != 0,
        underline: n & 0x80 != 0,
    }
}

#[derive(Debug, Clone, Copy)]
enum State {
    /// Between commands.
    Ground,
    /// A prefix byte was read; waiting for the command byte.
    Prefix(u8),
    /// A prefix and the first byte of a three-byte name (`GS (`, `GS 8`,
    /// ...) were read; waiting for the third byte.
    LongName(u8, u8),
    /// Collecting parameter bytes for a recognized command.
    Params(CommandSpec),
    /// Consuming the payload of an oversized command without storing it;
    /// `remaining` bytes are left.
    Skip { remaining: u64 },
}

/// Incremental parser for one connection.
#[derive(Debug, Clone)]
pub struct Parser {
    config: ParserConfig,
    code_page: CodePage,
    state: State,
    /// Offset of the next byte to be read.
    offset: u64,
    /// Pending printable bytes and the offset of the first one.
    text: Vec<u8>,
    text_start: u64,
    /// Bytes of the command being parsed and the offset of the first one.
    seq: Vec<u8>,
    seq_start: u64,
}

impl Default for Parser {
    fn default() -> Self {
        Self::new(ParserConfig::default())
    }
}

impl Parser {
    /// Creates a parser in power-on state.
    pub fn new(config: ParserConfig) -> Self {
        Self {
            config,
            code_page: config.default_code_page,
            state: State::Ground,
            offset: 0,
            text: Vec::new(),
            text_start: 0,
            seq: Vec::new(),
            seq_start: 0,
        }
    }

    /// Code page currently used to decode text.
    pub fn code_page(&self) -> CodePage {
        self.code_page
    }

    /// Total number of bytes fed so far.
    pub fn bytes_consumed(&self) -> u64 {
        self.offset
    }

    /// Bytes held for the text run or command in progress. Bounded by
    /// [`MAX_PAYLOAD`] plus a command header, whatever the input declares.
    pub fn pending_bytes(&self) -> usize {
        self.text.len().saturating_add(self.seq.len())
    }

    /// Consumes a chunk of the stream and returns the commands completed by it.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<DecodedCommand> {
        let mut out = Vec::new();
        for &byte in bytes {
            self.step(byte, &mut out);
            self.offset = self.offset.saturating_add(1);
        }
        out
    }

    /// Ends the stream: flushes pending text and turns an unfinished command
    /// into an `unknown` item with reason `incomplete`. The parser can keep
    /// being fed afterwards (offsets continue), starting from a clean state
    /// except for the selected code page.
    pub fn finish(&mut self) -> Vec<DecodedCommand> {
        let mut out = Vec::new();
        self.flush_text(&mut out);
        if !self.seq.is_empty() {
            let bytes = std::mem::take(&mut self.seq);
            out.push(DecodedCommand {
                offset: self.seq_start,
                bytes,
                command: Command::Unknown {
                    reason: UnknownReason::Incomplete,
                },
            });
        }
        self.state = State::Ground;
        out
    }

    fn step(&mut self, byte: u8, out: &mut Vec<DecodedCommand>) {
        match self.state {
            State::Ground => self.ground(byte, out),
            State::Prefix(prefix) => {
                self.seq.push(byte);
                if is_long_name(prefix, byte) {
                    self.state = State::LongName(prefix, byte);
                    return;
                }
                self.start(lookup(prefix, byte), out);
            }
            State::LongName(prefix, code) => {
                self.seq.push(byte);
                self.start(lookup_long(prefix, code, byte), out);
            }
            State::Params(spec) => {
                self.seq.push(byte);
                self.advance(spec, out);
            }
            State::Skip { remaining } => {
                if self.seq.is_empty() {
                    self.seq_start = self.offset;
                }
                self.seq.push(byte);
                let remaining = remaining.saturating_sub(1);
                if remaining == 0 || self.seq.len() >= SKIP_CHUNK {
                    let length = u32::try_from(self.seq.len()).unwrap_or(u32::MAX);
                    self.emit_seq(Command::SkippedData { length }, out);
                }
                if remaining > 0 {
                    self.state = State::Skip { remaining };
                }
            }
        }
    }

    /// The command name is complete: start collecting its parameters, or
    /// report it as unknown.
    fn start(&mut self, spec: Option<CommandSpec>, out: &mut Vec<DecodedCommand>) {
        match spec {
            Some(spec) => self.advance(spec, out),
            None => self.emit_seq(
                Command::Unknown {
                    reason: UnknownReason::UnsupportedCommand,
                },
                out,
            ),
        }
    }

    /// Checks whether the command in `seq` is complete.
    fn advance(&mut self, spec: CommandSpec, out: &mut Vec<DecodedCommand>) {
        let params = self.seq.get(spec.name_len..).unwrap_or(&[]);
        match spec.params.progress(params) {
            Progress::NeedMore => self.state = State::Params(spec),
            Progress::Done => self.complete(spec, out),
            Progress::Oversized { payload } => {
                self.complete(spec, out);
                self.state = State::Skip { remaining: payload };
            }
        }
    }

    fn ground(&mut self, byte: u8, out: &mut Vec<DecodedCommand>) {
        if byte >= 0x20 {
            if self.text.is_empty() {
                self.text_start = self.offset;
            }
            self.text.push(byte);
            if self.text.len() >= MAX_TEXT_RUN {
                self.flush_text(out);
            }
            return;
        }
        self.flush_text(out);
        let single = |command| DecodedCommand {
            offset: self.offset,
            bytes: vec![byte],
            command,
        };
        match byte {
            LF => out.push(single(Command::LineFeed)),
            CR => out.push(single(Command::CarriageReturn)),
            HT => out.push(single(Command::HorizontalTab)),
            // DLE starts real-time commands (stage 5); FS starts Kanji and
            // NV image commands. Both are prefixes so their command byte is
            // reported together with them.
            ESC | GS | DLE | FS => {
                self.seq.clear();
                self.seq.push(byte);
                self.seq_start = self.offset;
                self.state = State::Prefix(byte);
            }
            _ => out.push(single(Command::Unknown {
                reason: UnknownReason::ControlCharacter,
            })),
        }
    }

    fn complete(&mut self, spec: CommandSpec, out: &mut Vec<DecodedCommand>) {
        let command = (spec.decode)(self.seq.get(spec.name_len..).unwrap_or(&[]));
        match &command {
            Command::Initialize => self.code_page = self.config.default_code_page,
            // Deliberate deviation: an unsupported page cannot be drawn, so
            // text after it is decoded as CP437 (the printer default).
            Command::SetCodePage { code_page, .. } => {
                self.code_page = code_page.unwrap_or(CodePage::Cp437);
            }
            _ => {}
        }
        self.emit_seq(command, out);
    }

    fn emit_seq(&mut self, command: Command, out: &mut Vec<DecodedCommand>) {
        out.push(DecodedCommand {
            offset: self.seq_start,
            bytes: std::mem::take(&mut self.seq),
            command,
        });
        self.state = State::Ground;
    }

    fn flush_text(&mut self, out: &mut Vec<DecodedCommand>) {
        if self.text.is_empty() {
            return;
        }
        let bytes = std::mem::take(&mut self.text);
        out.push(DecodedCommand {
            offset: self.text_start,
            command: Command::Text {
                text: self.code_page.decode(&bytes),
            },
            bytes,
        });
    }
}

/// Parses a complete stream in one go (`feed` + `finish`).
pub fn parse(bytes: &[u8], config: ParserConfig) -> Vec<DecodedCommand> {
    let mut parser = Parser::new(config);
    let mut out = parser.feed(bytes);
    out.extend(parser.finish());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commands(bytes: &[u8]) -> Vec<Command> {
        parse(bytes, ParserConfig::default())
            .into_iter()
            .map(|d| d.command)
            .collect()
    }

    #[test]
    fn text_and_line_feed() {
        assert_eq!(
            commands(b"Hi\n"),
            vec![Command::Text { text: "Hi".into() }, Command::LineFeed]
        );
    }

    #[test]
    fn unknown_keeps_only_prefix_and_command_byte() {
        let out = parse(b"\x1dZ\x41", ParserConfig::default());
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].bytes, vec![0x1D, b'Z']);
        assert_eq!(out[1].command, Command::Text { text: "A".into() });
        assert_eq!(out[1].offset, 2);
    }

    #[test]
    fn raster_image_is_framed_by_its_header() {
        let mut input = b"\x1dv0\x01\x02\x00\x03\x00".to_vec();
        input.extend_from_slice(&[0xAA; 6]);
        input.extend_from_slice(b"x\n");
        let out = parse(&input, ParserConfig::default());
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].bytes.len(), 14);
        assert_eq!(
            out[0].command,
            Command::RasterImage {
                mode: Some(RasterMode::DoubleWidth),
                width: 16,
                height: 3,
                bytes_expected: 6,
                data: vec![0xAA; 6],
            }
        );
        // GS v not followed by '0': three bytes, unknown.
        let other = parse(b"\x1dv1A", ParserConfig::default());
        assert_eq!(other[0].bytes, b"\x1dv1".to_vec());
    }

    #[test]
    fn oversized_raster_is_skipped_in_chunks() {
        // 2048 bytes x 1024 rows = 2 MiB > MAX_PAYLOAD.
        let mut input = b"\x1dv0\x00\x00\x08\x00\x04".to_vec();
        let payload = 2048 * 1024;
        input.extend(std::iter::repeat_n(0x55, payload));
        input.extend_from_slice(b"ok\n");
        let out = parse(&input, ParserConfig::default());
        assert_eq!(out[0].bytes.len(), 8);
        assert!(matches!(
            &out[0].command,
            Command::RasterImage { bytes_expected: 2_097_152, data, .. } if data.is_empty()
        ));
        let skipped: Vec<_> = out
            .iter()
            .filter(|d| matches!(d.command, Command::SkippedData { .. }))
            .collect();
        assert_eq!(skipped.len(), payload / SKIP_CHUNK);
        assert!(skipped.iter().all(|d| d.bytes.len() == SKIP_CHUNK));
        assert_eq!(out.len(), 1 + skipped.len() + 2);
        assert_eq!(
            out[out.len() - 2].command,
            Command::Text { text: "ok".into() }
        );
    }

    #[test]
    fn qr_functions() {
        let cmds = commands(
            b"\x1d(k\x04\x001A2\x00\x1d(k\x03\x001C\x05\x1d(k\x03\x001E3\
              \x1d(k\x06\x001P0abc\x1d(k\x03\x001Q0\x1d(k\x03\x001R0\x1d(k\x03\x001C\x11",
        );
        assert_eq!(
            cmds,
            vec![
                Command::SetQrModel {
                    model: Some(QrCodeModel::Model2)
                },
                Command::SetQrModuleSize { size: Some(5) },
                Command::SetQrErrorCorrection {
                    level: Some(QrErrorCorrection::H)
                },
                Command::QrStoreData {
                    length: 3,
                    valid: true,
                    data: b"abc".to_vec()
                },
                Command::QrPrint,
                Command::QrRequestSize,
                Command::SetQrModuleSize { size: None },
            ]
        );
    }

    #[test]
    fn other_symbols_and_graphics_are_consumed_by_length() {
        let out = parse(
            b"\x1d(k\x05\x000P0AB\x1d(L\x04\x0002\x01\x02\x1d8L\x02\x00\x00\x0002\
              \x1b*\x21\x02\x00ABCDEF\x1d*\x01\x0112345678\x1d/\x00\x1cp\x01\x00\x1d(E\x02\x00ab",
            ParserConfig::default(),
        );
        let names: Vec<_> = out
            .iter()
            .map(|d| match &d.command {
                Command::UnsupportedImage { command, length } => format!("{command}/{length}"),
                other => format!("{other:?}"),
            })
            .collect();
        assert_eq!(
            names,
            vec![
                "GS ( k PDF417 fn=80/5",
                "GS ( L fn=50/4",
                "GS 8 L fn=50/2",
                "ESC * m=33/6",
                "GS */8",
                "GS / m=0/0",
                "FS p n=1 m=0/0",
                "Unknown { reason: UnsupportedCommand }",
            ]
        );
        assert_eq!(out[7].bytes.len(), 7);
    }

    #[test]
    fn barcode_functions_a_and_b() {
        let out = parse(
            b"\x1dk\x02789123456789\x00\x1dkC\x0c789123456789\x1dkI\x04{B\x7f\\\x1dk\x07X",
            ParserConfig::default(),
        );
        let ean = Command::Barcode {
            symbology: Some(Symbology::Ean13),
            function: Some(BarcodeFunction::A),
            data: "789123456789".into(),
            valid: true,
            problem: None,
            raw: b"789123456789".to_vec(),
        };
        assert_eq!(out[0].command, ean);
        assert_eq!(out[0].bytes.len(), 16);
        assert!(matches!(
            &out[1].command,
            Command::Barcode {
                function: Some(BarcodeFunction::B),
                valid: true,
                ..
            }
        ));
        assert!(matches!(
            &out[2].command,
            Command::Barcode { symbology: Some(Symbology::Code128), data, valid: true, .. }
                if data == "{B\\x7F\\\\"
        ));
        assert!(matches!(
            &out[3].command,
            Command::Barcode {
                symbology: None,
                problem: Some(crate::BarcodeProblem::UnknownSymbology),
                ..
            }
        ));
        assert_eq!(out[3].bytes.len(), 3);
        // Function A without NUL: ends after 256 data bytes.
        let mut long = b"\x1dk\x04".to_vec();
        long.extend(std::iter::repeat_n(b'A', 300));
        let out = parse(&long, ParserConfig::default());
        assert_eq!(out[0].bytes.len(), 3 + 256);
        assert!(matches!(
            &out[0].command,
            Command::Barcode {
                problem: Some(crate::BarcodeProblem::MissingTerminator),
                ..
            }
        ));
    }

    #[test]
    fn cut_lengths_depend_on_the_function() {
        let out = parse(
            b"\x1dV\x00\x1dVA\x05\x1dVh\x10\x1dV\x07X",
            ParserConfig::default(),
        );
        let lens: Vec<_> = out.iter().map(|d| d.bytes.len()).collect();
        assert_eq!(lens, vec![3, 4, 4, 3, 1]);
        assert_eq!(
            out[1].command,
            Command::Cut {
                mode: Some(CutMode::Full),
                function: Some(CutFunction::B),
                feed: Some(5)
            }
        );
        assert_eq!(
            out[2].command,
            Command::Cut {
                mode: Some(CutMode::Partial),
                function: Some(CutFunction::D),
                feed: Some(0x10)
            }
        );
        assert_eq!(
            out[3].command,
            Command::Cut {
                mode: None,
                function: None,
                feed: None
            }
        );
    }

    #[test]
    fn drawer_kick_pins_and_times() {
        let cmds = commands(b"\x1bp\x00\x19\xfa\x1bp1\x32\x32\x1bp\x02\x01\x01\x1bp\x00\x0a\x05");
        assert_eq!(
            cmds,
            vec![
                Command::DrawerKick {
                    pin: Some(2),
                    on_ms: 50,
                    off_ms: 500
                },
                Command::DrawerKick {
                    pin: Some(5),
                    on_ms: 100,
                    off_ms: 100
                },
                Command::DrawerKick {
                    pin: None,
                    on_ms: 2,
                    off_ms: 2
                },
                Command::DrawerKick {
                    pin: Some(2),
                    on_ms: 20,
                    off_ms: 20
                },
            ]
        );
    }

    #[test]
    fn incomplete_at_eof_is_preserved() {
        let out = parse(b"\x1b3", ParserConfig::default());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].bytes, vec![0x1B, b'3']);
        assert_eq!(
            out[0].command,
            Command::Unknown {
                reason: UnknownReason::Incomplete
            }
        );
    }

    #[test]
    fn code_page_switch_and_reset() {
        let mut p = Parser::new(ParserConfig {
            default_code_page: CodePage::Cp850,
        });
        let out = p.feed(b"\x1bt\x10\x80\x1b@\x80");
        let texts: Vec<_> = out
            .iter()
            .filter_map(|d| match &d.command {
                Command::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        // The second text is still pending (no terminator yet).
        assert_eq!(texts, vec!["€"]);
        let rest = p.finish();
        assert_eq!(rest[0].command, Command::Text { text: "Ç".into() });
    }

    #[test]
    fn long_text_is_split_deterministically() {
        let input = vec![b'x'; MAX_TEXT_RUN * 2 + 1];
        let out = parse(&input, ParserConfig::default());
        assert_eq!(out.len(), 3);
        assert_eq!(out[1].offset, MAX_TEXT_RUN as u64);
    }
}
