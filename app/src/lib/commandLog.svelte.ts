// Bounded, batched log of decoded commands, grouped by connection.
//
// `commands:received` batches are queued and flushed into reactive state at
// most once per FLUSH_INTERVAL_MS, so a burst of socket reads causes one render,
// not hundreds. A timer is used instead of requestAnimationFrame because rAF is
// suspended while the window is hidden, which would let the queue grow without
// bound. Consecutive batches from the same connection extend the same group.
// Retention is capped by row count and by total bytes; the oldest rows go
// first. Everything a row needs for display is computed once, on arrival.

import type { CommandsReceived, DecodedCommand } from "./backend";
import {
  displayParams,
  hexByte,
  humanize,
  mnemonic,
  textSegments,
  type DisplayParam,
  type TextSegment,
} from "./escpos";

export const MAX_ROWS = 5000;
export const MAX_BYTES = 2 * 1024 * 1024;
/** Hex shown per line in the log. */
export const HEX_BYTES_PER_LINE = 8;
/** Hex shown before a row offers "show all". */
export const HEX_PREVIEW_BYTES = 32;
/** Flushes larger than this skip the row entrance animation. */
const ANIMATE_MAX_ROWS = 24;
const FLUSH_INTERVAL_MS = 16;

interface RowBase {
  /** Monotonic key for keyed rendering. */
  key: number;
  offset: number;
  bytes: number[];
  /** Hex preview lines (at most HEX_PREVIEW_BYTES bytes). */
  hexPreview: string[];
  /** Bytes not shown in the preview. */
  hiddenBytes: number;
  mnemonic: string | null;
  /** True when the row should play its entrance animation. */
  enter: boolean;
}

export type LogRow =
  | (RowBase & { kind: "text"; segments: TextSegment[]; charCount: number })
  | (RowBase & { kind: "unknown"; reason: string | null; params: DisplayParam[] })
  | (RowBase & { kind: "command"; type: string; label: string; params: DisplayParam[] });

export interface LogGroup {
  key: number;
  connectionId: number;
  peer: string;
  startedAt: Date;
  rows: LogRow[];
  byteCount: number;
  unknownCount: number;
}

export interface SessionStats {
  commands: number;
  bytes: number;
  unknown: number;
  connections: number;
  lastAt: Date | null;
  lastPeer: string | null;
}

const EMPTY_STATS: SessionStats = {
  commands: 0,
  bytes: 0,
  unknown: 0,
  connections: 0,
  lastAt: null,
  lastPeer: null,
};

export function hexLines(bytes: readonly number[], limit = bytes.length): string[] {
  const lines: string[] = [];
  const end = Math.min(limit, bytes.length);
  for (let start = 0; start < end; start += HEX_BYTES_PER_LINE) {
    let line = "";
    const stop = Math.min(start + HEX_BYTES_PER_LINE, end);
    for (let i = start; i < stop; i++) line += (i > start ? " " : "") + hexByte(bytes[i]);
    lines.push(line);
  }
  return lines;
}

function toRow(decoded: DecodedCommand, key: number, enter: boolean): LogRow {
  const bytes = decoded.bytes;
  const base: RowBase = {
    key,
    offset: decoded.offset,
    bytes,
    hexPreview: hexLines(bytes, HEX_PREVIEW_BYTES),
    hiddenBytes: Math.max(0, bytes.length - HEX_PREVIEW_BYTES),
    mnemonic: mnemonic(bytes),
    enter,
  };
  const command = decoded.command;
  if (command.type === "text") {
    const text = typeof command.text === "string" ? command.text : "";
    return { ...base, kind: "text", segments: textSegments(text), charCount: text.length };
  }
  if (command.type === "unknown") {
    const reason = typeof command.reason === "string" ? humanize(command.reason).toLowerCase() : null;
    return { ...base, kind: "unknown", reason, params: displayParams(command, ["reason"]) };
  }
  return {
    ...base,
    kind: "command",
    type: command.type,
    label: humanize(command.type),
    params: displayParams(command),
  };
}

export class CommandLog {
  /** Retained groups, oldest first. Replaced (never mutated) on each flush. */
  groups: LogGroup[] = $state.raw([]);
  /** Rows currently retained. */
  rowCount = $state(0);
  /** Rows discarded because of the retention caps. */
  dropped = $state(0);
  /** Rows ever ingested (monotonic; used for "N new" counters). */
  ingested = $state(0);
  /** Unknown sequences among the retained rows. */
  retainedUnknown = $derived(this.groups.reduce((sum, group) => sum + group.unknownCount, 0));
  /** Totals since the last clear. */
  stats: SessionStats = $state.raw(EMPTY_STATS);

  #pending: CommandsReceived[] = [];
  #retainedBytes = 0;
  #nextKey = 0;
  #seenConnections = new Set<number>();
  #timer: ReturnType<typeof setTimeout> | null = null;

  push(batch: CommandsReceived): void {
    if (batch.commands.length === 0) return;
    this.#pending.push(batch);
    this.#timer ??= setTimeout(() => this.#flush(), FLUSH_INTERVAL_MS);
  }

  clear(): void {
    this.#cancelTimer();
    this.#pending = [];
    this.#retainedBytes = 0;
    this.#seenConnections.clear();
    this.groups = [];
    this.rowCount = 0;
    this.dropped = 0;
    this.stats = EMPTY_STATS;
  }

  dispose(): void {
    this.#cancelTimer();
    this.#pending = [];
  }

  #cancelTimer(): void {
    if (this.#timer !== null) {
      clearTimeout(this.#timer);
      this.#timer = null;
    }
  }

  #flush(): void {
    this.#timer = null;
    const batches = this.#pending;
    if (batches.length === 0) return;
    this.#pending = [];

    let incoming = 0;
    for (const batch of batches) incoming += batch.commands.length;
    const enter = incoming <= ANIMATE_MAX_ROWS;

    const groups = this.groups.slice();
    let rowCount = this.rowCount;
    let bytes = 0;
    let unknown = 0;
    let lastPeer = this.stats.lastPeer;

    for (const batch of batches) {
      const rows: LogRow[] = [];
      let batchBytes = 0;
      let batchUnknown = 0;
      for (const decoded of batch.commands) {
        const row = toRow(decoded, this.#nextKey++, enter);
        rows.push(row);
        batchBytes += decoded.bytes.length;
        if (row.kind === "unknown") batchUnknown++;
      }
      this.#seenConnections.add(batch.connectionId);
      lastPeer = batch.peer;

      const last = groups.at(-1);
      if (last && last.connectionId === batch.connectionId) {
        groups[groups.length - 1] = {
          ...last,
          rows: last.rows.concat(rows),
          byteCount: last.byteCount + batchBytes,
          unknownCount: last.unknownCount + batchUnknown,
        };
      } else {
        groups.push({
          key: this.#nextKey++,
          connectionId: batch.connectionId,
          peer: batch.peer,
          startedAt: new Date(),
          rows,
          byteCount: batchBytes,
          unknownCount: batchUnknown,
        });
      }
      rowCount += rows.length;
      bytes += batchBytes;
      unknown += batchUnknown;
    }

    this.#retainedBytes += bytes;
    this.ingested += incoming;
    this.stats = {
      commands: this.stats.commands + incoming,
      bytes: this.stats.bytes + bytes,
      unknown: this.stats.unknown + unknown,
      connections: this.#seenConnections.size,
      lastAt: new Date(),
      lastPeer,
    };

    // Enforce the caps by dropping the oldest rows (always keep the newest).
    let dropped = 0;
    while (
      (rowCount > MAX_ROWS || this.#retainedBytes > MAX_BYTES) &&
      rowCount > 1
    ) {
      const oldest = groups[0];
      const excessRows = rowCount - MAX_ROWS;
      let take = 0;
      let takeBytes = 0;
      let takeUnknown = 0;
      while (
        take < oldest.rows.length &&
        (take < excessRows || this.#retainedBytes - takeBytes > MAX_BYTES) &&
        rowCount - take > 1
      ) {
        const row = oldest.rows[take];
        takeBytes += row.bytes.length;
        if (row.kind === "unknown") takeUnknown++;
        take++;
      }
      if (take === 0) break;
      if (take === oldest.rows.length) groups.shift();
      else
        groups[0] = {
          ...oldest,
          rows: oldest.rows.slice(take),
          byteCount: oldest.byteCount - takeBytes,
          unknownCount: oldest.unknownCount - takeUnknown,
        };
      rowCount -= take;
      dropped += take;
      this.#retainedBytes -= takeBytes;
    }

    this.groups = groups;
    this.rowCount = rowCount;
    if (dropped > 0) this.dropped += dropped;
  }
}
