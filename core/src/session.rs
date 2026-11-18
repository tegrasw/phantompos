//! One printer connection: parser + receipt builder + status replies.
//!
//! [`Session`] is what the backend drives: it takes the bytes of each socket
//! read and the current simulated [`PrinterStatus`], and returns the decoded
//! commands (for the log) and, in stream order, the receipts closed by cuts,
//! the drawer kicks and the replies to write back to the host. The backend
//! never needs to look at ESC/POS commands.
//!
//! **Error states.** While the printer is offline (paper out, cover open or
//! the offline switch) received data is logged and discarded: commands are
//! decoded and returned, but nothing is applied to the receipt (text, feeds,
//! images, cuts, styles, `ESC @`, `ESC p`). The receipt in progress before
//! the error stays in progress and continues after recovery. Deliberate
//! deviation (owner decision): a real printer keeps buffered data in its
//! receive buffer and prints it after recovery; PhantomPOS drops it so the
//! developer sees exactly what would be lost if the job were abandoned.
//!
//! Status commands are always processed, in every state: the real-time
//! commands (`DLE EOT`, `DLE ENQ`, `DLE DC4`, processed on reception by a
//! real printer) and also `GS r` and `GS a`. Deliberate deviation: `GS r`
//! and `GS a` are buffered commands that an offline printer would only run
//! after recovery; answering them immediately is more useful when testing
//! how a POS reacts to an error. A drawer kick by `DLE DC4 1` works offline
//! (the reference ignores it only "in error status", and no error is
//! simulated); one by `ESC p` is buffered data and is discarded.
//!
//! Whether a command is discarded depends on the state when its last byte
//! was received (when the printer would execute it), not on the state when
//! the parser emits it: text received offline is discarded even if its
//! terminating `LF` arrives after recovery. So with a fixed state the result
//! never depends on how the stream is split.
//!
//! **Automatic Status Back.** `GS a n` with any status item enabled sends the
//! current 4-byte ASB status at once ("Then transmit the current basic ASB
//! status"); afterwards the status is sent whenever an enabled item changes
//! (see [`crate::status::asb_changed`]). `GS a 0`, `GS a` with only reserved
//! bits, and `ESC @` disable it (the reference: "effective until ESC @ is
//! executed"). The power-on default is disabled. ASB is per connection; the
//! backend calls [`Session::status_changed`] whenever the shared state
//! changes, and every `feed` first catches up with the state it is given.
//!
//! **Receipt byte ranges.** Every [`SessionEvent::ReceiptCompleted`] carries
//! the stream offset where the receipt ends, so the backend can cut the raw
//! stream into one byte range per receipt without looking at ESC/POS: a
//! receipt owns `[end of the previous receipt (or 0), end)`. The range holds
//! everything the host sent in between, including status requests, empty
//! cuts (a cut with nothing on paper closes no receipt, so its bytes belong
//! to the next one) and data discarded in an error state: the range is "what
//! the host sent", not "what was printed". Bytes after the last receipt
//! belong to no receipt when the stream ends without content. Replaying a
//! range on its own reproduces the receipt only when it does not depend on
//! state set by an earlier receipt (styles, code page, line spacing...): a
//! cut is not a reset, and that inherited state is not part of the range.

use std::collections::VecDeque;

use crate::command::{Command, DecodedCommand};
use crate::paper::PaperProfile;
use crate::parser::{Parser, ParserConfig};
use crate::receipt::{DrawerKick, Effect, Receipt, ReceiptBuilder, ReceiptConfig};
use crate::status::{
    ASB_MASK_BITS, ASB_MNEMONIC, PrinterStatus, StatusReply, SuppressReason, asb_changed,
    asb_reply, request_mnemonic, status_reply,
};

/// A reply to write back to the host, with the request it answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    /// Mnemonic of the request (`"DLE EOT 1"`, `"GS r 2"`), or `"ASB"` for
    /// Automatic Status Back.
    pub request: String,
    pub reply: StatusReply,
}

/// Something a batch did besides being logged, in stream order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    /// A cut closed a non-empty receipt, or (from [`Session::finish`]) the
    /// stream ended with content pending (`cut: None`).
    ReceiptCompleted {
        receipt: Receipt,
        /// Stream offset just after the receipt's last byte: the end of the
        /// cut command that closed it (exclusive), or the stream length when
        /// [`Session::finish`] closed it. See "Receipt byte ranges" in the
        /// module docs.
        end: u64,
    },
    /// The cash drawer was kicked (`ESC p` or `DLE DC4 1`); the session
    /// already considers the drawer open.
    DrawerKick(DrawerKick),
    /// Bytes to send to the host.
    Reply(Reply),
}

/// Data of a batch that was received in an error state and not printed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Suppressed {
    /// The error state; with several causes, see
    /// [`PrinterStatus::suppress_reason`].
    pub reason: SuppressReason,
    /// Number of bytes of the batch's commands that were discarded.
    pub bytes: u64,
}

/// What one [`Session::feed`] (or [`Session::finish`]) produced.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionOutput {
    /// Every command decoded in this batch, discarded ones included.
    pub commands: Vec<DecodedCommand>,
    pub events: Vec<SessionEvent>,
    /// `Some` when commands of this batch were discarded.
    pub suppressed: Option<Suppressed>,
}

/// Parser, receipt builder and status state of one connection.
#[derive(Debug, Clone)]
pub struct Session {
    parser: Parser,
    builder: ReceiptBuilder,
    /// `GS a` mask (status items enabled; 0 = ASB disabled).
    asb_mask: u8,
    /// The state this session last saw.
    status: PrinterStatus,
    /// Stream ranges not yet decoded and the error state they were received
    /// in: `(end offset, exclusive; reason)`, in stream order.
    segments: VecDeque<(u64, Option<SuppressReason>)>,
}

impl Session {
    pub fn new(parser: ParserConfig, receipt: ReceiptConfig, status: PrinterStatus) -> Self {
        Self {
            parser: Parser::new(parser),
            builder: ReceiptBuilder::new(receipt),
            asb_mask: 0,
            status,
            segments: VecDeque::new(),
        }
    }

    /// The `GS a` mask in effect (0 when ASB is disabled).
    pub fn asb_mask(&self) -> u8 {
        self.asb_mask
    }

    /// The state the session last saw (including drawer kicks it executed).
    pub fn status(&self) -> PrinterStatus {
        self.status
    }

    /// See [`ReceiptBuilder::set_paper`].
    pub fn set_paper(&mut self, paper: PaperProfile) {
        self.builder.set_paper(paper);
    }

    pub fn paper(&self) -> PaperProfile {
        self.builder.paper()
    }

    /// See [`ReceiptBuilder::take_update`].
    pub fn take_update(&mut self) -> Option<Receipt> {
        self.builder.take_update()
    }

    /// See [`ReceiptBuilder::snapshot`].
    pub fn snapshot(&self) -> Option<Receipt> {
        self.builder.snapshot()
    }

    /// Bytes held by the parser (see [`Parser::pending_bytes`]).
    pub fn pending_bytes(&self) -> usize {
        self.parser.pending_bytes()
    }

    /// The shared state changed: records it and returns the ASB status to
    /// send if ASB is enabled and an enabled item changed.
    pub fn status_changed(&mut self, status: PrinterStatus) -> Option<Reply> {
        let old = std::mem::replace(&mut self.status, status);
        (self.asb_mask != 0 && asb_changed(self.asb_mask, &old, &status)).then(|| Reply {
            request: ASB_MNEMONIC.to_owned(),
            reply: asb_reply(&status),
        })
    }

    /// Processes one chunk of the stream received while the printer was in
    /// state `status`.
    pub fn feed(&mut self, bytes: &[u8], status: PrinterStatus) -> SessionOutput {
        let mut output = SessionOutput::default();
        if let Some(reply) = self.status_changed(status) {
            output.events.push(SessionEvent::Reply(reply));
        }
        if !bytes.is_empty() {
            let len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
            let end = self.parser.bytes_consumed().saturating_add(len);
            let reason = status.suppress_reason();
            match self.segments.back_mut() {
                Some(last) if last.1 == reason => last.0 = end,
                _ => self.segments.push_back((end, reason)),
            }
        }
        let commands = self.parser.feed(bytes);
        self.process(&commands, &mut output);
        output.commands = commands;
        self.prune();
        output
    }

    /// Ends the stream: flushes the parser (pending text, incomplete
    /// command), then closes the receipt in progress (`cut: None`) if it has
    /// content. The receipt is completed whatever the state, since its
    /// content was accepted before any error.
    pub fn finish(&mut self, status: PrinterStatus) -> SessionOutput {
        let mut output = SessionOutput::default();
        if let Some(reply) = self.status_changed(status) {
            output.events.push(SessionEvent::Reply(reply));
        }
        let commands = self.parser.finish();
        self.process(&commands, &mut output);
        output.commands = commands;
        self.segments.clear();
        if let Some(receipt) = self.builder.finish() {
            output.events.push(SessionEvent::ReceiptCompleted {
                receipt,
                end: self.parser.bytes_consumed(),
            });
        }
        output
    }

    fn process(&mut self, commands: &[DecodedCommand], output: &mut SessionOutput) {
        for decoded in commands {
            let command = &decoded.command;
            let len = u64::try_from(decoded.bytes.len()).unwrap_or(u64::MAX);
            let end = decoded.offset.saturating_add(len);
            if is_status_command(command) {
                self.status_command(command, end, output);
                continue;
            }
            let last = end.saturating_sub(1);
            if let Some(reason) = self.reason_at(last) {
                let bytes = output.suppressed.map_or(0, |s| s.bytes).saturating_add(len);
                output.suppressed = Some(Suppressed { reason, bytes });
                continue;
            }
            if matches!(command, Command::Initialize) {
                self.asb_mask = 0;
            }
            self.apply(command, end, output);
        }
    }

    fn status_command(&mut self, command: &Command, end: u64, output: &mut SessionOutput) {
        match command {
            Command::SetAutomaticStatusBack { n, .. } => {
                self.asb_mask = n & ASB_MASK_BITS;
                if self.asb_mask != 0 {
                    output.events.push(SessionEvent::Reply(Reply {
                        request: ASB_MNEMONIC.to_owned(),
                        reply: asb_reply(&self.status),
                    }));
                }
            }
            _ => {
                if let (Some(request), Some(reply)) = (
                    request_mnemonic(command),
                    status_reply(command, &self.status),
                ) {
                    output
                        .events
                        .push(SessionEvent::Reply(Reply { request, reply }));
                }
                // Drawer pulse (DLE DC4 1) and buffer clear (DLE DC4 8).
                self.apply(command, end, output);
            }
        }
    }

    /// Applies one command whose last byte ends at stream offset `end`
    /// (exclusive) to the receipt.
    fn apply(&mut self, command: &Command, end: u64, output: &mut SessionOutput) {
        match self.builder.apply(command) {
            Some(Effect::ReceiptCompleted(receipt)) => {
                output
                    .events
                    .push(SessionEvent::ReceiptCompleted { receipt, end });
            }
            Some(Effect::DrawerKick(kick)) => {
                output.events.push(SessionEvent::DrawerKick(kick));
                let opened = PrinterStatus {
                    drawer_open: true,
                    ..self.status
                };
                if let Some(reply) = self.status_changed(opened) {
                    output.events.push(SessionEvent::Reply(reply));
                }
            }
            None => {}
        }
    }

    /// Error state in which the byte at `offset` was received.
    fn reason_at(&self, offset: u64) -> Option<SuppressReason> {
        self.segments
            .iter()
            .find(|(end, _)| *end > offset)
            .map_or_else(|| self.status.suppress_reason(), |(_, reason)| *reason)
    }

    /// Forgets the segments every future command starts after.
    fn prune(&mut self) {
        let pending = u64::try_from(self.parser.pending_bytes()).unwrap_or(u64::MAX);
        let first_pending = self.parser.bytes_consumed().saturating_sub(pending);
        while self
            .segments
            .front()
            .is_some_and(|(end, _)| *end <= first_pending)
        {
            self.segments.pop_front();
        }
    }
}

/// Commands processed in every printer state (see the module docs).
pub fn is_status_command(command: &Command) -> bool {
    matches!(
        command,
        Command::RealTimeStatus { .. }
            | Command::TransmitStatus { .. }
            | Command::SetAutomaticStatusBack { .. }
            | Command::RealTimeRequest { .. }
            | Command::RealTimePulse { .. }
            | Command::PowerOff { .. }
            | Command::SoundBuzzer { .. }
            | Command::TransmitSpecifiedStatus { .. }
            | Command::ClearBuffers { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::{ReplyKind, StatusFlag};

    const NORMAL: PrinterStatus = PrinterStatus {
        paper_out: false,
        cover_open: false,
        offline: false,
        drawer_open: false,
    };
    const PAPER_OUT: PrinterStatus = PrinterStatus {
        paper_out: true,
        ..NORMAL
    };

    fn session() -> Session {
        Session::new(
            ParserConfig::default(),
            ReceiptConfig::default(),
            PrinterStatus::default(),
        )
    }

    fn replies(output: &SessionOutput) -> Vec<(String, Vec<u8>)> {
        output
            .events
            .iter()
            .filter_map(|e| match e {
                SessionEvent::Reply(r) => Some((r.request.clone(), r.reply.bytes.clone())),
                _ => None,
            })
            .collect()
    }

    fn receipts(output: &SessionOutput) -> Vec<Receipt> {
        output
            .events
            .iter()
            .filter_map(|e| match e {
                SessionEvent::ReceiptCompleted { receipt, .. } => Some(receipt.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn requests_are_answered_in_stream_order() {
        let mut s = session();
        let out = s.feed(b"\x10\x04\x01\x1dr\x01\x10\x04\x04", PAPER_OUT);
        assert_eq!(
            replies(&out),
            vec![
                ("DLE EOT 1".into(), vec![0x1A]),
                ("GS r 1".into(), vec![0x0C]),
                ("DLE EOT 4".into(), vec![0x72]),
            ]
        );
        assert!(
            out.suppressed.is_none(),
            "status commands are not discarded"
        );
    }

    #[test]
    fn error_state_discards_data_and_recovery_continues_the_receipt() {
        let mut s = session();
        let out = s.feed(b"before\n", NORMAL);
        assert!(out.suppressed.is_none());
        assert!(s.take_update().is_some());
        // Text and a cut sent while the paper is out: nothing printed.
        let out = s.feed(b"lost\n\x1dV\x00", PAPER_OUT);
        assert_eq!(out.commands.len(), 3);
        assert_eq!(
            out.suppressed,
            Some(Suppressed {
                reason: SuppressReason::PaperOut,
                bytes: 8
            })
        );
        assert!(receipts(&out).is_empty());
        assert!(s.take_update().is_none());
        // Text received offline whose LF arrives after recovery is lost too.
        let out = s.feed(b"half", PAPER_OUT);
        assert!(out.commands.is_empty());
        let out = s.feed(b"\nafter\n\x1dV\x01", NORMAL);
        assert_eq!(out.suppressed.map(|s| s.bytes), Some(4));
        let done = receipts(&out);
        assert_eq!(done.len(), 1);
        let texts: Vec<_> = done[0]
            .elements
            .iter()
            .filter_map(|e| match e {
                crate::Element::Line(l) => l.spans.first().map(|s| s.text.clone()),
                crate::Element::Image(_) => None,
            })
            .collect();
        assert_eq!(texts, vec!["before", "after"]);
    }

    #[test]
    fn reason_priority_is_cover_then_paper_then_offline() {
        let all = PrinterStatus {
            paper_out: true,
            cover_open: true,
            offline: true,
            drawer_open: false,
        };
        let mut s = session();
        let out = s.feed(b"x\n", all);
        assert_eq!(
            out.suppressed.map(|s| s.reason),
            Some(SuppressReason::CoverOpen)
        );
        let out = s.feed(
            b"x\n",
            PrinterStatus {
                cover_open: false,
                ..all
            },
        );
        assert_eq!(
            out.suppressed.map(|s| s.reason),
            Some(SuppressReason::PaperOut)
        );
        let out = s.feed(
            b"x\n",
            PrinterStatus {
                offline: true,
                ..NORMAL
            },
        );
        assert_eq!(
            out.suppressed.map(|s| s.reason),
            Some(SuppressReason::Offline)
        );
    }

    #[test]
    fn drawer_kicks_open_the_drawer_for_later_requests() {
        let mut s = session();
        let out = s.feed(b"\x1dr\x02\x1bp\x00\x19\xfa\x1dr\x02\x10\x04\x01", NORMAL);
        assert_eq!(
            replies(&out),
            vec![
                ("GS r 2".into(), vec![0x00]),
                ("GS r 2".into(), vec![0x01]),
                ("DLE EOT 1".into(), vec![0x16]),
            ]
        );
        assert!(s.status().drawer_open);
        // ESC p is discarded offline; DLE DC4 1 is executed.
        let mut s = session();
        let out = s.feed(b"\x1bp\x00\x19\xfa", PAPER_OUT);
        assert!(out.events.is_empty());
        let out = s.feed(b"\x10\x14\x01\x01\x03", PAPER_OUT);
        assert_eq!(
            out.events,
            vec![SessionEvent::DrawerKick(DrawerKick {
                pin: 5,
                on_ms: 300,
                off_ms: 300
            })]
        );
    }

    #[test]
    fn asb_is_sent_on_enable_and_on_enabled_changes_only() {
        let mut s = session();
        let out = s.feed(b"\x1da\x02", NORMAL);
        assert_eq!(replies(&out), vec![("ASB".into(), vec![0x10, 0, 0, 0])]);
        assert_eq!(s.asb_mask(), 0x02);
        // Drawer item not enabled: no ASB.
        assert!(
            s.status_changed(PrinterStatus {
                drawer_open: true,
                ..NORMAL
            })
            .is_none()
        );
        let reply = s.status_changed(PrinterStatus {
            cover_open: true,
            drawer_open: true,
            ..NORMAL
        });
        let reply = reply.expect("cover change is reported");
        assert_eq!(reply.reply.bytes, vec![0x3C, 0, 0, 0]);
        assert_eq!(reply.reply.kind, ReplyKind::Automatic);
        assert!(reply.reply.flags.contains(&StatusFlag::CoverOpen));
        // feed() catches up with a change it was not told about.
        let out = s.feed(b"", NORMAL);
        assert_eq!(replies(&out), vec![("ASB".into(), vec![0x10, 0, 0, 0])]);
        // ESC @ disables ASB (not while offline: it is discarded then).
        s.feed(b"\x1b@", PAPER_OUT);
        assert_eq!(s.asb_mask(), 0x02);
        s.feed(b"\x1b@", NORMAL);
        assert_eq!(s.asb_mask(), 0);
        assert!(s.status_changed(PAPER_OUT).is_none());
        // Reserved bits only: disabled, nothing sent.
        let out = s.feed(b"\x1da\x30", NORMAL);
        assert!(replies(&out).is_empty());
    }

    #[test]
    fn own_drawer_kick_sends_asb_once() {
        let mut s = session();
        s.feed(b"\x1da\x01", NORMAL);
        let out = s.feed(b"\x1bp\x00\x01\x01", NORMAL);
        assert_eq!(replies(&out), vec![("ASB".into(), vec![0x14, 0, 0, 0])]);
        // The backend then publishes drawerOpen: no second ASB.
        assert!(
            s.status_changed(PrinterStatus {
                drawer_open: true,
                ..NORMAL
            })
            .is_none()
        );
    }

    #[test]
    fn clear_buffers_discards_the_pending_line_and_replies() {
        let mut s = session();
        let out = s.feed(
            b"kept\npending\x10\x14\x08\x01\x03\x14\x01\x06\x02\x08\n",
            NORMAL,
        );
        assert_eq!(
            replies(&out),
            vec![("DLE DC4 8".into(), vec![0x37, 0x25, 0x00])]
        );
        let receipt = s.finish(NORMAL);
        let done = receipts(&receipt);
        assert_eq!(done.len(), 1);
        // "kept" and the blank line of the final LF; "pending" is gone.
        assert_eq!(done[0].elements.len(), 2);
    }

    fn ends(output: &SessionOutput) -> Vec<u64> {
        output
            .events
            .iter()
            .filter_map(|e| match e {
                SessionEvent::ReceiptCompleted { end, .. } => Some(*end),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn completed_receipts_report_where_they_end_in_the_stream() {
        let mut s = session();
        // An empty cut (nothing on paper) closes no receipt; its bytes
        // belong to the next one. GS V 66 n is 4 bytes.
        let first = b"\x1dV\x00a\n\x1dVB\x03";
        let second = b"b\n\x1bi";
        let mut stream = first.to_vec();
        stream.extend_from_slice(second);
        stream.extend_from_slice(b"\x10\x04\x01tail");
        // Split in the middle of the second cut: offsets are stream offsets.
        let out = s.feed(&stream[..8], NORMAL);
        assert!(ends(&out).is_empty());
        let out = s.feed(&stream[8..], NORMAL);
        assert_eq!(ends(&out), vec![9, 9 + 4]);
        // The pending "tail" is closed by finish, at the stream length.
        let out = s.finish(NORMAL);
        assert_eq!(ends(&out), vec![u64::try_from(stream.len()).unwrap()]);

        // Nothing pending at the end: no receipt, no range.
        let mut s = session();
        s.feed(b"x\n\x1dV\x00\x10\x04\x01", NORMAL);
        assert!(ends(&s.finish(NORMAL)).is_empty());
    }

    #[test]
    fn data_discarded_in_an_error_state_stays_inside_the_range() {
        let mut s = session();
        s.feed(b"a\n", NORMAL);
        s.feed(b"lost\n\x1dV\x00", PAPER_OUT);
        let out = s.feed(b"\x1dV\x01", NORMAL);
        // The suppressed cut closed nothing; the receipt ends after GS V 1.
        assert_eq!(ends(&out), vec![2 + 8 + 3]);
    }
}
