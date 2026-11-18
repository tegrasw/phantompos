// Image elements (raster images, QR codes, barcode bars): core sends them as
// 1-bit bitmaps, base64 of packed rows (ceil(width / 8) bytes per row, MSB
// first, 1 = black dot). They are already exactly what the paper shows, so
// they are never drawn through the 2D context (no smoothing, no threshold):
// their dots are written straight into the band's ImageData after the text
// has been thresholded. Thermal print only adds ink, so 0 bits leave
// whatever is underneath.
//
// Decoding: once per distinct image. In-progress receipts are re-sent in full
// on every update (new objects, same strings), so bitmaps are cached by
// element object (WeakMap, free) and by the data string itself (one hash of
// the string, much cheaper than decoding it again), with a small LRU bound.
//
// Malformed data never throws: the decoder stops at the first character
// outside the base64 alphabet (whitespace is skipped), and rows missing from
// short data stay white. Extra data is ignored.

import type { ImageElement, ReceiptElement } from "../backend";
import type { Rgba } from "./render";

/** A decoded image: packed rows exactly as the contract describes them. */
export interface PackedBitmap {
  width: number;
  height: number;
  /** Bytes per row: ceil(width / 8). */
  stride: number;
  bits: Uint8Array;
}

/** Larger images are ignored (a 576-dot-wide image this tall is ~4.7 MB packed). */
const MAX_DOTS = 576 * 65536;
const CACHE_ENTRIES = 96;

const isNumber = (value: unknown): value is number => typeof value === "number" && Number.isFinite(value);

/** True for an image element with usable geometry (data may still be malformed). */
export function isImage(element: ReceiptElement): element is ImageElement {
  if (element.type !== "image") return false;
  const e = element as Partial<ImageElement>;
  return (
    isNumber(e.x) &&
    isNumber(e.y) &&
    isNumber(e.width) &&
    isNumber(e.height) &&
    e.width >= 1 &&
    e.height >= 1 &&
    e.width * e.height <= MAX_DOTS &&
    typeof e.data === "string"
  );
}

/** QR codes and barcodes: worth a closer look (and a phone scan). */
export function isCode(element: ImageElement): boolean {
  return element.kind === "qrCode" || element.kind === "barcode";
}

// Base64 ----------------------------------------------------------------------

const SKIP = -2;
const STOP = -1;
const ALPHABET = new Int8Array(128).fill(STOP);
{
  const chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  for (let i = 0; i < chars.length; i++) ALPHABET[chars.charCodeAt(i)] = i;
  // Tolerate the URL-safe alphabet too; it is unambiguous.
  ALPHABET[0x2d] = 62; // -
  ALPHABET[0x5f] = 63; // _
  for (const ws of [0x09, 0x0a, 0x0d, 0x20]) ALPHABET[ws] = SKIP;
}

/** Decodes into `out` (zero-filled); returns the number of bytes written. */
export function decodeBase64Into(text: string, out: Uint8Array): number {
  let acc = 0;
  let bits = 0;
  let written = 0;
  for (let i = 0; i < text.length && written < out.length; i++) {
    const code = text.charCodeAt(i);
    const value = code < 128 ? ALPHABET[code] : STOP;
    if (value === SKIP) continue;
    if (value === STOP) break; // "=" padding or garbage: keep what we have
    acc = ((acc << 6) | value) & 0xffffff;
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      out[written++] = (acc >> bits) & 0xff;
    }
  }
  return written;
}

// Cache -----------------------------------------------------------------------

const byElement = new WeakMap<object, PackedBitmap>();
const byData = new Map<string, PackedBitmap>();

function decode(element: ImageElement): PackedBitmap {
  const width = Math.floor(element.width);
  const height = Math.floor(element.height);
  const stride = Math.ceil(width / 8);
  const bits = new Uint8Array(stride * height);
  decodeBase64Into(element.data, bits);
  return { width, height, stride, bits };
}

/** The decoded bitmap of an image element (cached). */
export function imageBitmap(element: ImageElement): PackedBitmap {
  let bitmap = byElement.get(element);
  if (bitmap) return bitmap;
  const width = Math.floor(element.width);
  const height = Math.floor(element.height);
  bitmap = byData.get(element.data);
  if (bitmap && bitmap.width === width && bitmap.height === height) {
    // Refresh its LRU position.
    byData.delete(element.data);
  } else {
    bitmap = decode(element);
  }
  byData.set(element.data, bitmap);
  if (byData.size > CACHE_ENTRIES) {
    const oldest = byData.keys().next().value;
    if (oldest !== undefined) byData.delete(oldest);
  }
  byElement.set(element, bitmap);
  return bitmap;
}

// Drawing ---------------------------------------------------------------------

/**
 * Writes the black dots of `element` that fall inside the band (paper rows
 * `[top, top + rows)`, `width` dots wide) into `pixels` (RGBA, one pixel per
 * dot), in `ink`. Dots outside the paper are clipped.
 */
export function overlayImage(
  pixels: Uint8ClampedArray,
  width: number,
  top: number,
  rows: number,
  element: ImageElement,
  ink: Rgba,
): void {
  const bitmap = imageBitmap(element);
  const left = Math.round(element.x);
  const y = Math.round(element.y);
  const firstRow = Math.max(0, top - y);
  const lastRow = Math.min(bitmap.height, top + rows - y);
  if (firstRow >= lastRow) return;
  // Only bytes that can reach the paper.
  const firstByte = Math.max(0, Math.floor(-left / 8));
  const lastByte = Math.min(bitmap.stride, Math.ceil((width - left) / 8));
  const [r, g, b, a] = ink;

  for (let row = firstRow; row < lastRow; row++) {
    const src = row * bitmap.stride;
    const dst = (y + row - top) * width;
    for (let byte = firstByte; byte < lastByte; byte++) {
      const value = bitmap.bits[src + byte];
      if (value === 0) continue;
      for (let bit = 0; bit < 8; bit++) {
        if ((value & (0x80 >> bit)) === 0) continue;
        const column = byte * 8 + bit;
        if (column >= bitmap.width) break;
        const x = left + column;
        if (x < 0 || x >= width) continue;
        const i = (dst + x) * 4;
        pixels[i] = r;
        pixels[i + 1] = g;
        pixels[i + 2] = b;
        pixels[i + 3] = a;
      }
    }
  }
}

/**
 * Draws a bitmap alone at an integer scale (`scale` x `scale` pixels per
 * dot), black on white, for the code preview.
 */
export function drawScaled(canvas: HTMLCanvasElement, element: ImageElement, scale: number): void {
  const bitmap = imageBitmap(element);
  const k = Math.max(1, Math.floor(scale));
  canvas.width = bitmap.width * k;
  canvas.height = bitmap.height * k;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  const image = ctx.createImageData(canvas.width, canvas.height);
  const data = image.data;
  data.fill(255);
  const rowPixels = canvas.width;
  for (let row = 0; row < bitmap.height; row++) {
    for (let column = 0; column < bitmap.width; column++) {
      const value = bitmap.bits[row * bitmap.stride + (column >> 3)];
      if ((value & (0x80 >> (column & 7))) === 0) continue;
      for (let dy = 0; dy < k; dy++) {
        let i = ((row * k + dy) * rowPixels + column * k) * 4;
        for (let dx = 0; dx < k; dx++, i += 4) {
          data[i] = 0;
          data[i + 1] = 0;
          data[i + 2] = 0;
        }
      }
    }
  }
  ctx.putImageData(image, 0, 0);
}
