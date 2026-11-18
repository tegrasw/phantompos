//! Typed ESC/POS commands produced by the parser.
//!
//! The serialized form is the JSON contract with the UI log (see the crate
//! README): every item is a [`DecodedCommand`] envelope whose `command` is an
//! internally tagged object `{ "type": "<variant>", ...fields }`. Field values
//! are plain JSON scalars or camelCase strings; `null` means the parameter is
//! outside the range defined by the Epson reference (the command is logged but
//! has no effect on the receipt).

use serde::{Deserialize, Serialize};

use crate::codepage::CodePage;

/// One decoded item of the stream, with its position and raw bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecodedCommand {
    /// Offset of the first byte in the connection stream.
    pub offset: u64,
    /// The exact bytes that make up this item. Concatenating `bytes` of every
    /// item reproduces the input stream.
    pub bytes: Vec<u8>,
    /// What the bytes mean.
    pub command: Command,
}

/// Character font. The emulator draws only fonts A and B; the others are
/// recognized so the log is accurate, but selecting them has no effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Font {
    #[default]
    A,
    B,
    C,
    D,
    E,
    SpecialA,
    SpecialB,
}

/// Underline mode of `ESC -`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UnderlineMode {
    #[default]
    Off,
    /// 1-dot thick underline.
    OneDot,
    /// 2-dot thick underline.
    TwoDots,
}

/// Justification of `ESC a`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
}

/// Cutting shape of `GS V`, `ESC i` and `ESC m`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CutMode {
    Full,
    /// Partial cut (one or three points left uncut).
    Partial,
}

/// Which `GS V` function a cut command uses (Epson "Function A" to "D").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CutFunction {
    /// Cuts the paper (`m` = 0, 1, 48, 49; also `ESC i` and `ESC m`).
    A,
    /// Feeds to the cutting position + `n` and cuts (`m` = 65, 66).
    B,
    /// Presets the cutting position + `n`; cuts when it reaches the cutter
    /// (`m` = 97, 98).
    C,
    /// Feeds to the cutting position + `n`, cuts and reverse feeds to the
    /// print start position (`m` = 103, 104).
    D,
}

/// Scaling mode of `GS v 0` (`m` = 0-3 / 48-51).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RasterMode {
    /// x1 horizontally and vertically.
    Normal,
    /// x2 horizontally.
    DoubleWidth,
    /// x2 vertically.
    DoubleHeight,
    /// x2 in both directions.
    Quadruple,
}

impl RasterMode {
    /// Horizontal and vertical scale factors.
    pub fn scale(self) -> (u32, u32) {
        match self {
            Self::Normal => (1, 1),
            Self::DoubleWidth => (2, 1),
            Self::DoubleHeight => (1, 2),
            Self::Quadruple => (2, 2),
        }
    }
}

/// QR Code model of `GS ( k` function 165.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QrCodeModel {
    /// Model 1 (obsolete; drawn as model 2, see `receipt.rs`).
    Model1,
    /// Model 2, the printer default.
    #[default]
    Model2,
    /// Micro QR Code.
    Micro,
}

/// QR Code error correction level of `GS ( k` function 169.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QrErrorCorrection {
    /// About 7 % recovery; the printer default.
    #[default]
    L,
    /// About 15 %.
    M,
    /// About 25 %.
    Q,
    /// About 30 %.
    H,
}

/// Where `GS H` prints the human readable interpretation of a barcode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HriPosition {
    /// Not printed (default).
    #[default]
    None,
    Above,
    Below,
    Both,
}

/// Barcode system of `GS k`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Symbology {
    UpcA,
    UpcE,
    /// JAN13 / EAN-13.
    Ean13,
    /// JAN8 / EAN-8.
    Ean8,
    Code39,
    /// Interleaved 2 of 5.
    Itf,
    /// CODABAR (NW-7).
    Codabar,
    Code93,
    Code128,
    /// Recognized and logged; not drawn.
    Gs1128,
    /// Recognized and logged; not drawn.
    Gs1DataBarOmnidirectional,
    /// Recognized and logged; not drawn.
    Gs1DataBarTruncated,
    /// Recognized and logged; not drawn.
    Gs1DataBarLimited,
    /// Recognized and logged; not drawn.
    Gs1DataBarExpanded,
    /// Recognized and logged; not drawn.
    Code128Auto,
}

/// The two forms of `GS k`: function A (`m` 0-6, data ends with NUL) and
/// function B (`m` 65-79, data length byte `n`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BarcodeFunction {
    A,
    B,
}

/// Why a `GS k` barcode is not drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BarcodeProblem {
    /// `m` is not a barcode system of the reference.
    UnknownSymbology,
    /// A GS1 system or CODE128 auto: recognized, but not drawn by this
    /// version.
    UnsupportedSymbology,
    /// The number of data bytes is outside the range of the reference.
    InvalidLength,
    /// A data byte outside the character set of the symbology (or an invalid
    /// CODE128 escape).
    InvalidCharacter,
    /// CODABAR without a valid start/stop character (A-D, a-d) at both ends,
    /// or with one inside the data.
    InvalidStartStop,
    /// UPC-E data whose number system character (first digit) is not 0.
    InvalidNumberSystem,
    /// 11/12-digit UPC-E data that cannot be zero-suppressed (the zeros of
    /// the reference table are missing).
    NotCompressible,
    /// Function A data with no NUL within 255 bytes; the command ends there.
    MissingTerminator,
}

/// Why a sequence was not recognized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UnknownReason {
    /// A command prefix (ESC, GS, DLE or FS) followed by a command byte this
    /// version does not implement. Only the prefix and the command byte are
    /// consumed; any parameters that follow are processed as ordinary data.
    UnsupportedCommand,
    /// A control byte (0x00-0x1F) that is neither a command nor a prefix. The
    /// Epson reference says the printer ignores it.
    ControlCharacter,
    /// The stream ended in the middle of a command.
    Incomplete,
}

/// A decoded ESC/POS command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Command {
    /// A run of printable bytes (0x20-0xFF), decoded with the active code page.
    Text { text: String },
    /// `ESC @` - initialize printer.
    Initialize,
    /// `LF` - print and line feed.
    LineFeed,
    /// `CR` - print and carriage return.
    CarriageReturn,
    /// `HT` - horizontal tab.
    HorizontalTab,
    /// `ESC d n` - print and feed `n` lines.
    FeedLines { lines: u8 },
    /// `ESC J n` - print and feed `n` motion units (dots).
    FeedDots { dots: u8 },
    /// `ESC 2` - select default line spacing.
    ResetLineSpacing,
    /// `ESC 3 n` - set line spacing to `n` dots.
    SetLineSpacing { dots: u8 },
    /// `ESC E n` - emphasized (bold) on/off (LSB of `n`).
    SetBold { enabled: bool },
    /// `ESC - n` - underline mode; `null` when `n` is out of range.
    SetUnderline { mode: Option<UnderlineMode> },
    /// `GS B n` - white/black reverse on/off (LSB of `n`).
    SetReverse { enabled: bool },
    /// `ESC M n` - character font; `null` when `n` is out of range.
    SetFont { font: Option<Font> },
    /// `GS ! n` - width and height multipliers (1-8); both `null` when `n`
    /// sets a reserved bit.
    SetCharacterSize {
        width: Option<u8>,
        height: Option<u8>,
    },
    /// `ESC ! n` - font, emphasized, double height/width and underline at once.
    SetPrintMode {
        font: Font,
        bold: bool,
        double_height: bool,
        double_width: bool,
        underline: bool,
    },
    /// `ESC a n` - justification; `null` when `n` is out of range.
    SetAlignment { alignment: Option<Alignment> },
    /// `ESC t n` - character code table. `codePage` is `null` when page `n` is
    /// not supported; text is then decoded as CP437.
    SetCodePage {
        page: u8,
        code_page: Option<CodePage>,
    },
    /// `ESC R n` - international character set. Logged, no effect.
    SetInternationalCharset { n: u8 },
    /// `GS V m [n]`, `ESC i`, `ESC m` - paper cut. `feed` is the `n` of
    /// functions B, C and D (vertical motion units, i.e. dots here) and `null`
    /// for function A. All three fields are `null` when `m` is out of range
    /// (the command is then ignored).
    Cut {
        mode: Option<CutMode>,
        function: Option<CutFunction>,
        feed: Option<u8>,
    },
    /// `ESC p m t1 t2` - drawer kick pulse. `pin` is the drawer kick-out
    /// connector pin (2 or 5), `null` when `m` is out of range (no pulse).
    /// Times are in milliseconds (`t x 2 ms`); per the reference, when
    /// `t2 < t1` the OFF time equals the ON time.
    DrawerKick {
        pin: Option<u8>,
        on_ms: u32,
        off_ms: u32,
    },
    /// `GS v 0 m xL xH yL yH d1...dk` - raster bit image. `width` is in dots
    /// (`(xL + xH x 256) x 8`), `height` in dots, `bytesExpected` is `k`.
    /// `mode` is `null` when `m` is out of range (the image is consumed and
    /// not drawn). When the image data is larger than [`MAX_PAYLOAD`] the
    /// envelope holds only the 8-byte header, the data follows as
    /// `skippedData` items and nothing is drawn.
    ///
    /// [`MAX_PAYLOAD`]: crate::MAX_PAYLOAD
    RasterImage {
        mode: Option<RasterMode>,
        width: u32,
        height: u32,
        bytes_expected: u32,
        /// The image data (`d1...dk`); never serialized (it is in the
        /// envelope `bytes`).
        #[serde(skip)]
        data: Vec<u8>,
    },
    /// `GS ( k` function 165 - QR Code model; `null` when out of range.
    SetQrModel { model: Option<QrCodeModel> },
    /// `GS ( k` function 167 - QR Code module size in dots (1-16); `null`
    /// when out of range.
    SetQrModuleSize { size: Option<u8> },
    /// `GS ( k` function 169 - QR Code error correction level; `null` when
    /// out of range.
    SetQrErrorCorrection { level: Option<QrErrorCorrection> },
    /// `GS ( k` function 180 - store `length` bytes of QR Code data.
    /// `valid` is false when `m` is not 48 or `pL + pH x 256` is outside
    /// 4-7092; the data is then not stored.
    QrStoreData {
        length: u32,
        valid: bool,
        #[serde(skip)]
        data: Vec<u8>,
    },
    /// `GS ( k` function 181 - print the stored QR Code.
    QrPrint,
    /// `GS ( k` function 182 - transmit the QR Code size information. Logged;
    /// no reply is sent in this version.
    QrRequestSize,
    /// `GS h n` - barcode height in dots (1-255); `null` for 0.
    SetBarcodeHeight { dots: Option<u8> },
    /// `GS w n` - barcode module width (2-6); `null` when out of range.
    SetBarcodeWidth { module: Option<u8> },
    /// `GS H n` - HRI print position; `null` when out of range.
    SetHriPosition { position: Option<HriPosition> },
    /// `GS f n` - HRI font; `null` when out of range. Only A and B are drawn.
    SetHriFont { font: Option<Font> },
    /// `GS k` - print a barcode. `symbology` is `null` for an unknown `m`
    /// (then `function` is `null` too and no data is consumed). `data` is the
    /// barcode data as text: printable ASCII as is, any other byte as
    /// `\xNN` and a backslash as `\\`. When `valid` is false, `problem` says
    /// why and nothing is drawn.
    Barcode {
        symbology: Option<Symbology>,
        function: Option<BarcodeFunction>,
        data: String,
        valid: bool,
        problem: Option<BarcodeProblem>,
        /// The data bytes (without `n` or the NUL); never serialized.
        #[serde(skip)]
        raw: Vec<u8>,
    },
    /// An image or symbol command that is recognized by its length but not
    /// drawn: `ESC *`, `GS ( L`, `GS 8 L`, `GS *`, `GS /`, `FS p` and the
    /// `GS ( k` symbols other than QR Code. `command` names it (for example
    /// `"GS ( L fn=112"`, `"GS ( k PDF417 fn=80"`); `length` is the number of
    /// bytes after the command's own length field (or its data size).
    UnsupportedImage { command: String, length: u32 },
    /// A slice of the data of an image larger than [`MAX_PAYLOAD`] (at most
    /// 65536 bytes per item), consumed without being stored.
    ///
    /// [`MAX_PAYLOAD`]: crate::MAX_PAYLOAD
    SkippedData { length: u32 },
    /// Anything not recognized; the raw bytes are in the envelope.
    Unknown { reason: UnknownReason },
}
