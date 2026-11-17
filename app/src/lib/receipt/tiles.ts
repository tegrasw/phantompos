// Shows one receipt as a column of canvas tiles.
//
// Receipts can be arbitrarily long, and browsers limit canvas size, so the
// bitmap is split into bands of TILE_DOTS rows. Tiles far outside the
// viewport release their bitmap (a 576 x 1024 tile is 2.3 MB) and are redrawn
// when they come back. While a receipt is printing, only the tiles from the
// first changed line down are redrawn, at most once per animation frame.

import type { Receipt } from "../backend";
import { renderBand, rendererReady } from "./render";

export const TILE_DOTS = 1024;
/** Lines can draw this far above their own top (tall glyphs of the next line). */
const REDRAW_MARGIN_DOTS = 64;
/** Keep bitmaps for tiles within this distance of the viewport. */
const KEEP_MARGIN = "1500px 0px";

interface Tile {
  canvas: HTMLCanvasElement;
  top: number;
  rows: number;
  visible: boolean;
  dirty: boolean;
}

export class ReceiptTiles {
  #host: HTMLElement;
  #tiles: Tile[] = [];
  #receipt: Receipt | null = null;
  #zoom = 1;
  #observer: IntersectionObserver;
  #frame = 0;
  #ready = false;
  #destroyed = false;

  constructor(host: HTMLElement, scrollRoot: Element | null) {
    this.#host = host;
    this.#observer = new IntersectionObserver((entries) => this.#onIntersect(entries), {
      root: scrollRoot,
      rootMargin: KEEP_MARGIN,
    });
    void rendererReady().then(() => {
      if (this.#destroyed) return;
      this.#ready = true;
      for (const tile of this.#tiles) tile.dirty = true;
      this.#schedule();
    });
  }

  set(receipt: Receipt, zoom: number): void {
    const previous = this.#receipt;
    this.#receipt = receipt;
    const zoomChanged = zoom !== this.#zoom;
    this.#zoom = zoom;

    const dirtyFrom = previous ? firstChangedRow(previous, receipt) : 0;
    const count = receipt.height > 0 ? Math.ceil(receipt.height / TILE_DOTS) : 0;

    while (this.#tiles.length > count) {
      const tile = this.#tiles.pop();
      if (tile) {
        this.#observer.unobserve(tile.canvas);
        tile.canvas.remove();
      }
    }
    for (let i = 0; i < count; i++) {
      const top = i * TILE_DOTS;
      const rows = Math.min(TILE_DOTS, receipt.height - top);
      let tile = this.#tiles[i];
      if (!tile) {
        const canvas = document.createElement("canvas");
        canvas.width = 0;
        canvas.height = 0;
        canvas.setAttribute("aria-hidden", "true");
        this.#host.append(canvas);
        tile = { canvas, top, rows, visible: false, dirty: true };
        this.#tiles.push(tile);
        this.#observer.observe(canvas);
      }
      if (tile.rows !== rows || top + rows + REDRAW_MARGIN_DOTS > dirtyFrom) tile.dirty = true;
      tile.rows = rows;
      if (zoomChanged || tile.dirty) this.#place(tile, receipt.width);
    }
    this.#schedule();
  }

  destroy(): void {
    this.#destroyed = true;
    cancelAnimationFrame(this.#frame);
    this.#observer.disconnect();
    for (const tile of this.#tiles) tile.canvas.remove();
    this.#tiles = [];
  }

  #place(tile: Tile, width: number): void {
    const style = tile.canvas.style;
    style.top = `${tile.top * this.#zoom}px`;
    style.width = `${width * this.#zoom}px`;
    style.height = `${tile.rows * this.#zoom}px`;
  }

  #onIntersect(entries: IntersectionObserverEntry[]): void {
    for (const entry of entries) {
      const tile = this.#tiles.find((t) => t.canvas === entry.target);
      if (!tile) continue;
      tile.visible = entry.isIntersecting;
      if (!tile.visible && tile.canvas.width > 0) {
        // Release the bitmap; it is redrawn when the tile comes back.
        tile.canvas.width = 0;
        tile.canvas.height = 0;
        tile.dirty = true;
      }
    }
    this.#schedule();
  }

  #schedule(): void {
    if (this.#frame || !this.#ready || this.#destroyed) return;
    this.#frame = requestAnimationFrame(() => {
      this.#frame = 0;
      this.#draw();
    });
  }

  #draw(): void {
    const receipt = this.#receipt;
    if (!receipt) return;
    for (const tile of this.#tiles) {
      if (!tile.visible || !tile.dirty) continue;
      renderBand(tile.canvas, receipt, tile.top, tile.rows);
      tile.dirty = false;
    }
  }
}

/**
 * First paper row that may differ between two snapshots of the same receipt.
 * In-progress snapshots only append lines, so normally only the last old line
 * and what follows are redrawn; anything else redraws everything.
 */
function firstChangedRow(previous: Receipt, next: Receipt): number {
  if (previous === next) return Infinity;
  if (previous.width !== next.width) return 0;
  const count = previous.elements.length;
  if (count === 0) return 0;
  if (next.elements.length < count) return 0;
  const last = previous.elements[count - 1];
  const same = JSON.stringify(last) === JSON.stringify(next.elements[count - 1]);
  const y = typeof last.y === "number" ? last.y : 0;
  return same ? y : 0;
}
