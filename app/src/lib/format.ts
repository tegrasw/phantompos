// Small pure formatting helpers shared by the UI.

const pad = (value: number, width = 2): string =>
  String(value).padStart(width, "0");

/** 14:03:27.081 */
export function formatTime(date: Date): string {
  return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}.${pad(date.getMilliseconds(), 3)}`;
}

/** 14:03:27 */
export function formatClock(date: Date): string {
  return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}

const integer = new Intl.NumberFormat("en");

export function formatCount(value: number): string {
  return integer.format(value);
}

/** 912 B, 14.2 KB, 1.3 MB */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Stream offset as at least four hex digits: 0x00A4 → "00A4". */
export function formatOffset(offset: number): string {
  return offset.toString(16).toUpperCase().padStart(4, "0");
}
