//! `export_receipt`: saving a completed receipt as PNG or as its raw bytes.
//!
//! The UI renders the PNG (its canvas renderer is the source of truth for
//! the drawing) and sends the file bytes; the backend keeps the raw bytes
//! (see `listener.rs`). Everything here is independent of the save dialog,
//! which lives in `lib.rs`: [`prepare`] checks the request and returns what
//! to write, [`write`] writes it to the chosen path.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::listener::{ListenerState, RetainedError};

/// Largest PNG accepted from the UI.
pub const MAX_PNG_BYTES: usize = 32 * 1024 * 1024;

/// The 8-byte PNG file signature.
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// `format` argument of `export_receipt`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat {
    Png,
    Bin,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Bin => "bin",
        }
    }

    /// Name of the file type in the save dialog's filter.
    pub fn filter_name(self) -> &'static str {
        match self {
            Self::Png => "PNG image",
            Self::Bin => "ESC/POS bytes",
        }
    }
}

/// Result of `export_receipt`: `{ saved: true, path }` or `{ saved: false }`
/// when the user cancelled the dialog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExportResult {
    pub saved: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl ExportResult {
    pub fn cancelled() -> Self {
        Self {
            saved: false,
            path: None,
        }
    }

    pub fn saved(path: &Path) -> Self {
        Self {
            saved: true,
            path: Some(path.display().to_string()),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error(transparent)]
    Receipt(#[from] RetainedError),
    #[error("PNG export needs the image bytes (`png`).")]
    MissingPng,
    #[error("The image is not a PNG file.")]
    InvalidPng,
    #[error("The image is larger than 32 MiB.")]
    PngTooLarge,
    #[error("Could not write {path}: {source}")]
    Write {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("The save dialog returned a location that is not a file path.")]
    InvalidPath,
}

/// Bytes to write for an export request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Contents {
    Owned(Vec<u8>),
    Shared(Arc<[u8]>),
}

impl AsRef<[u8]> for Contents {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Owned(bytes) => bytes,
            Self::Shared(bytes) => bytes,
        }
    }
}

/// `phantompos-receipt-0007.png`.
pub fn suggested_file_name(id: u64, format: ExportFormat) -> String {
    format!("phantompos-receipt-{id:04}.{}", format.extension())
}

/// Checks the request and returns the file contents. `png` is ignored for
/// `.bin`. Done before the dialog opens, so a bad request never shows one.
pub fn prepare(
    state: &ListenerState,
    id: u64,
    format: ExportFormat,
    png: Option<Vec<u8>>,
) -> Result<Contents, ExportError> {
    match format {
        ExportFormat::Bin => Ok(Contents::Shared(state.receipt_bytes(id)?)),
        ExportFormat::Png => {
            state.check_receipt_id(id)?;
            let png = png.ok_or(ExportError::MissingPng)?;
            if png.len() > MAX_PNG_BYTES {
                return Err(ExportError::PngTooLarge);
            }
            if !png.starts_with(PNG_SIGNATURE) {
                return Err(ExportError::InvalidPng);
            }
            Ok(Contents::Owned(png))
        }
    }
}

/// The chosen path, with the format's extension added when it has none
/// (some dialogs do not add it from the filter).
pub fn with_extension(path: PathBuf, format: ExportFormat) -> PathBuf {
    if path.extension().is_some() {
        path
    } else {
        path.with_extension(format.extension())
    }
}

/// Writes `contents` to `path` (blocking).
pub fn write(path: &Path, contents: &Contents) -> Result<(), ExportError> {
    std::fs::write(path, contents.as_ref()).map_err(|source| ExportError::Write {
        path: path.display().to_string(),
        source,
    })
}

/// [`prepare`] and [`write`] to a path chosen without a dialog (tests).
#[cfg(test)]
pub fn export_to(
    state: &ListenerState,
    id: u64,
    format: ExportFormat,
    png: Option<Vec<u8>>,
    path: PathBuf,
) -> Result<ExportResult, ExportError> {
    let contents = prepare(state, id, format, png)?;
    let path = with_extension(path, format);
    write(&path, &contents)?;
    Ok(ExportResult::saved(&path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_shapes() {
        assert_eq!(
            suggested_file_name(7, ExportFormat::Png),
            "phantompos-receipt-0007.png"
        );
        assert_eq!(
            suggested_file_name(12345, ExportFormat::Bin),
            "phantompos-receipt-12345.bin"
        );
        assert_eq!(
            serde_json::to_string(&ExportResult::cancelled()).unwrap(),
            r#"{"saved":false}"#
        );
        assert_eq!(
            serde_json::to_string(&ExportResult::saved(Path::new("/tmp/a.png"))).unwrap(),
            r#"{"saved":true,"path":"/tmp/a.png"}"#
        );
        assert_eq!(
            serde_json::from_str::<ExportFormat>(r#""bin""#).unwrap(),
            ExportFormat::Bin
        );
        assert!(serde_json::from_str::<ExportFormat>(r#""jpg""#).is_err());
        assert_eq!(
            with_extension(PathBuf::from("/x/receipt"), ExportFormat::Bin),
            PathBuf::from("/x/receipt.bin")
        );
        assert_eq!(
            with_extension(PathBuf::from("/x/r.dat"), ExportFormat::Bin),
            PathBuf::from("/x/r.dat")
        );
    }
}
