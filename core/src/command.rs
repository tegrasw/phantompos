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
    /// Anything not recognized; the raw bytes are in the envelope.
    Unknown { reason: UnknownReason },
}
