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

use serde::{Deserialize, Serialize};

use crate::codepage::CodePage;
use crate::command::{Alignment, Command, DecodedCommand, Font, UnderlineMode, UnknownReason};

const HT: u8 = 0x09;
const LF: u8 = 0x0A;
const CR: u8 = 0x0D;
const DLE: u8 = 0x10;
const ESC: u8 = 0x1B;
const FS: u8 = 0x1C;
const GS: u8 = 0x1D;

/// Longest `text` command emitted; longer runs are split deterministically.
pub const MAX_TEXT_RUN: usize = 4096;

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
}

/// Table entry describing one recognized command.
#[derive(Debug, Clone, Copy)]
struct CommandSpec {
    params: ParamLength,
    /// Builds the command from its parameter bytes (exactly `params` long).
    decode: fn(&[u8]) -> Command,
}

const fn spec(params: usize, decode: fn(&[u8]) -> Command) -> CommandSpec {
    CommandSpec {
        params: ParamLength::Fixed(params),
        decode,
    }
}

/// First parameter byte, or 0 if missing (cannot happen for fixed lengths).
fn n(params: &[u8]) -> u8 {
    params.first().copied().unwrap_or(0)
}

/// The stage 2 command table, keyed by (prefix, command byte).
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
        _ => return None,
    };
    Some(found)
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
    /// Collecting parameter bytes for a recognized command.
    Params(CommandSpec),
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
                match lookup(prefix, byte) {
                    Some(spec) => match spec.params {
                        ParamLength::Fixed(0) => self.complete(spec, out),
                        ParamLength::Fixed(_) => self.state = State::Params(spec),
                    },
                    None => self.emit_seq(
                        Command::Unknown {
                            reason: UnknownReason::UnsupportedCommand,
                        },
                        out,
                    ),
                }
            }
            State::Params(spec) => {
                self.seq.push(byte);
                let ParamLength::Fixed(count) = spec.params;
                // seq = prefix + command byte + parameters.
                if self.seq.len() >= count.saturating_add(2) {
                    self.complete(spec, out);
                }
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
        let command = (spec.decode)(self.seq.get(2..).unwrap_or(&[]));
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
        let out = parse(b"\x1dV\x41", ParserConfig::default());
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].bytes, vec![0x1D, b'V']);
        assert_eq!(out[1].command, Command::Text { text: "A".into() });
        assert_eq!(out[1].offset, 2);
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
