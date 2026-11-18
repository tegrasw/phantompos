//! QR Code symbols for `GS ( k` (cn = 49), generated with the `qrcode` crate.
//!
//! Like the printer, the data is encoded with the smallest version that holds
//! it at the selected error correction level, choosing the numeric,
//! alphanumeric, Kanji (Shift JIS) or 8-bit byte modes automatically. The
//! quiet zone is not part of the symbol (reference: "The quiet zone is not
//! included in the printing data").
//!
//! Deliberate deviation: model 1 (obsolete; not supported by the generator
//! nor by most readers) is printed as model 2.

use qrcode::{EcLevel, QrCode, Version};

use crate::bitmap::Bitmap;
use crate::command::{QrCodeModel, QrErrorCorrection};

/// Why a stored QR Code cannot be printed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum QrProblem {
    /// The data does not fit in any version of the model at this level.
    #[error("the data does not fit in a QR Code of this model and level")]
    DataTooLong,
    /// The symbol is wider than the paper ("cannot be printed").
    #[error("the symbol is wider than the print area")]
    TooWide,
}

fn ec_level(level: QrErrorCorrection) -> EcLevel {
    match level {
        QrErrorCorrection::L => EcLevel::L,
        QrErrorCorrection::M => EcLevel::M,
        QrErrorCorrection::Q => EcLevel::Q,
        QrErrorCorrection::H => EcLevel::H,
    }
}

fn encode(data: &[u8], model: QrCodeModel, level: QrErrorCorrection) -> Option<QrCode> {
    let ec = ec_level(level);
    match model {
        QrCodeModel::Model1 | QrCodeModel::Model2 => {
            QrCode::with_error_correction_level(data, ec).ok()
        }
        QrCodeModel::Micro => {
            (1..=4).find_map(|version| QrCode::with_version(data, Version::Micro(version), ec).ok())
        }
    }
}

/// Draws the symbol with square modules of `module` dots, or says why it
/// cannot be printed on `max_width` dots.
pub(crate) fn render(
    data: &[u8],
    model: QrCodeModel,
    level: QrErrorCorrection,
    module: u32,
    max_width: u32,
) -> Result<Bitmap, QrProblem> {
    let code = encode(data, model, level).ok_or(QrProblem::DataTooLong)?;
    let modules = u32::try_from(code.width()).map_err(|_| QrProblem::TooWide)?;
    let size = modules.saturating_mul(module);
    if size > max_width {
        return Err(QrProblem::TooWide);
    }
    let mut bitmap = Bitmap::new(size, size);
    for (index, color) in code.to_colors().into_iter().enumerate() {
        if color != qrcode::Color::Dark {
            continue;
        }
        let index = u32::try_from(index).unwrap_or(u32::MAX);
        let (x, y) = (
            index.checked_rem(modules).unwrap_or(0),
            index.checked_div(modules).unwrap_or(0),
        );
        bitmap.fill(
            x.saturating_mul(module),
            y.saturating_mul(module),
            module,
            module,
        );
    }
    Ok(bitmap)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_follows_data_and_level() {
        let small = render(
            b"PhantomPOS",
            QrCodeModel::Model2,
            QrErrorCorrection::L,
            1,
            576,
        )
        .unwrap();
        assert_eq!(small.width(), 21); // version 1
        let url = render(
            b"https://github.com/tegrasw/phantompos",
            QrCodeModel::Model2,
            QrErrorCorrection::H,
            3,
            576,
        )
        .unwrap();
        assert_eq!(url.width() % 3, 0);
        assert!(url.width() > 21 * 3);
        let micro = render(b"12345", QrCodeModel::Micro, QrErrorCorrection::L, 1, 576).unwrap();
        assert_eq!(micro.width(), 11); // M1
    }

    #[test]
    fn too_long_or_too_wide() {
        let long = vec![b'x'; 3000];
        assert_eq!(
            render(&long, QrCodeModel::Model2, QrErrorCorrection::L, 1, 576).err(),
            Some(QrProblem::DataTooLong)
        );
        assert_eq!(
            render(b"abc", QrCodeModel::Model2, QrErrorCorrection::L, 16, 300).err(),
            Some(QrProblem::TooWide)
        );
        assert_eq!(
            render(b"abc", QrCodeModel::Micro, QrErrorCorrection::H, 3, 576).err(),
            Some(QrProblem::DataTooLong)
        );
    }
}
