//! Single-byte character code tables selected by `ESC t n`.
//!
//! Only the pages used for Portuguese are supported. The lower half
//! (0x20-0x7F) is the Epson "Common Page" (ASCII, international character set
//! U.S.A.); per the Epson table, 0x7F prints as a space. The upper halves are
//! generated from Python's codecs by `core/tools/gen_codepages.py`.

use serde::{Deserialize, Serialize};

use crate::codepage_tables;

/// A supported character code table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodePage {
    /// Page 0, PC437 (USA, Standard Europe). Printer default.
    #[default]
    Cp437,
    /// Page 2, PC850 (Multilingual).
    Cp850,
    /// Page 3, PC860 (Portuguese).
    Cp860,
    /// Page 16, WPC1252.
    Windows1252,
    /// Page 19, PC858 (PC850 with the euro sign).
    Cp858,
}

impl CodePage {
    /// Maps the `n` of `ESC t n` to a supported page (Epson "Select character
    /// code table": 0 = PC437, 2 = PC850, 3 = PC860, 16 = WPC1252, 19 = PC858).
    /// Returns `None` for pages this emulator does not implement.
    pub fn from_esc_t(n: u8) -> Option<Self> {
        match n {
            0 => Some(Self::Cp437),
            2 => Some(Self::Cp850),
            3 => Some(Self::Cp860),
            16 => Some(Self::Windows1252),
            19 => Some(Self::Cp858),
            _ => None,
        }
    }

    /// The `n` that selects this page with `ESC t n`.
    pub fn esc_t_value(self) -> u8 {
        match self {
            Self::Cp437 => 0,
            Self::Cp850 => 2,
            Self::Cp860 => 3,
            Self::Windows1252 => 16,
            Self::Cp858 => 19,
        }
    }

    fn upper_half(self) -> &'static [char; 128] {
        match self {
            Self::Cp437 => &codepage_tables::CP437,
            Self::Cp850 => &codepage_tables::CP850,
            Self::Cp860 => &codepage_tables::CP860,
            Self::Windows1252 => &codepage_tables::WINDOWS_1252,
            Self::Cp858 => &codepage_tables::CP858,
        }
    }

    /// Decodes one printable byte (0x20-0xFF). Control bytes never reach this
    /// function in practice; they decode as U+FFFD to stay total.
    pub fn decode_byte(self, byte: u8) -> char {
        match byte {
            0x7F => ' ',
            0x20..=0x7E => char::from(byte),
            0x80..=0xFF => self
                .upper_half()
                .get(usize::from(byte & 0x7F))
                .copied()
                .unwrap_or(char::REPLACEMENT_CHARACTER),
            _ => char::REPLACEMENT_CHARACTER,
        }
    }

    /// Decodes a run of printable bytes.
    pub fn decode(self, bytes: &[u8]) -> String {
        bytes.iter().map(|&b| self.decode_byte(b)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portuguese_characters() {
        assert_eq!(CodePage::Cp850.decode(&[0x87, 0xC6, 0xA6, 0xA7]), "çãªº");
        assert_eq!(CodePage::Cp860.decode(&[0x87, 0x84, 0x82]), "çãé");
        assert_eq!(CodePage::Cp858.decode(&[0xD5]), "€");
        assert_eq!(CodePage::Windows1252.decode(&[0x80, 0xE7, 0xE3]), "€çã");
        assert_eq!(CodePage::Cp437.decode(&[0x80, 0x7F, b'A']), "Ç A");
    }

    #[test]
    fn windows_1252_undefined_slots_are_spaces() {
        for b in [0x81, 0x8D, 0x8F, 0x90, 0x9D] {
            assert_eq!(CodePage::Windows1252.decode_byte(b), ' ');
        }
    }

    #[test]
    fn esc_t_round_trip() {
        for n in 0..=255u8 {
            if let Some(page) = CodePage::from_esc_t(n) {
                assert_eq!(page.esc_t_value(), n);
            }
        }
    }
}
