//! Paper profiles and font metrics.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::command::Font;
use crate::error::CoreError;

/// Supported paper widths. Serialized as `"58mm"` / `"80mm"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum PaperProfile {
    /// 58 mm paper: 384 dots, 32 columns of font A.
    #[serde(rename = "58mm")]
    Mm58,
    /// 80 mm paper: 576 dots, 48 columns of font A. Application default.
    #[default]
    #[serde(rename = "80mm")]
    Mm80,
}

impl PaperProfile {
    /// Printable width in dots.
    pub fn width_dots(self) -> u32 {
        match self {
            Self::Mm58 => 384,
            Self::Mm80 => 576,
        }
    }

    /// Characters per line in font A at normal width.
    pub fn columns_font_a(self) -> u32 {
        self.width_dots() / FONT_A.width
    }

    /// The name used in settings and JSON (`"58mm"`, `"80mm"`).
    pub fn name(self) -> &'static str {
        match self {
            Self::Mm58 => "58mm",
            Self::Mm80 => "80mm",
        }
    }
}

impl fmt::Display for PaperProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for PaperProfile {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "58mm" => Ok(Self::Mm58),
            "80mm" => Ok(Self::Mm80),
            other => Err(CoreError::UnknownPaperProfile(other.to_owned())),
        }
    }
}

/// Character cell of a font at 1x size, in dots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontMetrics {
    pub width: u32,
    pub height: u32,
    /// Distance from the top of the cell to the baseline.
    pub baseline: u32,
}

/// Font A, 12 x 24 dots, baseline 21 (Epson `ESC M`, TM-T88/TM-T20 class).
pub const FONT_A: FontMetrics = FontMetrics {
    width: 12,
    height: 24,
    baseline: 21,
};

/// Font B, 9 x 17 dots, baseline 16 (Epson `ESC M`, TM-T88/TM-T20 class).
pub const FONT_B: FontMetrics = FontMetrics {
    width: 9,
    height: 17,
    baseline: 16,
};

/// Metrics of a drawable font. Only fonts A and B exist in this emulator;
/// the others never become the active font, and map to font A here.
pub fn font_metrics(font: Font) -> FontMetrics {
    match font {
        Font::B => FONT_B,
        _ => FONT_A,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns() {
        assert_eq!(PaperProfile::Mm58.columns_font_a(), 32);
        assert_eq!(PaperProfile::Mm80.columns_font_a(), 48);
    }

    #[test]
    fn parse_names() {
        assert_eq!("58mm".parse::<PaperProfile>(), Ok(PaperProfile::Mm58));
        assert!("57mm".parse::<PaperProfile>().is_err());
    }
}
