// Small formatting helpers shared by the UI. Numbers, sizes and times follow
// the interface locale through `Intl` (grouping and decimal separators);
// clocks always use 24-hour time, which reads best in a log.

import { locale } from "./i18n/locale.svelte";

const cache = new Map<string, unknown>();

function numberFormat(key: string, options: Intl.NumberFormatOptions): Intl.NumberFormat {
  const id = `${locale.tag}|n|${key}`;
  let format = cache.get(id) as Intl.NumberFormat | undefined;
  if (!format) {
    format = new Intl.NumberFormat(locale.tag, options);
    cache.set(id, format);
  }
  return format;
}

function dateFormat(key: string, options: Intl.DateTimeFormatOptions): Intl.DateTimeFormat {
  const id = `${locale.tag}|d|${key}`;
  let format = cache.get(id) as Intl.DateTimeFormat | undefined;
  if (!format) {
    format = new Intl.DateTimeFormat(locale.tag, options);
    cache.set(id, format);
  }
  return format;
}

const CLOCK: Intl.DateTimeFormatOptions = { hour: "2-digit", minute: "2-digit", second: "2-digit", hourCycle: "h23" };

/** 14:03:27.081 */
export function formatTime(date: Date): string {
  return dateFormat("time", { ...CLOCK, fractionalSecondDigits: 3 }).format(date);
}

/** 14:03:27 */
export function formatClock(date: Date): string {
  return dateFormat("clock", CLOCK).format(date);
}

/** 12,345 (en) or 12.345 (pt). */
export function formatCount(value: number): string {
  return numberFormat("count", { maximumFractionDigits: 0 }).format(value);
}

/** 912 B, 14.2 KB, 1.3 MB (decimal separator per locale). */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${formatCount(bytes)} B`;
  const one = numberFormat("bytes", { minimumFractionDigits: 1, maximumFractionDigits: 1 });
  if (bytes < 1024 * 1024) return `${one.format(bytes / 1024)} KB`;
  return `${one.format(bytes / (1024 * 1024))} MB`;
}

/** Stream offset as at least four hex digits: 0x00A4 → "00A4". */
export function formatOffset(offset: number): string {
  return offset.toString(16).toUpperCase().padStart(4, "0");
}

/** "1 receipt", "3 receipts": the count formatted for the locale. */
export function plural(n: number, one: string, many: string): string {
  return `${formatCount(n)} ${n === 1 ? one : many}`;
}

/** "a, b and c" in the interface language. */
export function formatList(items: readonly string[]): string {
  try {
    const id = `${locale.tag}|list`;
    let format = cache.get(id) as Intl.ListFormat | undefined;
    if (!format) {
      format = new Intl.ListFormat(locale.tag, { style: "long", type: "conjunction" });
      cache.set(id, format);
    }
    return format.format(items);
  } catch {
    return items.join(", ");
  }
}

/** The file name of a path from the native save dialog (any separator). */
export function baseName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
}
