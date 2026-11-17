// Display-only helpers for decoded ESC/POS commands. Nothing here parses the
// byte stream: `phantompos-core` does that. These functions only turn what the
// parser produced into something readable in the command log.

import type { CommandBody, CommandParam } from "./backend";

const CONTROL_NAMES: readonly string[] = [
  "NUL", "SOH", "STX", "ETX", "EOT", "ENQ", "ACK", "BEL",
  "BS", "HT", "LF", "VT", "FF", "CR", "SO", "SI",
  "DLE", "DC1", "DC2", "DC3", "DC4", "NAK", "SYN", "ETB",
  "CAN", "EM", "SUB", "ESC", "FS", "GS", "RS", "US",
];

const HEX: readonly string[] = Array.from({ length: 256 }, (_, i) =>
  i.toString(16).toUpperCase().padStart(2, "0"),
);

/** Two-digit uppercase hex for a byte. */
export function hexByte(byte: number): string {
  return HEX[byte & 0xff];
}

/** ASCII control-character name (NUL…US, DEL) or undefined. */
export function controlName(byte: number): string | undefined {
  if (byte >= 0 && byte < 0x20) return CONTROL_NAMES[byte];
  if (byte === 0x7f) return "DEL";
  return undefined;
}

/** One byte as it is conventionally written in ESC/POS documentation. */
function token(byte: number): string {
  const name = controlName(byte);
  if (name) return name;
  if (byte === 0x20) return "SP";
  if (byte > 0x20 && byte < 0x7f) return String.fromCharCode(byte);
  return hexByte(byte);
}

const PREFIXES = new Set([0x10, 0x1b, 0x1c, 0x1d]); // DLE, ESC, FS, GS

/**
 * Conventional ESC/POS mnemonic for a command's leading bytes, e.g.
 * `1B 45 01` → "ESC E", `1D 21 11` → "GS !", `0A` → "LF", `1D 28 6B …` →
 * "GS ( k", `1D 76 30 …` → "GS v 0". Returns null for printable text.
 */
export function mnemonic(bytes: ArrayLike<number>): string | null {
  if (bytes.length === 0) return null;
  const first = bytes[0] & 0xff;
  if (PREFIXES.has(first)) {
    const parts = [token(first)];
    if (bytes.length > 1) {
      const second = bytes[1] & 0xff;
      parts.push(token(second));
      // Function-style commands carry a third selector byte.
      const third = bytes.length > 2 ? bytes[2] & 0xff : -1;
      if (second === 0x28 && third >= 0x21 && third < 0x7f) parts.push(token(third));
      else if (first === 0x1d && second === 0x76 && third === 0x30) parts.push("0");
    }
    return parts.join(" ");
  }
  return controlName(first) ?? null;
}

/** "setCharacterSize" → "Set character size"; "feedLines" → "Feed lines". */
export function humanize(identifier: string): string {
  const words = identifier
    .replace(/[_-]+/g, " ")
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .replace(/([A-Z]+)([A-Z][a-z])/g, "$1 $2")
    .trim()
    .toLowerCase();
  return words.charAt(0).toUpperCase() + words.slice(1);
}

export interface DisplayParam {
  key: string;
  value: string;
}

function formatValue(value: CommandParam | undefined): string {
  if (value === null || value === undefined) return "–";
  if (typeof value === "boolean") return value ? "on" : "off";
  if (typeof value === "number") return String(value);
  if (typeof value === "string") return value;
  // Not part of the contract, but never let an unexpected shape break the log.
  return JSON.stringify(value);
}

/** All parameters except `type` (and fields the caller renders itself). */
export function displayParams(
  command: CommandBody,
  skip: readonly string[] = [],
): DisplayParam[] {
  const params: DisplayParam[] = [];
  for (const [key, value] of Object.entries(command)) {
    if (key === "type" || skip.includes(key)) continue;
    params.push({ key: humanize(key).toLowerCase(), value: formatValue(value) });
  }
  return params;
}

export type TextSegment =
  | { kind: "text"; value: string }
  /** Leading or trailing spaces, made visible as middle dots. */
  | { kind: "space"; count: number }
  /** A control character inside the text, shown by its mnemonic. */
  | { kind: "control"; name: string };

function controlLabel(code: number): string | undefined {
  if (code < 0x20 || code === 0x7f) return controlName(code);
  if (code >= 0x80 && code <= 0x9f) return `U+${code.toString(16).toUpperCase().padStart(4, "0")}`;
  return undefined;
}

/**
 * Splits decoded text into display segments so whitespace and control
 * characters that would otherwise be invisible are shown explicitly.
 */
export function textSegments(text: string): TextSegment[] {
  const segments: TextSegment[] = [];
  const leading = text.length - text.trimStart().length;
  const trailingStart = text.trimEnd().length;
  let run = "";
  const flush = (): void => {
    if (run) segments.push({ kind: "text", value: run });
    run = "";
  };

  let i = 0;
  while (i < text.length) {
    const char = text[i];
    if (char === " " && (i < leading || i >= trailingStart)) {
      flush();
      let count = 0;
      while (i < text.length && text[i] === " " && (i < leading || i >= trailingStart)) {
        count++;
        i++;
      }
      segments.push({ kind: "space", count });
      continue;
    }
    const label = controlLabel(char.charCodeAt(0));
    if (label) {
      flush();
      segments.push({ kind: "control", name: label });
    } else {
      run += char;
    }
    i++;
  }
  flush();
  return segments;
}
