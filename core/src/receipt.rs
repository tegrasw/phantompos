//! Receipt model and the builder that turns commands into it.
//!
//! The model is laid out in printer dots so the UI can draw it on a canvas of
//! exactly [`Receipt::width`] dots: every line knows its vertical position and
//! paper advance, and every span its horizontal position (alignment already
//! applied), size and style.
//!
//! Behaviour follows the Epson ESC/POS reference (Standard mode only):
//!
//! * **Wrapping**: when a character does not fit in the remaining width, the
//!   line is printed and fed as by `LF` and the character starts the next line
//!   ("print buffer-full printing"). Wrapping is per character, not per word.
//! * **Line advance**: a printed line feeds `max(feed, tallest character)`,
//!   where `feed` is the line spacing for `LF`/wrapping, `n` x line spacing for
//!   `ESC d n` and `n` dots for `ESC J n`. A feed with nothing to print adds a
//!   blank line of exactly `feed` dots (nothing for a zero feed).
//! * **Baseline**: characters of different heights on a line share the
//!   baseline (`Span::y` is the offset of the cell top inside the line).
//! * **HT**: default tab stops every 8 font-A columns (96 dots); with no stop
//!   left inside the paper the position moves past the right edge, so the next
//!   character wraps. Tab stops cannot be changed yet (`ESC D` is not
//!   implemented).
//! * **ESC a** only takes effect at the beginning of a line; mid-line it is
//!   ignored.
//! * **ESC @** discards the unprinted line (it clears the print buffer) and
//!   restores the power-on state.
//! * **Underline** is not drawn under reverse characters; a span's `underline`
//!   is the effective one.
//! * **CR** is ignored (auto line feed disabled, the usual setting) unless
//!   [`ReceiptConfig::cr_line_feed`] is set, in which case it acts as `LF`.
//! * **Cuts** (`GS V`, `ESC i`, `ESC m`) close the current receipt and start
//!   a new, empty one; printer settings (styles, spacing, alignment, code
//!   page) are kept, since a cut is not a reset. Per the reference ("enabled
//!   only when processed at the beginning of line"), a cut received while the
//!   print buffer holds data (text or an HT skip) is **ignored**: the data
//!   stays in the buffer and continues on the same receipt. Functions B, C
//!   and D first feed `n` dots (a blank line, as `ESC J n` with no data).
//! * A receipt with nothing on it (no printed line and no feed, i.e. zero
//!   height) is never produced: a cut on it only starts the next one.
//! * **ESC p** produces a [`DrawerKick`] effect when `m` is valid.
//!
//! Deliberate deviations: fonts C, D, E and the special fonts are not drawn
//! (selecting them is ignored); `ESC R` is ignored (always U.S.A.); an
//! unprinted line at the end of the stream is printed as if `LF` followed,
//! where a real printer would keep it in its buffer. The distance between the
//! print head and the autocutter is not modelled: the paper is cut right
//! below the last feed, not 10-15 mm further down (so the end of one receipt
//! never spills onto the next). `GS V` function C (preset cutting position)
//! is treated like function B (feed `n`, cut now), and the reverse feed of
//! function D has no visible effect.

use serde::{Deserialize, Serialize};

use crate::command::{Alignment, Command, CutMode, DecodedCommand, Font, UnderlineMode};
use crate::paper::{FONT_A, PaperProfile, font_metrics};

/// Default line spacing (`ESC 2`): 30 dots on TM-T88/TM-T20 class printers.
pub const DEFAULT_LINE_SPACING: u32 = 30;

/// Default tab interval: 8 characters of the default font (font A).
const TAB_INTERVAL_DOTS: u32 = 8 * FONT_A.width;
/// Default tab stops are 8, 16, ..., 248 characters.
const TAB_STOP_COUNT: u32 = 31;

/// Settings of the receipt builder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptConfig {
    pub paper: PaperProfile,
    /// Treat `CR` as `LF` (printer "auto line feed" switch). Default off.
    pub cr_line_feed: bool,
}

/// A finished receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub paper: PaperProfile,
    /// Paper width in dots (384 or 576).
    pub width: u32,
    /// Total paper length in dots (sum of every element's advance).
    pub height: u32,
    /// How the receipt ended: the cut that closed it, or `null` when it was
    /// closed by the end of the stream (or is still in progress).
    pub cut: Option<CutMode>,
    pub elements: Vec<Element>,
}

/// A drawer kick pulse (`ESC p` with a valid pin).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DrawerKick {
    /// Drawer kick-out connector pin: 2 or 5.
    pub pin: u8,
    pub on_ms: u32,
    pub off_ms: u32,
}

/// Something a command did besides drawing, returned by
/// [`ReceiptBuilder::apply`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// A cut closed a non-empty receipt (`cut` is set).
    ReceiptCompleted(Receipt),
    /// The cash drawer was kicked.
    DrawerKick(DrawerKick),
}

/// Everything a complete stream produced (see [`render`]).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderOutput {
    /// Receipts in closing order; the last one has `cut: null` when the
    /// stream ended with pending content.
    pub receipts: Vec<Receipt>,
    pub drawer_kicks: Vec<DrawerKick>,
}

/// Something printed on the paper. Later stages add images and codes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Element {
    Line(Line),
}

/// One printed line (possibly blank, for paper feeds).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    /// Top of the line, in dots from the top of the receipt.
    pub y: u32,
    /// Height of the tallest character cell (0 for a blank line).
    pub height: u32,
    /// Baseline, in dots from the top of the line (0 for a blank line).
    pub baseline: u32,
    /// Paper fed after this line; the next element starts at `y + advance`.
    pub advance: u32,
    pub alignment: Alignment,
    /// Width of the printed content in dots, tab gaps included.
    pub content_width: u32,
    pub spans: Vec<Span>,
}

/// A run of characters with the same style.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    pub text: String,
    /// Left edge in dots from the left edge of the paper (alignment applied).
    pub x: u32,
    /// Top of the character cells in dots from the top of the line.
    pub y: u32,
    /// Total width in dots (`columns * char_width`).
    pub width: u32,
    /// Number of characters.
    pub columns: u32,
    /// Cell width of one character, multiplier applied.
    pub char_width: u32,
    /// Cell height of one character, multiplier applied.
    pub char_height: u32,
    pub style: TextStyle,
}

/// Character style of a span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextStyle {
    /// `a` or `b`.
    pub font: Font,
    pub bold: bool,
    pub underline: UnderlineMode,
    pub reverse: bool,
    pub width_multiplier: u32,
    pub height_multiplier: u32,
}

/// Printer settings that affect the receipt (reset by `ESC @`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PrintState {
    font: Font,
    bold: bool,
    underline_on: bool,
    /// Kept while underline is off (`ESC -` notes: "the thickness is
    /// maintained"); `ESC !` bit 7 turns underline on with this thickness.
    underline_thickness: UnderlineMode,
    reverse: bool,
    width_multiplier: u32,
    height_multiplier: u32,
    alignment: Alignment,
    line_spacing: u32,
}

impl Default for PrintState {
    fn default() -> Self {
        Self {
            font: Font::A,
            bold: false,
            underline_on: false,
            underline_thickness: UnderlineMode::OneDot,
            reverse: false,
            width_multiplier: 1,
            height_multiplier: 1,
            alignment: Alignment::Left,
            line_spacing: DEFAULT_LINE_SPACING,
        }
    }
}

impl PrintState {
    fn style(&self) -> TextStyle {
        let underline = if self.underline_on && !self.reverse {
            self.underline_thickness
        } else {
            UnderlineMode::Off
        };
        TextStyle {
            font: self.font,
            bold: self.bold,
            underline,
            reverse: self.reverse,
            width_multiplier: self.width_multiplier,
            height_multiplier: self.height_multiplier,
        }
    }
}

/// The line being filled (the printer's print buffer).
#[derive(Debug, Clone, Default)]
struct PendingLine {
    spans: Vec<Span>,
    /// Print position in dots from the left; may be `width + 1` after a tab.
    cursor: u32,
    /// False at the "beginning of the line" (no data and no tab skipped).
    has_data: bool,
}

/// Consumes commands and builds receipts, one per cut.
///
/// Feed it every decoded command with [`apply`](Self::apply) /
/// [`apply_all`](Self::apply_all); they return the [`Effect`]s (completed
/// receipts, drawer kicks). [`take_update`](Self::take_update) reports the
/// in-progress receipt when it changed, and [`finish`](Self::finish) closes
/// it at the end of the stream.
#[derive(Debug, Clone)]
pub struct ReceiptBuilder {
    config: ReceiptConfig,
    /// Paper for the next receipt, set while one is in progress.
    next_paper: Option<PaperProfile>,
    state: PrintState,
    line: PendingLine,
    elements: Vec<Element>,
    y: u32,
    /// The in-progress receipt changed since the last `take_update`.
    dirty: bool,
}

impl ReceiptBuilder {
    pub fn new(config: ReceiptConfig) -> Self {
        Self {
            config,
            next_paper: None,
            state: PrintState::default(),
            line: PendingLine::default(),
            elements: Vec::new(),
            y: 0,
            dirty: false,
        }
    }

    /// Paper of the receipt in progress (or of the next one, when empty).
    pub fn paper(&self) -> PaperProfile {
        self.config.paper
    }

    /// Selects the paper for receipts started from now on. A receipt that
    /// already has content (printed or in the print buffer) keeps its paper;
    /// the new one applies after it is closed.
    pub fn set_paper(&mut self, paper: PaperProfile) {
        if self.is_empty() {
            self.config.paper = paper;
            self.next_paper = None;
        } else if paper == self.config.paper {
            self.next_paper = None;
        } else {
            self.next_paper = Some(paper);
        }
    }

    fn width(&self) -> u32 {
        self.config.paper.width_dots()
    }

    /// Applies one command and returns its effect, if any.
    pub fn apply(&mut self, command: &Command) -> Option<Effect> {
        match command {
            Command::Text { text } => {
                for c in text.chars() {
                    self.put_char(c);
                }
            }
            Command::Initialize => {
                self.line = PendingLine::default();
                self.state = PrintState::default();
            }
            Command::LineFeed => self.print_and_feed(self.state.line_spacing),
            Command::CarriageReturn => {
                if self.config.cr_line_feed {
                    self.print_and_feed(self.state.line_spacing);
                }
            }
            Command::HorizontalTab => self.tab(),
            Command::FeedLines { lines } => {
                self.print_and_feed(u32::from(*lines).saturating_mul(self.state.line_spacing));
            }
            Command::FeedDots { dots } => self.print_and_feed(u32::from(*dots)),
            Command::ResetLineSpacing => self.state.line_spacing = DEFAULT_LINE_SPACING,
            Command::SetLineSpacing { dots } => self.state.line_spacing = u32::from(*dots),
            Command::SetBold { enabled } => self.state.bold = *enabled,
            Command::SetUnderline { mode } => match mode {
                Some(UnderlineMode::Off) => self.state.underline_on = false,
                Some(thickness) => {
                    self.state.underline_on = true;
                    self.state.underline_thickness = *thickness;
                }
                None => {}
            },
            Command::SetReverse { enabled } => self.state.reverse = *enabled,
            Command::SetFont { font } => {
                if let Some(font @ (Font::A | Font::B)) = font {
                    self.state.font = *font;
                }
            }
            Command::SetCharacterSize { width, height } => {
                if let (Some(w), Some(h)) = (width, height) {
                    self.state.width_multiplier = u32::from(*w);
                    self.state.height_multiplier = u32::from(*h);
                }
            }
            Command::SetPrintMode {
                font,
                bold,
                double_height,
                double_width,
                underline,
            } => {
                self.state.font = *font;
                self.state.bold = *bold;
                self.state.height_multiplier = if *double_height { 2 } else { 1 };
                self.state.width_multiplier = if *double_width { 2 } else { 1 };
                self.state.underline_on = *underline;
            }
            Command::SetAlignment { alignment } => {
                if let Some(alignment) = alignment
                    && !self.line.has_data
                {
                    self.state.alignment = *alignment;
                }
            }
            Command::Cut {
                mode: Some(mode),
                feed,
                ..
            } => {
                // Only effective at the beginning of a line (Epson notes).
                if self.line.has_data {
                    return None;
                }
                if let Some(dots) = feed {
                    self.print_and_feed(u32::from(*dots));
                }
                return self.close(Some(*mode)).map(Effect::ReceiptCompleted);
            }
            Command::DrawerKick {
                pin: Some(pin),
                on_ms,
                off_ms,
            } => {
                return Some(Effect::DrawerKick(DrawerKick {
                    pin: *pin,
                    on_ms: *on_ms,
                    off_ms: *off_ms,
                }));
            }
            Command::Cut { mode: None, .. }
            | Command::DrawerKick { pin: None, .. }
            | Command::SetCodePage { .. }
            | Command::SetInternationalCharset { .. }
            | Command::Unknown { .. } => {}
        }
        None
    }

    /// Applies every command of a decoded batch and returns their effects in
    /// order.
    pub fn apply_all<'a>(
        &mut self,
        commands: impl IntoIterator<Item = &'a DecodedCommand>,
    ) -> Vec<Effect> {
        commands
            .into_iter()
            .filter_map(|decoded| self.apply(&decoded.command))
            .collect()
    }

    /// True when nothing has been printed and the print buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty() && !self.line.has_data
    }

    /// The paper printed so far in the receipt in progress (`cut: null`), or
    /// `None` if nothing has been printed. Data still in the print buffer (a
    /// line without its `LF`) is not on paper yet and is not included.
    pub fn snapshot(&self) -> Option<Receipt> {
        if self.elements.is_empty() {
            return None;
        }
        Some(Receipt {
            paper: self.config.paper,
            width: self.width(),
            height: self.y,
            cut: None,
            elements: self.elements.clone(),
        })
    }

    /// Like [`snapshot`](Self::snapshot), but only when the receipt in
    /// progress changed since the previous call (or since it started).
    pub fn take_update(&mut self) -> Option<Receipt> {
        if !std::mem::take(&mut self.dirty) {
            return None;
        }
        self.snapshot()
    }

    /// Ends the stream: prints any pending line and returns the receipt in
    /// progress with `cut: null`, or `None` if it is empty. Printer settings
    /// are kept, so the builder can go on with a new receipt.
    pub fn finish(&mut self) -> Option<Receipt> {
        if self.line.has_data {
            self.print_and_feed(self.state.line_spacing);
        }
        self.close(None)
    }

    /// Closes the receipt in progress (the print buffer must be empty) and
    /// starts a new one at `y = 0`, with the pending paper selection.
    fn close(&mut self, cut: Option<CutMode>) -> Option<Receipt> {
        let receipt = (!self.elements.is_empty()).then(|| Receipt {
            paper: self.config.paper,
            width: self.width(),
            height: self.y,
            cut,
            elements: std::mem::take(&mut self.elements),
        });
        self.y = 0;
        self.dirty = false;
        if let Some(paper) = self.next_paper.take() {
            self.config.paper = paper;
        }
        receipt
    }

    fn put_char(&mut self, c: char) {
        let style = self.state.style();
        let metrics = font_metrics(style.font);
        let char_width = metrics.width.saturating_mul(style.width_multiplier);
        let char_height = metrics.height.saturating_mul(style.height_multiplier);
        if self.line.has_data && self.line.cursor.saturating_add(char_width) > self.width() {
            self.print_and_feed(self.state.line_spacing);
        }
        let cursor = self.line.cursor;
        match self.line.spans.last_mut() {
            Some(span) if span.style == style && span.x.saturating_add(span.width) == cursor => {
                span.text.push(c);
                span.columns = span.columns.saturating_add(1);
                span.width = span.width.saturating_add(char_width);
            }
            _ => self.line.spans.push(Span {
                text: c.to_string(),
                x: cursor,
                // Final value is set when the line is printed.
                y: metrics.baseline.saturating_mul(style.height_multiplier),
                width: char_width,
                columns: 1,
                char_width,
                char_height,
                style,
            }),
        }
        self.line.cursor = cursor.saturating_add(char_width);
        self.line.has_data = true;
    }

    fn tab(&mut self) {
        let width = self.width();
        if self.line.cursor > width {
            self.print_and_feed(self.state.line_spacing);
        }
        let cursor = self.line.cursor;
        let next = (1..=TAB_STOP_COUNT)
            .map(|k| k.saturating_mul(TAB_INTERVAL_DOTS))
            .find(|&stop| stop > cursor);
        self.line.cursor = match next {
            Some(stop) if stop <= width => stop,
            _ => width.saturating_add(1),
        };
        self.line.has_data = true;
    }

    /// Prints the pending line (if any) and feeds `feed` dots.
    fn print_and_feed(&mut self, feed: u32) {
        let line = std::mem::take(&mut self.line);
        let width = self.width();
        if !line.has_data {
            if feed > 0 {
                self.push_line(Line {
                    y: self.y,
                    height: 0,
                    baseline: 0,
                    advance: feed,
                    alignment: self.state.alignment,
                    content_width: 0,
                    spans: Vec::new(),
                });
            }
            return;
        }
        let height = line.spans.iter().map(|s| s.char_height).max().unwrap_or(0);
        // While pending, `Span::y` holds the span's own baseline.
        let baseline = line.spans.iter().map(|s| s.y).max().unwrap_or(0);
        let content_width = line.cursor.min(width);
        let free = width.saturating_sub(content_width);
        let offset = match self.state.alignment {
            Alignment::Left => 0,
            Alignment::Center => free / 2,
            Alignment::Right => free,
        };
        let spans = line
            .spans
            .into_iter()
            .map(|mut span| {
                span.x = span.x.saturating_add(offset);
                span.y = baseline.saturating_sub(span.y);
                span
            })
            .collect();
        self.push_line(Line {
            y: self.y,
            height,
            baseline,
            advance: feed.max(height),
            alignment: self.state.alignment,
            content_width,
            spans,
        });
    }

    fn push_line(&mut self, line: Line) {
        self.y = self.y.saturating_add(line.advance);
        self.elements.push(Element::Line(line));
        self.dirty = true;
    }
}

/// Builds the receipts and collects the drawer kicks of a complete list of
/// commands.
pub fn render(commands: &[DecodedCommand], config: ReceiptConfig) -> RenderOutput {
    let mut builder = ReceiptBuilder::new(config);
    let mut output = RenderOutput::default();
    for effect in builder.apply_all(commands) {
        output.push(effect);
    }
    output.receipts.extend(builder.finish());
    output
}

impl RenderOutput {
    /// Records one effect.
    pub fn push(&mut self, effect: Effect) {
        match effect {
            Effect::ReceiptCompleted(receipt) => self.receipts.push(receipt),
            Effect::DrawerKick(kick) => self.drawer_kicks.push(kick),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{ParserConfig, parse};

    fn rendered(bytes: &[u8], paper: PaperProfile) -> RenderOutput {
        let commands = parse(bytes, ParserConfig::default());
        render(
            &commands,
            ReceiptConfig {
                paper,
                cr_line_feed: false,
            },
        )
    }

    fn receipt(bytes: &[u8], paper: PaperProfile) -> Receipt {
        let mut out = rendered(bytes, paper);
        assert_eq!(out.receipts.len(), 1, "expected one receipt");
        out.receipts.remove(0)
    }

    fn lines(r: &Receipt) -> Vec<&Line> {
        r.elements
            .iter()
            .map(|e| match e {
                Element::Line(l) => l,
            })
            .collect()
    }

    #[test]
    fn wraps_at_32_columns_on_58mm() {
        let r = receipt(&[b'x'; 33], PaperProfile::Mm58);
        let l = lines(&r);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].spans[0].columns, 32);
        assert_eq!(l[1].spans[0].columns, 1);
        assert_eq!(l[1].y, 30);
    }

    #[test]
    fn exactly_full_line_does_not_add_blank_line() {
        let mut input = vec![b'x'; 48];
        input.push(b'\n');
        let r = receipt(&input, PaperProfile::Mm80);
        assert_eq!(lines(&r).len(), 1);
    }

    #[test]
    fn alignment_ignored_mid_line() {
        let r = receipt(b"ab\x1ba\x01cd\n", PaperProfile::Mm80);
        assert_eq!(lines(&r)[0].alignment, Alignment::Left);
    }

    #[test]
    fn center_offset() {
        let r = receipt(b"\x1ba\x01ab\n", PaperProfile::Mm80);
        assert_eq!(lines(&r)[0].spans[0].x, (576 - 24) / 2);
    }

    #[test]
    fn baseline_alignment_of_mixed_sizes() {
        let r = receipt(b"a\x1d!\x11b\n", PaperProfile::Mm80);
        let l = lines(&r)[0];
        assert_eq!(l.height, 48);
        assert_eq!(l.baseline, 42);
        assert_eq!(l.spans[0].y, 21);
        assert_eq!(l.spans[1].y, 0);
        assert_eq!(l.advance, 48);
    }

    #[test]
    fn initialize_discards_unprinted_line() {
        let r = receipt(b"lost\x1b@kept\n", PaperProfile::Mm80);
        let l = lines(&r);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].spans[0].text, "kept");
    }

    #[test]
    fn tab_moves_to_next_stop() {
        let r = receipt(b"a\tb\n", PaperProfile::Mm80);
        let l = lines(&r)[0];
        assert_eq!(l.spans[1].x, 96);
        assert_eq!(l.content_width, 108);
    }

    #[test]
    fn cut_closes_and_keeps_settings() {
        let out = rendered(b"\x1bE\x01a\n\x1dV\x00b\n\x1dVB\x0a", PaperProfile::Mm80);
        assert_eq!(out.receipts.len(), 2);
        assert_eq!(out.receipts[0].cut, Some(CutMode::Full));
        assert_eq!(out.receipts[0].height, 30);
        let second = lines(&out.receipts[1]);
        assert!(second[0].spans[0].style.bold, "bold survives the cut");
        assert_eq!(second[0].y, 0);
        assert_eq!(out.receipts[1].cut, Some(CutMode::Partial));
        // Function B feeds n dots before cutting.
        assert_eq!(out.receipts[1].height, 40);
    }

    #[test]
    fn cut_mid_line_is_ignored() {
        let out = rendered(b"abc\x1dV\x00def\n", PaperProfile::Mm80);
        assert_eq!(out.receipts.len(), 1);
        assert_eq!(out.receipts[0].cut, None);
        assert_eq!(lines(&out.receipts[0])[0].content_width, 72);
    }

    #[test]
    fn empty_cuts_produce_no_receipt() {
        let out = rendered(b"\x1dV\x00\x1bi\x1dVA\x00", PaperProfile::Mm80);
        assert!(out.receipts.is_empty());
    }

    #[test]
    fn paper_change_waits_for_the_next_receipt() {
        let mut b = ReceiptBuilder::new(ReceiptConfig::default());
        b.set_paper(PaperProfile::Mm58);
        assert_eq!(b.paper(), PaperProfile::Mm58, "empty builder switches now");
        b.apply(&Command::Text { text: "x".into() });
        b.set_paper(PaperProfile::Mm80);
        b.apply(&Command::LineFeed);
        assert_eq!(b.take_update().map(|r| r.width), Some(384));
        assert_eq!(b.take_update(), None, "unchanged");
        let cut = Command::Cut {
            mode: Some(CutMode::Full),
            function: None,
            feed: None,
        };
        match b.apply(&cut) {
            Some(Effect::ReceiptCompleted(r)) => assert_eq!(r.width, 384),
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(b.paper(), PaperProfile::Mm80);
        assert_eq!(b.snapshot(), None);
        assert_eq!(b.finish(), None);
    }

    #[test]
    fn drawer_kick_effect() {
        let out = rendered(b"\x1bp\x0022\x1bp\x07\x01\x01", PaperProfile::Mm80);
        assert_eq!(
            out.drawer_kicks,
            vec![DrawerKick {
                pin: 2,
                on_ms: 100,
                off_ms: 100
            }]
        );
    }
}
