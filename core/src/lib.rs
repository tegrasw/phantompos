//! ESC/POS parser and receipt model for PhantomPOS.
//!
//! This crate is deliberately isolated: it must never depend on networking,
//! disk access, Tokio or Tauri. It turns bytes into typed commands and builds
//! the receipt model; everything else lives in the application crate.
//!
//! The parser itself arrives in stage 2 of the project plan (see `SPEC.md`).
