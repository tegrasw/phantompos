//! Error types of the crate.
//!
//! Parsing itself is infallible by design (malformed input becomes `unknown`
//! commands), so errors only exist at the configuration boundary.

use thiserror::Error;

/// Errors returned by `phantompos-core`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CoreError {
    /// A paper profile name other than `"58mm"` or `"80mm"`.
    #[error("unknown paper profile {0:?} (expected \"58mm\" or \"80mm\")")]
    UnknownPaperProfile(String),
}
