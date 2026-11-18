//! 1-bit bitmaps of printed images and their base64 form.

/// A black and white bitmap, one bit per dot, rows packed MSB first (the
/// layout of `GS v 0` data and of the `image` element's `data`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Bitmap {
    width: u32,
    height: u32,
    /// Bytes per row: `ceil(width / 8)`.
    stride: usize,
    bits: Vec<u8>,
}

impl Bitmap {
    /// A white bitmap. Callers bound the size (the width never exceeds the
    /// paper; heights come from capped payloads).
    pub fn new(width: u32, height: u32) -> Self {
        let stride = (width as usize).div_ceil(8);
        Self {
            width,
            height,
            stride,
            bits: vec![0; stride.saturating_mul(height as usize)],
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Makes the dot at (`x`, `y`) black; outside the bitmap it is ignored.
    pub fn set(&mut self, x: u32, y: u32) {
        if x >= self.width || y >= self.height {
            return;
        }
        let index = (y as usize)
            .saturating_mul(self.stride)
            .saturating_add(x as usize / 8);
        if let Some(byte) = self.bits.get_mut(index) {
            *byte |= 0x80 >> (x % 8);
        }
    }

    /// Fills a rectangle with black, clipped to the bitmap.
    pub fn fill(&mut self, x: u32, y: u32, width: u32, height: u32) {
        let x_end = x.saturating_add(width).min(self.width);
        let y_end = y.saturating_add(height).min(self.height);
        for row in y..y_end {
            for col in x..x_end {
                self.set(col, row);
            }
        }
    }

    /// Copies row `from` over row `to` (both must exist).
    pub fn copy_row(&mut self, from: u32, to: u32) {
        let stride = self.stride;
        let start = (from as usize).saturating_mul(stride);
        let dest = (to as usize).saturating_mul(stride);
        if start.saturating_add(stride) <= self.bits.len()
            && dest.saturating_add(stride) <= self.bits.len()
        {
            self.bits
                .copy_within(start..start.saturating_add(stride), dest);
        }
    }

    /// The packed rows as standard base64 with padding.
    pub fn to_base64(&self) -> String {
        base64(&self.bits)
    }
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 (RFC 4648, with `=` padding). Hand-written: it is a few
/// lines and avoids a dependency for the only place that needs it.
pub(crate) fn base64(input: &[u8]) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(3).saturating_mul(4));
    let symbol = |index: u32| {
        char::from(
            ALPHABET
                .get((index & 0x3F) as usize)
                .copied()
                .unwrap_or(b'A'),
        )
    };
    for chunk in input.chunks(3) {
        let b = |i: usize| u32::from(chunk.get(i).copied().unwrap_or(0));
        let triple = (b(0) << 16) | (b(1) << 8) | b(2);
        out.push(symbol(triple >> 18));
        out.push(symbol(triple >> 12));
        out.push(if chunk.len() > 1 {
            symbol(triple >> 6)
        } else {
            '='
        });
        out.push(if chunk.len() > 2 { symbol(triple) } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_rfc4648_vectors() {
        let cases: [(&[u8], &str); 7] = [
            (b"", ""),
            (b"f", "Zg=="),
            (b"fo", "Zm8="),
            (b"foo", "Zm9v"),
            (b"foob", "Zm9vYg=="),
            (b"fooba", "Zm9vYmE="),
            (b"foobar", "Zm9vYmFy"),
        ];
        for (input, expected) in cases {
            assert_eq!(base64(input), expected);
        }
        assert_eq!(base64(&[0xFF, 0xFE, 0xFD]), "//79");
    }

    #[test]
    fn set_packs_msb_first() {
        let mut b = Bitmap::new(10, 2);
        b.set(0, 0);
        b.set(9, 1);
        b.set(10, 0); // outside
        assert_eq!(b.bits, vec![0x80, 0x00, 0x00, 0x40]);
        b.copy_row(1, 0);
        assert_eq!(b.bits, vec![0x00, 0x40, 0x00, 0x40]);
    }
}
