// Box drawing (U+2500-257F), block elements (U+2580-259F) and U+25A0 drawn as
// dot rectangles on the cell grid instead of font glyphs. Fonts draw these
// with their own line height, so on a 12x24 printer cell the strokes would not
// meet; drawing them like a printer's character generator guarantees that
// adjacent cells connect and that shades tile seamlessly.
//
// Every coordinate here is an integer dot. Callers set nothing up: the colour
// is passed in, the context transform must be the identity.

/**
 * Arm weights per box-drawing character, as "LRUD" digits: 0 none, 1 light,
 * 2 double, 3 heavy. Generated from the Unicode character names (dashed,
 * rounded and diagonal forms are left to the font).
 */
const BOX_TABLE =
  "2500:1100 2501:3300 2502:0011 2503:0033 250c:0101 250d:0301 250e:0103 250f:0303 2510:1001 " +
  "2511:3001 2512:1003 2513:3003 2514:0110 2515:0310 2516:0130 2517:0330 2518:1010 2519:3010 " +
  "251a:1030 251b:3030 251c:0111 251d:0311 251e:0131 251f:0113 2520:0133 2521:0331 2522:0313 " +
  "2523:0333 2524:1011 2525:3011 2526:1031 2527:1013 2528:1033 2529:3031 252a:3013 252b:3033 " +
  "252c:1101 252d:3101 252e:1301 252f:3301 2530:1103 2531:3103 2532:1303 2533:3303 2534:1110 " +
  "2535:3110 2536:1310 2537:3310 2538:1130 2539:3130 253a:1330 253b:3330 253c:1111 253d:3111 " +
  "253e:1311 253f:3311 2540:1131 2541:1113 2542:1133 2543:3131 2544:1331 2545:3113 2546:1313 " +
  "2547:3331 2548:3313 2549:3133 254a:1333 254b:3333 2550:2200 2551:0022 2552:0201 2553:0102 " +
  "2554:0202 2555:2001 2556:1002 2557:2002 2558:0210 2559:0120 255a:0220 255b:2010 255c:1020 " +
  "255d:2020 255e:0211 255f:0122 2560:0222 2561:2011 2562:1022 2563:2022 2564:2201 2565:1102 " +
  "2566:2202 2567:2210 2568:1120 2569:2220 256a:2211 256b:1122 256c:2222 2574:1000 2575:0010 " +
  "2576:0100 2577:0001 2578:3000 2579:0030 257a:0300 257b:0003";

type Weight = 0 | 1 | 2 | 3;
const NONE = 0;
const DOUBLE = 2;
const HEAVY = 3;

const BOX = new Map<number, [Weight, Weight, Weight, Weight]>(
  BOX_TABLE.split(" ").map((entry) => {
    const [cp, arms] = entry.split(":");
    return [parseInt(cp, 16), Array.from(arms, Number) as [Weight, Weight, Weight, Weight]];
  }),
);

/** A cell on the band, in dots. `phaseX/phaseY` place it on the paper for dithering. */
export interface Cell {
  x: number;
  y: number;
  w: number;
  h: number;
  widthMultiplier: number;
  heightMultiplier: number;
  /** Paper row of `y` (the band may start below the top of the receipt). */
  paperY: number;
}

/** True when `cp` is drawn by this module rather than by the font. */
export function isBlockChar(cp: number): boolean {
  return BOX.has(cp) || (cp >= 0x2580 && cp <= 0x259f) || cp === 0x25a0;
}

/**
 * Draws `cp` into `cell` with `ink`; `background` is used to cut the gaps of
 * double lines. Returns false if the character is not handled here.
 */
export function drawBlockChar(
  ctx: CanvasRenderingContext2D,
  cp: number,
  cell: Cell,
  ink: string,
  background: string,
): boolean {
  const arms = BOX.get(cp);
  if (arms) {
    drawBox(ctx, arms, cell, ink, background);
    return true;
  }
  if ((cp >= 0x2580 && cp <= 0x259f) || cp === 0x25a0) {
    ctx.fillStyle = ink;
    drawBlock(ctx, cp, cell);
    return true;
  }
  return false;
}

// ---------------------------------------------------------------------------
// Box drawing
// ---------------------------------------------------------------------------

/**
 * Lines are 2 dots thick per multiplier. A double line is a 3-stroke band with
 * the middle stroke cut out; joins come from overlapping the bands and then
 * cutting the gaps only where a double line actually runs.
 */
function drawBox(
  ctx: CanvasRenderingContext2D,
  [left, right, up, down]: [Weight, Weight, Weight, Weight],
  { x, y, w, h, widthMultiplier, heightMultiplier }: Cell,
  ink: string,
  background: string,
): void {
  const tv = 2 * widthMultiplier; // width of vertical strokes
  const th = 2 * heightMultiplier; // height of horizontal strokes
  const cx = x + Math.floor((w - tv) / 2); // left edge of the centre column
  const cy = y + Math.floor((h - th) / 2); // top edge of the centre row
  const ev = Math.max(1, Math.round(tv / 4)); // heavy extra, each side
  const eh = Math.max(1, Math.round(th / 4));

  const vWeight = Math.max(up, down) as Weight;
  const hWeight = Math.max(left, right) as Weight;

  // Extent of a vertical stroke of each weight: [start, end) columns.
  const colSpan = (weight: Weight): [number, number] =>
    weight === DOUBLE ? [cx - tv, cx + 2 * tv] : weight === HEAVY ? [cx - ev, cx + tv + ev] : [cx, cx + tv];
  const rowSpan = (weight: Weight): [number, number] =>
    weight === DOUBLE ? [cy - th, cy + 2 * th] : weight === HEAVY ? [cy - eh, cy + th + eh] : [cy, cy + th];

  const [vStructL, vStructR] = colSpan(vWeight);
  const [hStructT, hStructB] = rowSpan(hWeight);

  ctx.fillStyle = ink;
  const rect = (x0: number, y0: number, x1: number, y1: number): void => {
    if (x1 > x0 && y1 > y0) ctx.fillRect(x0, y0, x1 - x0, y1 - y0);
  };

  // Arms. A single arm meeting a double perpendicular line stops at its
  // nearer stroke; everything else runs into the far side of the junction.
  if (left) {
    const [t, b] = rowSpan(left);
    rect(x, t, vWeight === DOUBLE && left !== DOUBLE ? cx : vStructR, b);
  }
  if (right) {
    const [t, b] = rowSpan(right);
    rect(vWeight === DOUBLE && right !== DOUBLE ? cx + tv : vStructL, t, x + w, b);
  }
  if (up) {
    const [l, r] = colSpan(up);
    rect(l, y, r, hWeight === DOUBLE && up !== DOUBLE ? cy : hStructB);
  }
  if (down) {
    const [l, r] = colSpan(down);
    rect(l, hWeight === DOUBLE && down !== DOUBLE ? cy + th : hStructT, r, y + h);
  }

  // Gaps of double lines.
  ctx.fillStyle = background;
  const vSingle = vWeight !== NONE && vWeight !== DOUBLE;
  const hSingle = hWeight !== NONE && hWeight !== DOUBLE;
  // Keep the junction filled where a single line passes through, or where
  // the double line ends at it (corners and tees on the single line).
  const keepH = vSingle && ((up !== NONE && down !== NONE) || (left === DOUBLE) !== (right === DOUBLE));
  const keepV = hSingle && ((left !== NONE && right !== NONE) || (up === DOUBLE) !== (down === DOUBLE));
  if (left === DOUBLE) rect(x, cy, keepH ? vStructL : cx + tv, cy + th);
  if (right === DOUBLE) rect(keepH ? vStructR : cx, cy, x + w, cy + th);
  if (up === DOUBLE) rect(cx, y, cx + tv, keepV ? hStructT : cy + th);
  if (down === DOUBLE) rect(cx, keepV ? hStructB : cy, cx + tv, y + h);
}

// ---------------------------------------------------------------------------
// Blocks and shades
// ---------------------------------------------------------------------------

function drawBlock(ctx: CanvasRenderingContext2D, cp: number, cell: Cell): void {
  const { x, y, w, h } = cell;
  const hw = Math.round(w / 2);
  const hh = Math.round(h / 2);
  const rect = (x0: number, y0: number, ww: number, hh2: number): void => ctx.fillRect(x0, y0, ww, hh2);

  if (cp === 0x2580) return rect(x, y, w, hh); // upper half
  if (cp >= 0x2581 && cp <= 0x2588) {
    const n = Math.round((h * (cp - 0x2580)) / 8); // lower n/8
    return rect(x, y + h - n, w, n);
  }
  if (cp >= 0x2589 && cp <= 0x258f) {
    const n = Math.round((w * (0x2590 - cp)) / 8); // left n/8
    return rect(x, y, n, h);
  }
  if (cp === 0x2590) return rect(x + hw, y, w - hw, h); // right half
  if (cp >= 0x2591 && cp <= 0x2593) return shade(ctx, cp - 0x2590, cell);
  if (cp === 0x2594) return rect(x, y, w, Math.round(h / 8));
  if (cp === 0x2595) {
    const n = Math.round(w / 8);
    return rect(x + w - n, y, n, h);
  }
  if (cp === 0x25a0) {
    // Black square: centred on the x-height, like CP437's 0xFE.
    const sw = Math.round(w * 0.58);
    const sh = Math.round(h * 0.3);
    return rect(x + Math.floor((w - sw) / 2), y + Math.round(h * 0.56 - sh / 2), sw, sh);
  }
  // Quadrants U+2596-259F: bit 0 UL, 1 UR, 2 LL, 3 LR.
  const QUADRANTS = [0b0100, 0b1000, 0b0001, 0b1101, 0b1001, 0b0111, 0b1011, 0b0010, 0b0110, 0b1110];
  const q = QUADRANTS[cp - 0x2596];
  if (q === undefined) return;
  if (q & 1) rect(x, y, hw, hh);
  if (q & 2) rect(x + hw, y, w - hw, hh);
  if (q & 4) rect(x, y + hh, hw, h - hh);
  if (q & 8) rect(x + hw, y + hh, w - hw, h - hh);
}

/**
 * Light (25%), medium (50%) and dark (75%) shades as ordered dot patterns,
 * phased on paper coordinates so neighbouring cells and lines tile.
 * Multipliers enlarge the pattern dots, as a printer enlarges the glyph.
 */
function shade(ctx: CanvasRenderingContext2D, level: number, cell: Cell): void {
  const { x, y, w, h, widthMultiplier: mx, heightMultiplier: my, paperY } = cell;
  const on = (px: number, py: number): boolean => {
    const light = py % 2 === 0 ? px % 4 === 0 : px % 4 === 2;
    if (level === 1) return light;
    if (level === 2) return (px + py) % 2 === 0;
    return !light;
  };
  for (let dy = 0; dy < h; dy += my) {
    const py = Math.floor((paperY + dy) / my);
    for (let dx = 0; dx < w; dx += mx) {
      if (on(Math.floor((x + dx) / mx), py)) ctx.fillRect(x + dx, y + dy, mx, my);
    }
  }
}
