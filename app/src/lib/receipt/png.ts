// PNG export of a receipt: exactly `width` x `height` dots, one pixel per dot,
// pure black on white, drawn by the same renderer as the screen
// (`renderBand` with EXPORT_COLORS).
//
// Canvas size limits: browsers cap a single canvas (Chromium/WebView2 at
// 65,535 px per side, WebKitGTK through Cairo at 32,767, iOS-class WebKit
// at 16,777,216 px of area, and memory pressure can fail earlier), so one
// canvas plus `toBlob()` cannot hold an arbitrarily long receipt. Instead the
// receipt is drawn in bands of BAND_DOTS rows on one small canvas, each band
// packed to 1 bit per dot and streamed through `CompressionStream("deflate")`
// (zlib, exactly what PNG's IDAT wants) into a grayscale 1-bit PNG. There is
// no height limit beyond memory for the (tiny) compressed output, and the
// file is a fraction of an RGBA PNG's size.
//
// Fallback, where CompressionStream is missing (WebKit before Safari 16.4 /
// macOS 13.3): the whole receipt on one canvas and `canvas.toBlob()`, limited
// to MAX_CANVAS_ROWS rows (2 m of paper at 8 dots/mm), safely inside every
// engine's limits; taller receipts fail with ReceiptTooTallError.

import type { Receipt } from "../backend";
import { EXPORT_COLORS, renderBand, renderReceipt, rendererReady } from "./render";

const BAND_DOTS = 1024;
/** Rows the single-canvas fallback accepts. */
export const MAX_CANVAS_ROWS = 16384;

export class ReceiptTooTallError extends Error {
  constructor(readonly maxRows: number) {
    super(`receipt taller than ${maxRows} dots`);
  }
}

/** Encodes the receipt as PNG file bytes. */
export async function receiptPng(receipt: Receipt): Promise<Uint8Array> {
  await rendererReady();
  if (typeof CompressionStream === "function") return bandedPng(receipt);
  return canvasPng(receipt);
}

async function bandedPng(receipt: Receipt): Promise<Uint8Array> {
  const width = Math.max(1, receipt.width);
  const height = Math.max(1, Math.ceil(receipt.height));
  const stride = Math.ceil(width / 8);

  const deflate = new CompressionStream("deflate");
  const compressed = new Response(deflate.readable).arrayBuffer();
  const writer = deflate.writable.getWriter();

  const canvas = document.createElement("canvas");
  for (let top = 0; top < height; top += BAND_DOTS) {
    const rows = Math.min(BAND_DOTS, height - top);
    renderBand(canvas, receipt, top, rows, EXPORT_COLORS);
    const ctx = canvas.getContext("2d", { willReadFrequently: true }) as CanvasRenderingContext2D | null;
    if (!ctx) throw new Error("canvas 2d context unavailable");
    const pixels = ctx.getImageData(0, 0, width, rows).data;
    // Each scanline: filter type 0 (None), then 1 bit per dot, MSB first,
    // 1 = white (grayscale), padded with white.
    const band = new Uint8Array(rows * (stride + 1));
    for (let y = 0; y < rows; y++) {
      const line = y * (stride + 1);
      band[line] = 0;
      for (let byte = 0; byte < stride; byte++) {
        let bits = 0;
        for (let bit = 0; bit < 8; bit++) {
          const x = byte * 8 + bit;
          const white = x >= width || pixels[(y * width + x) * 4] >= 128;
          if (white) bits |= 0x80 >> bit;
        }
        band[line + 1 + byte] = bits;
      }
    }
    await writer.write(band);
  }
  await writer.close();
  const idat = new Uint8Array(await compressed);

  const header = new Uint8Array(13);
  const view = new DataView(header.buffer);
  view.setUint32(0, width);
  view.setUint32(4, height);
  header[8] = 1; // bit depth
  header[9] = 0; // colour type: grayscale
  header[10] = 0; // compression: deflate
  header[11] = 0; // filter method
  header[12] = 0; // no interlace
  return concat([SIGNATURE, chunk("IHDR", header), chunk("IDAT", idat), chunk("IEND", new Uint8Array(0))]);
}

async function canvasPng(receipt: Receipt): Promise<Uint8Array> {
  if (Math.ceil(receipt.height) > MAX_CANVAS_ROWS) throw new ReceiptTooTallError(MAX_CANVAS_ROWS);
  const canvas = renderReceipt(receipt, EXPORT_COLORS);
  const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/png"));
  canvas.width = canvas.height = 0;
  if (!blob) throw new Error("canvas.toBlob returned no image");
  return new Uint8Array(await blob.arrayBuffer());
}

const SIGNATURE = new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]);

function chunk(type: string, data: Uint8Array): Uint8Array {
  const out = new Uint8Array(12 + data.length);
  const view = new DataView(out.buffer);
  view.setUint32(0, data.length);
  for (let i = 0; i < 4; i++) out[4 + i] = type.charCodeAt(i);
  out.set(data, 8);
  view.setUint32(8 + data.length, crc32(out.subarray(4, 8 + data.length)));
  return out;
}

function concat(parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((n, part) => n + part.length, 0));
  let offset = 0;
  for (const part of parts) {
    out.set(part, offset);
    offset += part.length;
  }
  return out;
}

let crcTable: Uint32Array | null = null;

function crc32(bytes: Uint8Array): number {
  if (!crcTable) {
    crcTable = new Uint32Array(256);
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      crcTable[n] = c >>> 0;
    }
  }
  let crc = 0xffffffff;
  for (const byte of bytes) crc = crcTable[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  return (crc ^ 0xffffffff) >>> 0;
}
