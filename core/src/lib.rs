//! ESC/POS parser and receipt model for PhantomPOS.
//!
//! This crate is deliberately isolated: it must never depend on networking,
//! disk access, Tokio or Tauri. It turns bytes into typed commands and builds
//! the receipt model; everything else lives in the application crate.
//!
//! Typical use, one parser and one builder per connection:
//!
//! ```
//! use phantompos_core::{Parser, ParserConfig, ReceiptBuilder, ReceiptConfig};
//!
//! let mut parser = Parser::new(ParserConfig::default());
//! let mut builder = ReceiptBuilder::new(ReceiptConfig::default());
//!
//! // For every socket read:
//! let batch = parser.feed(b"\x1bE\x01Total\x1bE\x00 9,90\n");
//! builder.apply_all(&batch); // `batch` also goes to the command log
//!
//! // When the connection ends:
//! let tail = parser.finish();
//! builder.apply_all(&tail);
//! let receipt = builder.finish();
//! assert_eq!(receipt.elements.len(), 1);
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

mod codepage;
mod codepage_tables;
mod command;
mod error;
mod paper;
mod parser;
mod receipt;

pub use codepage::CodePage;
pub use command::{Alignment, Command, DecodedCommand, Font, UnderlineMode, UnknownReason};
pub use error::CoreError;
pub use paper::{FONT_A, FONT_B, FontMetrics, PaperProfile, font_metrics};
pub use parser::{MAX_TEXT_RUN, Parser, ParserConfig, parse};
pub use receipt::{
    DEFAULT_LINE_SPACING, Element, Line, Receipt, ReceiptBuilder, ReceiptConfig, Span, TextStyle,
    render,
};
