//! ESC/POS parser and receipt model for PhantomPOS.
//!
//! This crate is deliberately isolated: it must never depend on networking,
//! disk access, Tokio or Tauri. It turns bytes into typed commands and builds
//! the receipt model; everything else lives in the application crate.
//!
//! Typical use, one parser and one builder per connection:
//!
//! ```
//! use phantompos_core::{Effect, Parser, ParserConfig, ReceiptBuilder, ReceiptConfig};
//!
//! let mut parser = Parser::new(ParserConfig::default());
//! let mut builder = ReceiptBuilder::new(ReceiptConfig::default());
//!
//! // For every socket read (`batch` also goes to the command log):
//! let batch = parser.feed(b"\x1bE\x01Total\x1bE\x00 9,90\n\x1dV\x00Next\n");
//! for effect in builder.apply_all(&batch) {
//!     match effect {
//!         Effect::ReceiptCompleted(receipt) => assert!(receipt.cut.is_some()),
//!         Effect::DrawerKick(kick) => println!("drawer pin {}", kick.pin),
//!     }
//! }
//! let in_progress = builder.take_update(); // Some(..) when it changed
//! assert!(in_progress.is_some());
//!
//! // When the connection ends:
//! let tail = parser.finish();
//! builder.apply_all(&tail);
//! let last = builder.finish(); // pending content, `cut: None`
//! assert_eq!(last.map(|r| r.elements.len()), Some(1));
//! ```
//!
//! See `core/README.md` for the JSON contract.

// Bytes come from the network: library code must not panic on any input.
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::arithmetic_side_effects
    )
)]

mod barcode;
mod bitmap;
mod codepage;
mod codepage_tables;
mod command;
mod error;
mod paper;
mod parser;
mod qr;
mod receipt;

pub use codepage::CodePage;
pub use command::{
    Alignment, BarcodeFunction, BarcodeProblem, Command, CutFunction, CutMode, DecodedCommand,
    Font, HriPosition, QrCodeModel, QrErrorCorrection, RasterMode, Symbology, UnderlineMode,
    UnknownReason,
};
pub use error::CoreError;
pub use paper::{FONT_A, FONT_B, FontMetrics, PaperProfile, font_metrics};
pub use parser::{MAX_PAYLOAD, MAX_TEXT_RUN, Parser, ParserConfig, SKIP_CHUNK, parse};
pub use receipt::{
    DEFAULT_BARCODE_HEIGHT, DEFAULT_BARCODE_MODULE, DEFAULT_LINE_SPACING, DEFAULT_QR_MODULE,
    DrawerKick, Effect, Element, Image, ImageKind, Line, Receipt, ReceiptBuilder, ReceiptConfig,
    RenderOutput, Span, TextStyle, render,
};
