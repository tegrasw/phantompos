// Receipt renderer: draws a Receipt (core's model, in printer dots) onto a
// canvas whose bitmap is exactly one pixel per dot, then reduces it to two
// colours, like thermal print. Pure module: no Svelte, no DOM besides the
// canvas it is given, so the PNG export (stage 6) can reuse it.
//
// Text: each character is drawn alone, centred in its cell (`span.x +
// i * charWidth`), so columns follow the printer grid whatever the font does.
// The font size makes the glyph advance equal the cell width (JetBrains Mono
// advance is 0.6 em: 20 px for font A's 12-dot cell, 15 px for font B's 9),
// stretched a little vertically to match the tall printer cells; width and
// height multipliers scale the glyph with the canvas transform, the way a
// printer enlarges its character pattern. Glyphs sit on the line's baseline.
// Bold is the 700 weight double-struck one dot to the right (printers
// emphasise by striking twice). Reverse fills the cells first and draws the
// glyphs in paper colour. Underline is 1 or 2 dots at the bottom of the cells.
// Box drawing and block characters are drawn as dots (see blocks.ts).
//
// Anti-aliased pixels are then thresholded: a pixel is ink when at least half
// of it is covered, otherwise paper. The result has exactly two colours.

import type { LineElement, Receipt, Span } from "../backend";
import { drawBlockChar, isBlockChar } from "./blocks";
import { RECEIPT_FONT_STACK, receiptFontReady } from "./font";

/** [red, green, blue, alpha], 0-255. */
export type Rgba = readonly [number, number, number, number];

export interface RenderColors {
  ink: Rgba;
  paper: Rgba;
}

/** On screen: near-black ink on a transparent bitmap (the page draws the paper). */
export const SCREEN_COLORS: RenderColors = { ink: [22, 23, 26, 255], paper: [0, 0, 0, 0] };
/** For export: black on opaque white. */
export const EXPORT_COLORS: RenderColors = { ink: [0, 0, 0, 255], paper: [255, 255, 255, 255] };

/** Font size (px) per printer font: glyph advance 0.6 em = cell width. */
const FONT_EM = { a: 20, b: 15 } as const;
/** Printer cells are taller than the font's proportions. */
const VERTICAL_STRETCH = 1.1;
/** Coverage (0-255 darkness) from which an anti-aliased pixel becomes ink. */
const INK_THRESHOLD = 128;
/** Glyph overflow past a line (descenders, tall accents), in dots per multiplier. */
const OVERFLOW_DOTS = 8;

const DRAW_INK = "#000";
const DRAW_PAPER = "#fff";

const isLine = (element: { type: string }): element is LineElement => element.type === "line";

/**
 * Draws rows `[top, top + height)` of the receipt into `canvas`, resizing its
 * bitmap to `receipt.width` x `height`. Elements of unknown types are skipped.
 */
export function renderBand(
  canvas: HTMLCanvasElement | OffscreenCanvas,
  receipt: Receipt,
  top: number,
  height: number,
  colors: RenderColors = SCREEN_COLORS,
): void {
  const width = Math.max(1, receipt.width);
  const rows = Math.max(1, Math.ceil(height));
  if (canvas.width !== width) canvas.width = width;
  if (canvas.height !== rows) canvas.height = rows;
  const ctx = canvas.getContext("2d", { willReadFrequently: true }) as
    | CanvasRenderingContext2D
    | null;
  if (!ctx) return;

  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.globalAlpha = 1;
  ctx.globalCompositeOperation = "source-over";
  ctx.fillStyle = DRAW_PAPER;
  ctx.fillRect(0, 0, width, rows);
  ctx.textAlign = "center";
  ctx.textBaseline = "alphabetic";

  const bottom = top + rows;
  for (const element of receipt.elements) {
    if (!isLine(element) || element.spans.length === 0) continue;
    const tallest = element.spans.reduce((m, s) => Math.max(m, s.style.heightMultiplier), 1);
    const reach = Math.max(element.height, element.advance) + OVERFLOW_DOTS * tallest;
    if (element.y + reach <= top || element.y - OVERFLOW_DOTS * tallest >= bottom) continue;
    drawLine(ctx, element, top);
  }

  threshold(ctx, width, rows, colors);
}

/** Renders the whole receipt on a new canvas (for export). */
export function renderReceipt(receipt: Receipt, colors: RenderColors = EXPORT_COLORS): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  renderBand(canvas, receipt, 0, receipt.height, colors);
  return canvas;
}

function drawLine(ctx: CanvasRenderingContext2D, line: LineElement, top: number): void {
  const baseline = line.y - top + line.baseline;
  for (const span of line.spans) {
    const cellTop = line.y - top + span.y;
    if (span.style.reverse) {
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.fillStyle = DRAW_INK;
      ctx.fillRect(span.x, cellTop, span.width, span.charHeight);
    }
    drawSpanText(ctx, span, cellTop, baseline, line.y + span.y);
    const thickness = span.style.underline === "twoDots" ? 2 : span.style.underline === "oneDot" ? 1 : 0;
    if (thickness > 0) {
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.fillStyle = DRAW_INK;
      ctx.fillRect(span.x, cellTop + span.charHeight - thickness, span.width, thickness);
    }
  }
}

function drawSpanText(
  ctx: CanvasRenderingContext2D,
  span: Span,
  cellTop: number,
  baseline: number,
  paperTop: number,
): void {
  const { style, charWidth, charHeight } = span;
  const ink = style.reverse ? DRAW_PAPER : DRAW_INK;
  const background = style.reverse ? DRAW_INK : DRAW_PAPER;
  const em = FONT_EM[style.font] ?? FONT_EM.a;
  const font = `${style.bold ? 700 : 400} ${em}px ${RECEIPT_FONT_STACK}`;
  const advance = em * 0.6;
  const sx = style.widthMultiplier;
  const sy = style.heightMultiplier * VERTICAL_STRETCH;
  const strike = style.bold ? Math.max(1, style.widthMultiplier) : 0;

  let column = 0;
  for (const char of span.text) {
    const x = span.x + column * charWidth;
    column++;
    const cp = char.codePointAt(0) ?? 0x20;
    if (cp === 0x20 || cp === 0xa0) continue;

    if (isBlockChar(cp)) {
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      drawBlockChar(
        ctx,
        cp,
        {
          x,
          y: cellTop,
          w: charWidth,
          h: charHeight,
          widthMultiplier: style.widthMultiplier,
          heightMultiplier: style.heightMultiplier,
          paperY: paperTop,
        },
        ink,
        background,
      );
      continue;
    }

    ctx.font = font;
    ctx.fillStyle = ink;
    // Glyphs from fallback fonts may be wider than the cell: squeeze them.
    const fit = Math.min(1, advance / glyphWidth(ctx, font, char));
    const centre = x + charWidth / 2;
    ctx.setTransform(sx * fit, 0, 0, sy, centre, baseline);
    ctx.fillText(char, 0, 0);
    if (strike) {
      ctx.setTransform(sx * fit, 0, 0, sy, centre + strike, baseline);
      ctx.fillText(char, 0, 0);
    }
  }
  ctx.setTransform(1, 0, 0, 1, 0, 0);
}

const widths = new Map<string, number>();

function glyphWidth(ctx: CanvasRenderingContext2D, font: string, char: string): number {
  const key = `${font}|${char}`;
  let width = widths.get(key);
  if (width === undefined) {
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    width = Math.max(1, ctx.measureText(char).width);
    widths.set(key, width);
  }
  return width;
}

let ready: Promise<void> | null = null;

/**
 * Resolves when the receipt font is loaded (or given up on). Draw after it,
 * otherwise the first bands use the fallback font.
 */
export function rendererReady(): Promise<void> {
  ready ??= receiptFontReady().then(() => widths.clear());
  return ready;
}

/** Reduces the anti-aliased black-on-white drawing to exactly two colours. */
function threshold(ctx: CanvasRenderingContext2D, width: number, height: number, colors: RenderColors): void {
  const image = ctx.getImageData(0, 0, width, height);
  const data = image.data;
  const [ir, ig, ib, ia] = colors.ink;
  const [pr, pg, pb, pa] = colors.paper;
  for (let i = 0; i < data.length; i += 4) {
    const darkness = 255 - ((data[i] * 77 + data[i + 1] * 150 + data[i + 2] * 29) >> 8);
    if (darkness >= INK_THRESHOLD) {
      data[i] = ir;
      data[i + 1] = ig;
      data[i + 2] = ib;
      data[i + 3] = ia;
    } else {
      data[i] = pr;
      data[i + 1] = pg;
      data[i + 2] = pb;
      data[i + 3] = pa;
    }
  }
  ctx.putImageData(image, 0, 0);
}
