// Loads the receipt font (a JetBrains Mono subset covering every character of
// the supported code pages; see src/assets/fonts/README.md) under its own
// family name, so it never interferes with the UI font.

import regularUrl from "../../assets/fonts/jetbrains-mono-receipt-400.woff2";
import boldUrl from "../../assets/fonts/jetbrains-mono-receipt-700.woff2";

export const RECEIPT_FONT_FAMILY = "PhantomPOS Receipt";

/** Font stack for canvas text: system monospace fonts cover missing glyphs. */
export const RECEIPT_FONT_STACK = `"${RECEIPT_FONT_FAMILY}", ui-monospace, Menlo, Consolas, "DejaVu Sans Mono", monospace`;

/** Gives up waiting after this long and draws with the fallback fonts. */
const LOAD_TIMEOUT_MS = 4000;

let ready: Promise<void> | null = null;

/**
 * Resolves once both weights are usable by canvas text (or loading failed or
 * timed out, in which case the fallback fonts are used). Idempotent.
 */
export function receiptFontReady(): Promise<void> {
  ready ??= load();
  return ready;
}

async function load(): Promise<void> {
  if (typeof FontFace === "undefined" || !document.fonts) return;
  const faces = [
    new FontFace(RECEIPT_FONT_FAMILY, `url(${regularUrl}) format("woff2")`, { weight: "400" }),
    new FontFace(RECEIPT_FONT_FAMILY, `url(${boldUrl}) format("woff2")`, { weight: "700" }),
  ];
  const loading = Promise.all(
    faces.map(async (face) => {
      document.fonts.add(face);
      await face.load();
    }),
  );
  const timeout = new Promise<void>((resolve) => setTimeout(resolve, LOAD_TIMEOUT_MS));
  try {
    await Promise.race([loading, timeout]);
  } catch {
    // Draw with the fallback fonts.
  }
}
