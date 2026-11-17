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
    /// Anything not recognized; the raw bytes are in the envelope.
    Unknown { reason: UnknownReason },
}
