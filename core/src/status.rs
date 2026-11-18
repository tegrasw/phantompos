//! Simulated printer state and the status bytes the printer sends back.
//!
//! Everything here is a pure function of a [`PrinterStatus`]: the backend
//! keeps the state, `phantompos-core` says which bytes a status request is
//! answered with. Every bit follows the Epson ESC/POS reference (pages
//! `dle_eot.html`, `gs_lr.html`, `gs_la.html`, `dle_dc4_fn7.html` and
//! `dle_dc4_fn8.html` of
//! <https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/>).
//!
//! How the simulated states map to the reference:
//!
//! * **Offline** (bit 3 of `DLE EOT 1` and of the first ASB byte) is set by
//!   any of `paperOut`, `coverOpen` and `offline`: a TM printer goes offline
//!   when the cover is open or printing stopped at a paper end. The `offline`
//!   switch is an offline state with no other cause, so `DLE EOT 2` reports
//!   no cause bit for it.
//! * **Cover open**: bit 2 of `DLE EOT 2`, bit 5 of the first ASB byte.
//! * **Paper out**: roll paper end sensor "paper not present" (`DLE EOT 4`
//!   bits 5-6, `GS r 1` bits 2-3, ASB third byte bits 2-3) and "printing stops
//!   due to a paper-end" (`DLE EOT 2` bit 5). Several models note that they
//!   do not execute `GS r 1` at a paper end (so it never reports it); this
//!   emulator answers it with the paper-end bits set instead.
//! * **Drawer open** is reported as "drawer kick-out connector pin 3 HIGH"
//!   (`DLE EOT 1` bit 2, `GS r 2` bit 0, ASB first byte bit 2). The level a
//!   real drawer produces when open depends on the drawer's switch wiring:
//!   some drawers pull pin 3 HIGH when open, others when closed. HIGH = open
//!   is the convention chosen for PhantomPOS.
//! * Not simulated, always reported inactive: the roll paper near-end sensor
//!   ("paper adequate"), the paper feed button, online recovery wait, and
//!   every error (recoverable, autocutter, unrecoverable, auto-recoverable,
//!   `DLE EOT 2` bit 6 "error occurred").
//! * Fixed bits have the values of the reference; reserved bits are sent as
//!   0 (the reference leaves their value undefined).
//!
//! Inconsistencies found in the reference: the `DLE EOT 1` table gives bit 6
//! as "Hex 04, Decimal 64" (0x40 is used, matching the decimal value and the
//! bit position); the `GS r` notes say a status can be recognized as
//! `0xx1xx10b`, but its bit tables define bit 4 as fixed 0 and bits 0-1 as
//! sensor bits (the tables are followed: paper present, adequate = `0x00`).

use serde::{Deserialize, Serialize};

use crate::command::Command;

/// Simulated printer state, set from the UI (and `drawerOpen` also by drawer
/// kicks). Partial updates use [`PrinterStatusPatch`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrinterStatus {
    /// The roll paper end sensor detects no paper; printing stopped.
    pub paper_out: bool,
    /// The roll paper cover is open.
    pub cover_open: bool,
    /// Offline with no other cause (the "offline" switch).
    pub offline: bool,
    /// The cash drawer is open (drawer kick-out connector pin 3 HIGH).
    pub drawer_open: bool,
}

/// A partial [`PrinterStatus`]: fields that are present replace the current
/// value (`set_printer_status` takes this shape).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrinterStatusPatch {
    #[serde(default)]
    pub paper_out: Option<bool>,
    #[serde(default)]
    pub cover_open: Option<bool>,
    #[serde(default)]
    pub offline: Option<bool>,
    #[serde(default)]
    pub drawer_open: Option<bool>,
}

impl PrinterStatus {
    /// The state with the fields of `patch` that are set.
    #[must_use]
    pub fn merged(self, patch: PrinterStatusPatch) -> Self {
        Self {
            paper_out: patch.paper_out.unwrap_or(self.paper_out),
            cover_open: patch.cover_open.unwrap_or(self.cover_open),
            offline: patch.offline.unwrap_or(self.offline),
            drawer_open: patch.drawer_open.unwrap_or(self.drawer_open),
        }
    }

    /// True when the printer is offline for any reason.
    pub fn is_offline(&self) -> bool {
        self.paper_out || self.cover_open || self.offline
    }

    /// Why received data is not printed, or `None` when the printer prints.
    /// With several causes the most physical one wins: cover open, then
    /// paper out, then the offline switch.
    pub fn suppress_reason(&self) -> Option<SuppressReason> {
        if self.cover_open {
            Some(SuppressReason::CoverOpen)
        } else if self.paper_out {
            Some(SuppressReason::PaperOut)
        } else if self.offline {
            Some(SuppressReason::Offline)
        } else {
            None
        }
    }
}

/// Why data received in an error state was discarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SuppressReason {
    CoverOpen,
    PaperOut,
    Offline,
}

/// How a reply reached the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReplyKind {
    /// Answer to a real-time command (`DLE EOT`, `DLE DC4`).
    RealTime,
    /// Answer to a buffered status command (`GS r`).
    Transmit,
    /// Automatic Status Back (`GS a`), sent without a request.
    Automatic,
}

/// A condition a reply reports as active. Serialized as stable camelCase
/// codes for the UI (which translates them); see `core/README.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StatusFlag {
    Online,
    Offline,
    /// Drawer kick-out connector pin 3 HIGH (drawer open in PhantomPOS).
    DrawerHigh,
    /// Drawer kick-out connector pin 3 LOW (drawer closed).
    DrawerLow,
    CoverOpen,
    CoverClosed,
    /// Waiting for online recovery (never simulated).
    WaitingForOnlineRecovery,
    /// The paper feed button is pressed (never simulated).
    FeedButtonPressed,
    /// Paper is being fed by the paper feed button (never simulated).
    PaperFedByButton,
    /// Printing stopped due to a paper end (`DLE EOT 2`).
    PaperEndStop,
    /// `DLE EOT 2` bit 6 (never simulated).
    ErrorOccurred,
    RecoverableError,
    AutocutterError,
    UnrecoverableError,
    AutoRecoverableError,
    /// None of the four error kinds above.
    NoError,
    /// Roll paper end sensor: paper present.
    PaperPresent,
    /// Roll paper end sensor: paper not present.
    PaperEnd,
    /// Roll paper near-end sensor: paper adequate.
    PaperAdequate,
    /// Roll paper near-end sensor: paper near end (never simulated).
    PaperNearEnd,
    /// `DLE DC4 8`: the receive and print buffers were cleared.
    BuffersCleared,
}

/// Bytes the printer sends back, with what they report.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusReply {
    pub kind: ReplyKind,
    pub bytes: Vec<u8>,
    /// Conditions reported as active, decoded from `bytes`.
    pub flags: Vec<StatusFlag>,
}

/// Which status byte layout a reply uses (each has its own bit meanings).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layout {
    PrinterStatus,
    OfflineCause,
    ErrorCause,
    RollPaperSensor,
    PaperSensor,
    DrawerConnector,
}

/// Fixed bits of the `DLE EOT` status bytes: bit 1 = 1, bit 4 = 1 (bits 0
/// and 7 = 0), i.e. `0xx1xx10b`.
const DLE_EOT_FIXED: u8 = 0x12;

// DLE EOT 1 - printer status.
const PRINTER_DRAWER_HIGH: u8 = 0x04;
const PRINTER_OFFLINE: u8 = 0x08;
const PRINTER_WAITING_RECOVERY: u8 = 0x20;
const PRINTER_FEED_BUTTON: u8 = 0x40;
// DLE EOT 2 - offline cause status.
const OFFLINE_COVER_OPEN: u8 = 0x04;
const OFFLINE_FEEDING_BY_BUTTON: u8 = 0x08;
const OFFLINE_PAPER_END_STOP: u8 = 0x20;
const OFFLINE_ERROR: u8 = 0x40;
// DLE EOT 3 - error cause status.
const ERROR_RECOVERABLE: u8 = 0x04;
const ERROR_AUTOCUTTER: u8 = 0x08;
const ERROR_UNRECOVERABLE: u8 = 0x20;
const ERROR_AUTO_RECOVERABLE: u8 = 0x40;
// DLE EOT 4 - roll paper sensor status.
const ROLL_NEAR_END: u8 = 0x0C;
const ROLL_PAPER_END: u8 = 0x60;
// GS r 1 - paper sensor status (bit 4 and 7 fixed 0).
const SENSOR_NEAR_END: u8 = 0x03;
const SENSOR_PAPER_END: u8 = 0x0C;
// GS r 2 - drawer kick-out connector status.
const CONNECTOR_PIN3_HIGH: u8 = 0x01;

// Basic ASB status (4 bytes). First byte: bit 4 fixed 1, bits 0, 1, 7 fixed 0.
const ASB1_FIXED: u8 = 0x10;
const ASB1_DRAWER_HIGH: u8 = 0x04;
const ASB1_OFFLINE: u8 = 0x08;
const ASB1_COVER_OPEN: u8 = 0x20;
const ASB1_FEEDING_BY_BUTTON: u8 = 0x40;
// Second byte.
const ASB2_WAITING_RECOVERY: u8 = 0x01;
const ASB2_FEED_BUTTON: u8 = 0x02;
const ASB2_RECOVERABLE: u8 = 0x04;
const ASB2_AUTOCUTTER: u8 = 0x08;
const ASB2_UNRECOVERABLE: u8 = 0x20;
const ASB2_AUTO_RECOVERABLE: u8 = 0x40;
// Third byte.
const ASB3_NEAR_END: u8 = 0x03;
const ASB3_PAPER_END: u8 = 0x0C;

/// `DLE DC4 8` Clear response: header 37h, identifier 25h, NUL.
pub const CLEAR_RESPONSE: [u8; 3] = [0x37, 0x25, 0x00];

/// `GS a n` bits that select status items (bits 4, 5 and 7 are reserved).
pub const ASB_MASK_BITS: u8 = 0b0100_1111;

/// Bits of the 4-byte ASB status covered by each `GS a` enable bit, from the
/// table in the `GS a` notes. Bit 1 (online/offline) also covers the paper
/// sensor bits of the third byte, valid when the sensor stops printing
/// (`ESC c 4`), which is always the case for the simulated paper end.
const ASB_ITEMS: [(u8, [u8; 4]); 5] = [
    // Drawer kick-out connector status.
    (0x01, [ASB1_DRAWER_HIGH, 0, 0, 0]),
    // Online/offline status.
    (
        0x02,
        [
            ASB1_OFFLINE | ASB1_COVER_OPEN | ASB1_FEEDING_BY_BUTTON,
            ASB2_WAITING_RECOVERY,
            ASB3_NEAR_END | ASB3_PAPER_END,
            0,
        ],
    ),
    // Error status.
    (
        0x04,
        [
            0,
            ASB2_RECOVERABLE | ASB2_AUTOCUTTER | ASB2_UNRECOVERABLE | ASB2_AUTO_RECOVERABLE,
            0,
            0,
        ],
    ),
    // Roll paper sensor status.
    (0x08, [0, 0, ASB3_NEAR_END | ASB3_PAPER_END, 0]),
    // Panel switch status.
    (0x40, [0, ASB2_FEED_BUTTON, 0, 0]),
];

fn set(condition: bool, bits: u8) -> u8 {
    if condition { bits } else { 0 }
}

/// `DLE EOT 1`: printer status.
pub fn printer_status_byte(status: &PrinterStatus) -> u8 {
    DLE_EOT_FIXED
        | set(status.drawer_open, PRINTER_DRAWER_HIGH)
        | set(status.is_offline(), PRINTER_OFFLINE)
}

/// `DLE EOT 2`: offline cause status.
pub fn offline_cause_byte(status: &PrinterStatus) -> u8 {
    DLE_EOT_FIXED
        | set(status.cover_open, OFFLINE_COVER_OPEN)
        | set(status.paper_out, OFFLINE_PAPER_END_STOP)
}

/// `DLE EOT 3`: error cause status (no error is simulated).
pub fn error_cause_byte(_status: &PrinterStatus) -> u8 {
    DLE_EOT_FIXED
}

/// `DLE EOT 4`: roll paper sensor status (the near-end sensor always reports
/// paper adequate).
pub fn roll_paper_sensor_byte(status: &PrinterStatus) -> u8 {
    DLE_EOT_FIXED | set(status.paper_out, ROLL_PAPER_END)
}

/// `GS r 1` / `GS r 49`: paper sensor status.
pub fn paper_sensor_byte(status: &PrinterStatus) -> u8 {
    set(status.paper_out, SENSOR_PAPER_END)
}

/// `GS r 2` / `GS r 50`: drawer kick-out connector status.
pub fn drawer_connector_byte(status: &PrinterStatus) -> u8 {
    set(status.drawer_open, CONNECTOR_PIN3_HIGH)
}

/// The 4-byte basic ASB status for `status`. Its content never depends on
/// which items `GS a` enabled: "each status transmission represents the
/// current status" (`GS a` notes); the enabled items only decide when it is
/// sent (see [`asb_changed`]).
pub fn automatic_status(status: &PrinterStatus) -> [u8; 4] {
    [
        ASB1_FIXED
            | set(status.drawer_open, ASB1_DRAWER_HIGH)
            | set(status.is_offline(), ASB1_OFFLINE)
            | set(status.cover_open, ASB1_COVER_OPEN),
        0,
        set(status.paper_out, ASB3_PAPER_END),
        0,
    ]
}

/// Whether a printer whose `GS a` mask is `mask` transmits its ASB status
/// when the state changes from `old` to `new`: true when a status item the
/// mask enables changed.
pub fn asb_changed(mask: u8, old: &PrinterStatus, new: &PrinterStatus) -> bool {
    let (before, after) = (automatic_status(old), automatic_status(new));
    ASB_ITEMS
        .iter()
        .filter(|(bit, _)| mask & bit != 0)
        .any(|(_, bits)| {
            bits.iter()
                .zip(before.iter().zip(after.iter()))
                .any(|(relevant, (b, a))| (b ^ a) & relevant != 0)
        })
}

/// The ASB status of `status` as an automatic reply.
pub fn asb_reply(status: &PrinterStatus) -> StatusReply {
    let bytes = automatic_status(status);
    StatusReply {
        kind: ReplyKind::Automatic,
        flags: asb_flags(&bytes),
        bytes: bytes.to_vec(),
    }
}

/// Reply to a status command, or `None` when the command is not answered
/// (not a status request, a parameter outside the reference, or a status
/// this printer does not have: ink, peeler, interface, DM-D).
pub fn status_reply(command: &Command, status: &PrinterStatus) -> Option<StatusReply> {
    let (kind, bytes, layout) = match command {
        Command::RealTimeStatus { n, a, .. } => {
            let (byte, layout) = match (n, a) {
                (1, None) => (printer_status_byte(status), Layout::PrinterStatus),
                (2, None) => (offline_cause_byte(status), Layout::OfflineCause),
                (3, None) => (error_cause_byte(status), Layout::ErrorCause),
                (4, None) => (roll_paper_sensor_byte(status), Layout::RollPaperSensor),
                _ => return None,
            };
            (ReplyKind::RealTime, vec![byte], layout)
        }
        Command::TransmitStatus { n } => {
            let (byte, layout) = match n {
                1 | 49 => (paper_sensor_byte(status), Layout::PaperSensor),
                2 | 50 => (drawer_connector_byte(status), Layout::DrawerConnector),
                _ => return None,
            };
            (ReplyKind::Transmit, vec![byte], layout)
        }
        Command::TransmitSpecifiedStatus { m: 1 } => {
            let bytes = automatic_status(status);
            return Some(StatusReply {
                kind: ReplyKind::RealTime,
                flags: asb_flags(&bytes),
                bytes: bytes.to_vec(),
            });
        }
        Command::ClearBuffers { valid: true } => {
            return Some(StatusReply {
                kind: ReplyKind::RealTime,
                bytes: CLEAR_RESPONSE.to_vec(),
                flags: vec![StatusFlag::BuffersCleared],
            });
        }
        _ => return None,
    };
    let flags = bytes
        .first()
        .map(|&b| byte_flags(layout, b))
        .unwrap_or_default();
    Some(StatusReply { kind, bytes, flags })
}

/// Short name of a status request for the log (`"DLE EOT 1"`, `"GS r 49"`),
/// or `None` for other commands.
pub fn request_mnemonic(command: &Command) -> Option<String> {
    Some(match command {
        Command::RealTimeStatus { n, a: Some(a), .. } => format!("DLE EOT {n} {a}"),
        Command::RealTimeStatus { n, a: None, .. } => format!("DLE EOT {n}"),
        Command::TransmitStatus { n } => format!("GS r {n}"),
        Command::TransmitSpecifiedStatus { m } => format!("DLE DC4 7 {m}"),
        Command::ClearBuffers { .. } => "DLE DC4 8".to_owned(),
        _ => return None,
    })
}

/// Mnemonic of automatic (ASB) replies in the log.
pub const ASB_MNEMONIC: &str = "ASB";

fn pick(flags: &mut Vec<StatusFlag>, condition: bool, yes: StatusFlag, no: Option<StatusFlag>) {
    if condition {
        flags.push(yes);
    } else if let Some(no) = no {
        flags.push(no);
    }
}

fn error_flags(flags: &mut Vec<StatusFlag>, byte: u8, bits: [u8; 4]) {
    let [recoverable, autocutter, unrecoverable, auto] = bits;
    let errors = [
        (recoverable, StatusFlag::RecoverableError),
        (autocutter, StatusFlag::AutocutterError),
        (unrecoverable, StatusFlag::UnrecoverableError),
        (auto, StatusFlag::AutoRecoverableError),
    ];
    let mut any = false;
    for (bit, flag) in errors {
        if byte & bit != 0 {
            flags.push(flag);
            any = true;
        }
    }
    if !any {
        flags.push(StatusFlag::NoError);
    }
}

/// Flags of a one-byte status, decoded from its bits.
fn byte_flags(layout: Layout, byte: u8) -> Vec<StatusFlag> {
    use StatusFlag as F;
    let mut flags = Vec::new();
    let on = |bits: u8| byte & bits == bits;
    match layout {
        Layout::PrinterStatus => {
            pick(&mut flags, on(PRINTER_OFFLINE), F::Offline, Some(F::Online));
            pick(
                &mut flags,
                on(PRINTER_DRAWER_HIGH),
                F::DrawerHigh,
                Some(F::DrawerLow),
            );
            pick(
                &mut flags,
                on(PRINTER_WAITING_RECOVERY),
                F::WaitingForOnlineRecovery,
                None,
            );
            pick(
                &mut flags,
                on(PRINTER_FEED_BUTTON),
                F::FeedButtonPressed,
                None,
            );
        }
        Layout::OfflineCause => {
            pick(
                &mut flags,
                on(OFFLINE_COVER_OPEN),
                F::CoverOpen,
                Some(F::CoverClosed),
            );
            pick(
                &mut flags,
                on(OFFLINE_FEEDING_BY_BUTTON),
                F::PaperFedByButton,
                None,
            );
            pick(
                &mut flags,
                on(OFFLINE_PAPER_END_STOP),
                F::PaperEndStop,
                None,
            );
            pick(&mut flags, on(OFFLINE_ERROR), F::ErrorOccurred, None);
        }
        Layout::ErrorCause => error_flags(
            &mut flags,
            byte,
            [
                ERROR_RECOVERABLE,
                ERROR_AUTOCUTTER,
                ERROR_UNRECOVERABLE,
                ERROR_AUTO_RECOVERABLE,
            ],
        ),
        Layout::RollPaperSensor => {
            pick(
                &mut flags,
                on(ROLL_PAPER_END),
                F::PaperEnd,
                Some(F::PaperPresent),
            );
            pick(
                &mut flags,
                on(ROLL_NEAR_END),
                F::PaperNearEnd,
                Some(F::PaperAdequate),
            );
        }
        Layout::PaperSensor => {
            pick(
                &mut flags,
                on(SENSOR_PAPER_END),
                F::PaperEnd,
                Some(F::PaperPresent),
            );
            pick(
                &mut flags,
                on(SENSOR_NEAR_END),
                F::PaperNearEnd,
                Some(F::PaperAdequate),
            );
        }
        Layout::DrawerConnector => {
            pick(
                &mut flags,
                on(CONNECTOR_PIN3_HIGH),
                F::DrawerHigh,
                Some(F::DrawerLow),
            );
        }
    }
    flags
}

/// Flags of a 4-byte basic ASB status.
fn asb_flags(bytes: &[u8; 4]) -> Vec<StatusFlag> {
    use StatusFlag as F;
    let [first, second, third, _] = *bytes;
    let mut flags = Vec::new();
    let on = |byte: u8, bits: u8| byte & bits == bits;
    pick(
        &mut flags,
        on(first, ASB1_OFFLINE),
        F::Offline,
        Some(F::Online),
    );
    pick(
        &mut flags,
        on(first, ASB1_DRAWER_HIGH),
        F::DrawerHigh,
        Some(F::DrawerLow),
    );
    pick(
        &mut flags,
        on(first, ASB1_COVER_OPEN),
        F::CoverOpen,
        Some(F::CoverClosed),
    );
    pick(
        &mut flags,
        on(first, ASB1_FEEDING_BY_BUTTON),
        F::PaperFedByButton,
        None,
    );
    pick(
        &mut flags,
        on(second, ASB2_WAITING_RECOVERY),
        F::WaitingForOnlineRecovery,
        None,
    );
    pick(
        &mut flags,
        on(second, ASB2_FEED_BUTTON),
        F::FeedButtonPressed,
        None,
    );
    error_flags(
        &mut flags,
        second,
        [
            ASB2_RECOVERABLE,
            ASB2_AUTOCUTTER,
            ASB2_UNRECOVERABLE,
            ASB2_AUTO_RECOVERABLE,
        ],
    );
    pick(
        &mut flags,
        on(third, ASB3_PAPER_END),
        F::PaperEnd,
        Some(F::PaperPresent),
    );
    pick(
        &mut flags,
        on(third, ASB3_NEAR_END),
        F::PaperNearEnd,
        Some(F::PaperAdequate),
    );
    flags
}

#[cfg(test)]
mod tests {
    use super::*;

    const NORMAL: PrinterStatus = PrinterStatus {
        paper_out: false,
        cover_open: false,
        offline: false,
        drawer_open: false,
    };

    fn eot(n: u8) -> Command {
        Command::RealTimeStatus {
            n,
            a: None,
            status: None,
        }
    }

    #[test]
    fn fixed_bits_are_always_set_as_in_the_reference() {
        for bits in 0..16_u8 {
            let s = PrinterStatus {
                paper_out: bits & 1 != 0,
                cover_open: bits & 2 != 0,
                offline: bits & 4 != 0,
                drawer_open: bits & 8 != 0,
            };
            // DLE EOT: 0xx1xx10b.
            for byte in [
                printer_status_byte(&s),
                offline_cause_byte(&s),
                error_cause_byte(&s),
                roll_paper_sensor_byte(&s),
            ] {
                assert_eq!(byte & 0b1001_0011, 0b0001_0010, "{byte:#04x}");
            }
            // GS r: bits 4 and 7 fixed 0, reserved bits 0.
            assert_eq!(paper_sensor_byte(&s) & 0xF0, 0);
            assert_eq!(drawer_connector_byte(&s) & 0xFE, 0);
            // ASB: first byte 0xx1xx00b; second byte bit 4/7 = 0; third and
            // fourth bytes bits 4 and 7 = 0.
            let asb = automatic_status(&s);
            assert_eq!(asb[0] & 0b1001_0011, 0b0001_0000);
            for byte in &asb[1..] {
                assert_eq!(byte & 0b1001_0000, 0);
            }
        }
    }

    #[test]
    fn python_escpos_masks_read_the_replies_correctly() {
        // escpos.constants: RT_MASK_ONLINE 8, RT_MASK_PAPER 18,
        // RT_MASK_LOWPAPER 30, RT_MASK_NOPAPER 114.
        assert_eq!(printer_status_byte(&NORMAL) & 8, 0);
        let out = PrinterStatus {
            paper_out: true,
            ..NORMAL
        };
        assert_ne!(printer_status_byte(&out) & 8, 0);
        assert_eq!(roll_paper_sensor_byte(&NORMAL) & 114, 18);
        assert_eq!(roll_paper_sensor_byte(&out) & 114, 114);
    }

    #[test]
    fn replies_carry_decoded_flags() {
        let reply = status_reply(&eot(1), &NORMAL).expect("reply");
        assert_eq!(reply.bytes, vec![0x12]);
        assert_eq!(reply.flags, vec![StatusFlag::Online, StatusFlag::DrawerLow]);
        let both = PrinterStatus {
            cover_open: true,
            paper_out: true,
            ..NORMAL
        };
        let reply = status_reply(&eot(2), &both).expect("reply");
        assert_eq!(reply.bytes, vec![0x36]);
        assert_eq!(
            reply.flags,
            vec![StatusFlag::CoverOpen, StatusFlag::PaperEndStop]
        );
        assert_eq!(
            status_reply(&eot(3), &both).expect("reply").flags,
            vec![StatusFlag::NoError]
        );
        assert!(status_reply(&eot(5), &NORMAL).is_none());
        assert!(status_reply(&Command::TransmitStatus { n: 4 }, &NORMAL).is_none());
        let asb = asb_reply(&both);
        assert_eq!(asb.bytes, vec![0x38, 0x00, 0x0C, 0x00]);
        assert_eq!(asb.kind, ReplyKind::Automatic);
    }

    #[test]
    fn asb_is_sent_only_for_enabled_items() {
        let drawer = PrinterStatus {
            drawer_open: true,
            ..NORMAL
        };
        let cover = PrinterStatus {
            cover_open: true,
            ..NORMAL
        };
        let paper = PrinterStatus {
            paper_out: true,
            ..NORMAL
        };
        let offline = PrinterStatus {
            offline: true,
            ..NORMAL
        };
        assert!(asb_changed(0x01, &NORMAL, &drawer));
        assert!(!asb_changed(0x02, &NORMAL, &drawer));
        assert!(asb_changed(0x02, &NORMAL, &cover));
        assert!(!asb_changed(0x01 | 0x04 | 0x08 | 0x40, &NORMAL, &cover));
        assert!(asb_changed(0x08, &NORMAL, &paper));
        assert!(asb_changed(0x02, &NORMAL, &paper));
        assert!(!asb_changed(0x04, &NORMAL, &paper));
        assert!(asb_changed(0x02, &NORMAL, &offline));
        assert!(!asb_changed(0x08, &NORMAL, &offline));
        // Offline switch while the cover is open: nothing visible changes.
        let both = PrinterStatus {
            offline: true,
            ..cover
        };
        assert!(!asb_changed(0xFF, &cover, &both));
        assert!(!asb_changed(0, &NORMAL, &drawer));
    }

    #[test]
    fn patch_merges_present_fields_only() {
        let patch: PrinterStatusPatch =
            serde_json::from_str(r#"{"coverOpen":true}"#).expect("patch");
        let merged = PrinterStatus {
            drawer_open: true,
            ..NORMAL
        }
        .merged(patch);
        assert_eq!(
            merged,
            PrinterStatus {
                cover_open: true,
                drawer_open: true,
                ..NORMAL
            }
        );
        assert!(serde_json::from_str::<PrinterStatusPatch>(r#"{"jammed":true}"#).is_err());
        assert_eq!(
            serde_json::to_string(&merged).expect("json"),
            r#"{"paperOut":false,"coverOpen":true,"offline":false,"drawerOpen":true}"#
        );
    }
}
