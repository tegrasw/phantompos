// Pure hex-dump formatting, in the style of `hexdump -C`:
//
//   00000000  48 65 6c 6c 6f 2c 20 77  6f 72 6c 64 21 0a        |Hello, world!.|
//
// Offset (8 hex digits), 16 bytes per row split into two groups of 8, and an
// ASCII gutter where anything outside printable ASCII (0x20..0x7e) is a dot.

export const BYTES_PER_ROW = 16;

const HEX: readonly string[] = Array.from({ length: 256 }, (_, i) =>
  i.toString(16).padStart(2, "0"),
);

/** Width of the hex column: 16 * "xx " plus the extra group separator. */
const HEX_COLUMN_WIDTH = BYTES_PER_ROW * 3 + 1;

function asciiChar(byte: number): string {
  return byte >= 0x20 && byte <= 0x7e ? String.fromCharCode(byte) : ".";
}

/** Formats a single row starting at `start` (at most 16 bytes). */
export function formatHexRow(bytes: ArrayLike<number>, start: number): string {
  const end = Math.min(start + BYTES_PER_ROW, bytes.length);
  let hex = "";
  let ascii = "";
  for (let i = start; i < end; i++) {
    const byte = bytes[i] & 0xff;
    hex += HEX[byte] + " ";
    if (i - start === 7) hex += " ";
    ascii += asciiChar(byte);
  }
  const offset = start.toString(16).padStart(8, "0");
  return `${offset}  ${hex.padEnd(HEX_COLUMN_WIDTH)} |${ascii}|`;
}

/** Formats a whole buffer as hex-dump rows. Empty input yields no rows. */
export function hexDumpLines(bytes: ArrayLike<number>): string[] {
  const lines: string[] = [];
  for (let start = 0; start < bytes.length; start += BYTES_PER_ROW) {
    lines.push(formatHexRow(bytes, start));
  }
  return lines;
}

/** Formats a whole buffer as a single newline-separated string. */
export function hexDump(bytes: ArrayLike<number>): string {
  return hexDumpLines(bytes).join("\n");
}
