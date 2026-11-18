// DEVELOPMENT ONLY. Bitmaps for the demo backend's image commands. In the
// real app core generates every image and code; the demo has no core, so it
// builds them here: the app mark rasterised for a logo, a dithered poster
// (a raster command over 40 KB, to exercise the command log), a real EAN-13
// encoder and a precomputed QR code. Both codes are genuine and scannable.

/** A 1-bit image packed as the receipt contract wants it. */
export interface DemoBitmap {
  width: number;
  height: number;
  /** ceil(width / 8) bytes per row, MSB first, 1 = black. */
  bits: Uint8Array;
}

export function pack(width: number, height: number, black: (x: number, y: number) => boolean): DemoBitmap {
  const stride = Math.ceil(width / 8);
  const bits = new Uint8Array(stride * height);
  for (let y = 0; y < height; y++)
    for (let x = 0; x < width; x++) if (black(x, y)) bits[y * stride + (x >> 3)] |= 0x80 >> (x & 7);
  return { width, height, bits };
}

export function toBase64(bytes: Uint8Array): string {
  let binary = "";
  for (let i = 0; i < bytes.length; i += 0x8000)
    binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(binary);
}

// QR code ---------------------------------------------------------------------

/**
 * `https://github.com/tegrasw/phantompos`, version 3, error correction M,
 * mask chosen by the encoder, no quiet zone. Generated offline with the
 * Python `qrcode` package (not a dependency of this repo).
 */
const QR_ROWS = [
  "11111110111111000100001111111",
  "10000010100110101111001000001",
  "10111010111001100010101011101",
  "10111010001010111100101011101",
  "10111010111111101111001011101",
  "10000010011111101110101000001",
  "11111110101010101010101111111",
  "00000000001010100001000000000",
  "10011111111101110101010010111",
  "00101001000010110001010110110",
  "10010111011100111010100110100",
  "11101100100010100100111101001",
  "10111010011101101001001100001",
  "00110101001110011000001111111",
  "00100010111100110101111010101",
  "10110100101111110001000110101",
  "10010110110110000011110001000",
  "10000001001000010011000010110",
  "11101111001111111001000011001",
  "11010001001001001001001001100",
  "11111110011011010100111111110",
  "00000000111011010000100011000",
  "11111110100001010111101011000",
  "10000010111101010111100010011",
  "10111010110101001001111111001",
  "10111010100010111110010000001",
  "10111010001110010010110110111",
  "10000010010100101010001101101",
  "11111110111000010001101010000",
];
export const QR_TEXT = "https://github.com/tegrasw/phantompos";

export function qrBitmap(moduleDots: number): DemoBitmap {
  const size = QR_ROWS.length * moduleDots;
  return pack(size, size, (x, y) => QR_ROWS[Math.floor(y / moduleDots)][Math.floor(x / moduleDots)] === "1");
}

// EAN-13 ----------------------------------------------------------------------

const L_CODES = ["0001101", "0011001", "0010011", "0111101", "0100011", "0110001", "0101111", "0111011", "0110111", "0001011"];
const flip = (code: string): string => code.replace(/[01]/g, (b) => (b === "0" ? "1" : "0"));
const R_CODES = L_CODES.map(flip);
const G_CODES = R_CODES.map((code) => [...code].reverse().join(""));
const PARITY = ["LLLLLL", "LLGLGG", "LLGGLG", "LLGGGL", "LGLLGG", "LGGLLG", "LGGGLL", "LGLGLG", "LGLGGL", "LGGLGL"];

/** Appends the check digit to 12 digits. */
export function ean13(digits12: string): string {
  let sum = 0;
  for (let i = 0; i < 12; i++) sum += Number(digits12[i]) * (i % 2 === 0 ? 1 : 3);
  return digits12 + String((10 - (sum % 10)) % 10);
}

/** The 95 modules of an EAN-13 symbol as "0"/"1". */
export function ean13Modules(code: string): string {
  const d = [...code].map(Number);
  const parity = PARITY[d[0]];
  let modules = "101";
  for (let i = 1; i <= 6; i++) modules += (parity[i - 1] === "L" ? L_CODES : G_CODES)[d[i]];
  modules += "01010";
  for (let i = 7; i <= 12; i++) modules += R_CODES[d[i]];
  return modules + "101";
}

export function ean13Bitmap(code: string, moduleDots: number, height: number): DemoBitmap {
  const modules = ean13Modules(code);
  return pack(modules.length * moduleDots, height, (x) => modules[Math.floor(x / moduleDots)] === "1");
}

// Logo and poster ---------------------------------------------------------------

/** The app mark (AppMark.svelte), 24 x 24 units. */
const MARK_PATHS = [
  "M4 3.5h16a2 2 0 0 1 0 4H4a2 2 0 0 1 0-4zM5.5 5a.5.5 0 0 0 0 1h13a.5.5 0 0 0 0-1z",
  "M6.5 9.6q0-.6.6-.6h9.8q.6 0 .6.6v10.95q0 .45-.25.08l-.87-1.26q-.255-.37-.51 0L15 20.63q-.25.37-.5 0l-.87-1.26q-.255-.37-.51 0l-.87 1.26q-.25.37-.5 0l-.87-1.26q-.255-.37-.51 0L9.5 20.63q-.25.37-.5 0l-.87-1.26q-.255-.37-.51 0l-.87 1.26q-.25.37-.25-.08zM9.3 12.85v1.9a.85.85 0 0 0 1.7 0v-1.9a.85.85 0 0 0-1.7 0zM13 12.85v1.9a.85.85 0 0 0 1.7 0v-1.9a.85.85 0 0 0-1.7 0z",
];

function context(width: number, height: number): CanvasRenderingContext2D {
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) throw new Error("demo: no 2D context");
  return ctx;
}

function drawMark(ctx: CanvasRenderingContext2D, x: number, y: number, size: number): void {
  ctx.save();
  ctx.translate(x, y);
  ctx.scale(size / 24, size / 24);
  for (const d of MARK_PATHS) ctx.fill(new Path2D(d), "evenodd");
  ctx.restore();
}

/** The mark, thresholded: a typical shop logo sent with `GS v 0`. */
export function logoBitmap(size: number): DemoBitmap {
  const ctx = context(size, size);
  ctx.fillStyle = "#000";
  drawMark(ctx, 0, 0, size);
  const alpha = ctx.getImageData(0, 0, size, size).data;
  return pack(size, size, (x, y) => alpha[(y * size + x) * 4 + 3] >= 128);
}

const BAYER_8 = [
  0, 32, 8, 40, 2, 34, 10, 42, 48, 16, 56, 24, 50, 18, 58, 26, 12, 44, 4, 36, 14, 46, 6, 38, 60, 28, 52, 20, 62, 30, 54, 22,
  3, 35, 11, 43, 1, 33, 9, 41, 51, 19, 59, 27, 49, 17, 57, 25, 15, 47, 7, 39, 13, 45, 5, 37, 63, 31, 55, 23, 61, 29, 53, 21,
];

/** A full-width "photo": the mark over a soft glow, ordered-dithered. */
export function posterBitmap(width: number, height: number): DemoBitmap {
  const ctx = context(width, height);
  const glow = ctx.createRadialGradient(width / 2, height * 0.45, width * 0.05, width / 2, height / 2, width * 0.72);
  glow.addColorStop(0, "#fff");
  glow.addColorStop(0.55, "#b8b8b8");
  glow.addColorStop(1, "#2a2a2a");
  ctx.fillStyle = glow;
  ctx.fillRect(0, 0, width, height);
  const size = Math.min(width, height) * 0.82;
  ctx.fillStyle = "#151515";
  drawMark(ctx, (width - size) / 2, (height - size) / 2, size);
  const rgba = ctx.getImageData(0, 0, width, height).data;
  return pack(width, height, (x, y) => {
    const i = (y * width + x) * 4;
    const luminance = (rgba[i] * 77 + rgba[i + 1] * 150 + rgba[i + 2] * 29) >> 8;
    return luminance < ((BAYER_8[(y & 7) * 8 + (x & 7)] + 0.5) * 255) / 64;
  });
}
