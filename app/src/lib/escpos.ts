// Display-only helpers for decoded ESC/POS commands. Nothing here parses the
// byte stream: `phantompos-core` does that. These functions only turn what the
// parser produced into something readable in the command log.

import type { CommandBody, CommandParam } from "./backend";
import { messages } from "./messages";

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
 * `1B 45 01` → "ESC E", `1D 21 11` → "GS !", `0A` → "LF". Function-style
 * commands keep their selector: `1D 28 6B …` → "GS ( k", `1D 28 4C …` →
 * "GS ( L", `1D 38 4C …` → "GS 8 L", `1D 76 30 …` → "GS v 0". Image and code
 * commands read naturally from their bytes: "GS k", "GS h", "GS w", "GS H",
 * "GS f", "ESC *". Status commands read the same way: `10 04 01` → "DLE EOT",
 * `10 05 02` → "DLE ENQ", `10 14 01 …` → "DLE DC4", `1D 72 01` → "GS r",
 * `1D 61 FF` → "GS a" (their parameter is shown by the decoded command).
 * Returns null for printable text.
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
      else if (first === 0x1d && second === 0x38 && third === 0x4c) parts.push("L");
    }
    return parts.join(" ");
  }
  return controlName(first) ?? null;
}

/** Abbreviations that read better in capitals ("Qr print" → "QR print"). */
const ACRONYMS = /\b(qr|hri|nv|pdf417|upc|ean|itf|gs1)\b/g;

/**
 * "setCharacterSize" → "Set character size"; "feedLines" → "Feed lines";
 * "qrPrint" → "QR print".
 */
export function humanize(identifier: string): string {
  const words = identifier
    .replace(/[_-]+/g, " ")
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .replace(/([A-Z]+)([A-Z][a-z])/g, "$1 $2")
    .trim()
    .toLowerCase()
    .replace(ACRONYMS, (word) => word.toUpperCase())
    .replace(/\breal time\b/g, "real-time");
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

const SYMBOLOGIES: Record<string, string> = {
  upcA: "UPC-A",
  upcE: "UPC-E",
  ean13: "EAN-13",
  jan13: "JAN-13",
  ean8: "EAN-8",
  jan8: "JAN-8",
  code39: "CODE39",
  itf: "ITF",
  codabar: "CODABAR",
  nw7: "NW-7",
  code93: "CODE93",
  code128: "CODE128",
  gs1128: "GS1-128",
  gs1DataBarOmnidirectional: "GS1 DataBar Omnidirectional",
  gs1DataBarTruncated: "GS1 DataBar Truncated",
  gs1DataBarLimited: "GS1 DataBar Limited",
  gs1DataBarExpanded: "GS1 DataBar Expanded",
  code128Auto: "CODE128 (auto)",
};

/** "ean13" → "EAN-13"; unknown names are humanised. */
export function symbologyName(symbology: string): string {
  return SYMBOLOGIES[symbology] ?? SYMBOLOGIES[symbology.toLowerCase()] ?? humanize(symbology);
}

const IMAGE_TYPE = /image|raster|graphic/i;

/**
 * A friendlier one-line reading of some commands' parameters (images and
 * codes), plus the parameter keys it already covers. Everything else is shown
 * generically. Parameter names are matched loosely so a renamed field simply
 * falls back to the generic display.
 */
export function commandSummary(command: CommandBody): { summary: string | null; skip: string[] } {
  const { type } = command;
  const width = command.width;
  const height = command.height;
  if (IMAGE_TYPE.test(type) && typeof width === "number" && typeof height === "number") {
    return { summary: messages.log.summary.dots(width, height), skip: ["width", "height"] };
  }
  if (/barcode/i.test(type) && typeof command.symbology === "string") {
    const data = typeof command.data === "string" ? command.data : null;
    return {
      summary: messages.log.summary.barcode(symbologyName(command.symbology), data, command.valid === false),
      skip: ["symbology", "data", "valid"],
    };
  }
  if (/^qr/i.test(type) && /store|data/i.test(type) && typeof command.length === "number") {
    return { summary: messages.log.summary.storedData(command.length), skip: ["length"] };
  }
  if (/status/i.test(type)) {
    // GS a: the mask's bits select what Automatic Status Back reports.
    if (typeof command.mask === "number") {
      const names = messages.log.asbBits.filter((_, bit) => (command.mask as number) & (1 << bit));
      return { summary: messages.log.asbMask(names), skip: ["mask"] };
    }
    // DLE EOT n / GS r n: which status is requested ("printer", "paperSensor"...).
    for (const key of STATUS_REQUEST_KEYS) {
      const value = command[key];
      if (typeof value === "string") return { summary: humanize(value).toLowerCase(), skip: [key] };
    }
  }
  return { summary: null, skip: [] };
}

/** Parameter names that may carry the kind of status a command requests. */
const STATUS_REQUEST_KEYS = ["request", "status", "kind", "target", "which"] as const;

// ---------------------------------------------------------------------------
// Status replies (printer → host)
// ---------------------------------------------------------------------------

/** "coverOpen" → "Cover open"; unknown codes are humanised. */
export function flagLabel(code: string): string {
  return messages.log.flags[code] ?? humanize(code);
}

/** Flags describing a condition that stops printing (or an error). */
const FAULT_FLAG = /offline|open|end|error|stop|fail|waiting/i;
const OK_FLAG = /^(online|paperPresent|paperAdequate|coverClosed)$/;

/** How a reply flag is tinted: a fault, a healthy condition, or neutral. */
export function flagTone(code: string): "fault" | "ok" | "neutral" {
  if (OK_FLAG.test(code)) return "ok";
  if (/nearEnd/i.test(code)) return "neutral";
  return FAULT_FLAG.test(code) ? "fault" : "neutral";
}
