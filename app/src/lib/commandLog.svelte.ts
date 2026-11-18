// Bounded, batched log of decoded commands, grouped by connection.
//
// `commands:received` batches are queued and flushed into reactive state at
// most once per FLUSH_INTERVAL_MS, so a burst of socket reads causes one render,
// not hundreds. A timer is used instead of requestAnimationFrame because rAF is
// suspended while the window is hidden, which would let the queue grow without
// bound. Consecutive batches from the same connection extend the same group.
// Retention is capped by row count and by total bytes; the oldest rows go
// first. Everything a row needs for display is computed once, on arrival.
//
// Huge commands (a raster image or QR data can be tens of kilobytes): a row
// keeps only the first HEX_RETAINED_BYTES of its bytes, which is also all that
// expanding the row can show. The log is for reading, not for recovering the
// stream; the raw-bytes export (stage 6) comes from the backend. The byte cap
// counts what is retained, so one 50 KB image does not evict 50 KB of other
// rows; `byteLength` and the statistics keep the real sizes.
//
// Status replies (`status:sent`, printer → host) go through the same queue, so
// they land in arrival order, in the group of their connection, as `reply`
// rows. They count as rows for retention but not as received commands or
// bytes; the statistics count them separately.

import type { CommandsReceived, DecodedCommand, StatusSent } from "./backend";
import {
  commandSummary,
  displayParams,
  flagLabel,
  flagTone,
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
/** Bytes a row keeps (and shows when expanded); the rest is only counted. */
export const HEX_RETAINED_BYTES = 256;
/** Flushes larger than this skip the row entrance animation. */
const ANIMATE_MAX_ROWS = 24;
const FLUSH_INTERVAL_MS = 16;

interface RowBase {
  /** Monotonic key for keyed rendering. */
  key: number;
  /** Stream offset (host → printer rows); null for replies. */
  offset: number | null;
  /** The first HEX_RETAINED_BYTES bytes of the command. */
  bytes: number[];
  /** Length of the whole command in bytes. */
  byteLength: number;
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
  | (RowBase & {
      kind: "command";
      type: string;
      label: string;
      /** Friendly one-line reading of the parameters, e.g. "576 × 120 dots". */
      summary: string | null;
      params: DisplayParam[];
    })
  | (RowBase & {
      kind: "reply";
      replyKind: string;
      /** What was answered, e.g. "DLE EOT 1". */
      request: string;
      flags: ReplyFlag[];
    });

export interface ReplyFlag {
  code: string;
  label: string;
  tone: "fault" | "ok" | "neutral";
}

/** The most recent reply, for the printer state panel. */
export interface LastReply {
  request: string;
  bytes: number[];
  flags: ReplyFlag[];
  at: Date;
  peer: string;
}

export interface LogGroup {
  key: number;
  connectionId: number;
  peer: string;
  startedAt: Date;
  rows: LogRow[];
  /** Bytes received (replies not included). */
  byteCount: number;
  unknownCount: number;
  replyCount: number;
}

export interface SessionStats {
  commands: number;
  bytes: number;
  unknown: number;
  /** Status replies sent by the printer. */
  replies: number;
  connections: number;
  lastAt: Date | null;
  lastPeer: string | null;
}

const EMPTY_STATS: SessionStats = {
  commands: 0,
  bytes: 0,
  unknown: 0,
  replies: 0,
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
  const all = decoded.bytes;
  const bytes = all.length > HEX_RETAINED_BYTES ? all.slice(0, HEX_RETAINED_BYTES) : all;
  const base: RowBase = {
    key,
    offset: decoded.offset,
    bytes,
    byteLength: all.length,
    hexPreview: hexLines(bytes, HEX_PREVIEW_BYTES),
    hiddenBytes: Math.max(0, all.length - HEX_PREVIEW_BYTES),
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
  const { summary, skip } = commandSummary(command);
  return {
    ...base,
    kind: "command",
    type: command.type,
    label: humanize(command.type),
    summary,
    params: displayParams(command, skip),
  };
}

function toReplyRow(sent: StatusSent, key: number, enter: boolean): LogRow {
  const all = sent.reply.bytes;
  const bytes = all.length > HEX_RETAINED_BYTES ? all.slice(0, HEX_RETAINED_BYTES) : all;
  return {
    key,
    offset: null,
    bytes,
    byteLength: all.length,
    hexPreview: hexLines(bytes, HEX_PREVIEW_BYTES),
    hiddenBytes: Math.max(0, all.length - HEX_PREVIEW_BYTES),
    mnemonic: null,
    enter,
    kind: "reply",
    replyKind: sent.reply.kind,
    request: sent.request,
    flags: replyFlags(sent.reply.flags),
  };
}

export function replyFlags(codes: readonly string[]): ReplyFlag[] {
  return codes.map((code) => ({ code, label: flagLabel(code), tone: flagTone(code) }));
}

/** One queued event: a batch of decoded commands or one status reply. */
type Pending = { kind: "commands"; batch: CommandsReceived } | { kind: "reply"; sent: StatusSent };

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
  /** The most recent status reply (kept across clears of the log). */
  lastReply: LastReply | null = $state.raw(null);

  #pending: Pending[] = [];
  #retainedBytes = 0;
  #nextKey = 0;
  #seenConnections = new Set<number>();
  #timer: ReturnType<typeof setTimeout> | null = null;

  push(batch: CommandsReceived): void {
    if (batch.commands.length === 0) return;
    this.#enqueue({ kind: "commands", batch });
  }

  pushReply(sent: StatusSent): void {
    this.#enqueue({ kind: "reply", sent });
  }

  #enqueue(item: Pending): void {
    this.#pending.push(item);
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

    let incomingRows = 0;
    let incoming = 0;
    for (const item of batches) {
      const n = item.kind === "commands" ? item.batch.commands.length : 1;
      incomingRows += n;
      if (item.kind === "commands") incoming += n;
    }
    const enter = incomingRows <= ANIMATE_MAX_ROWS;

    const groups = this.groups.slice();
    let rowCount = this.rowCount;
    let bytes = 0;
    let retained = 0;
    let unknown = 0;
    let replies = 0;
    let lastPeer = this.stats.lastPeer;
    let lastReply = this.lastReply;

    for (const item of batches) {
      const rows: LogRow[] = [];
      let batchBytes = 0;
      let batchRetained = 0;
      let batchUnknown = 0;
      const batch = item.kind === "commands" ? item.batch : item.sent;
      if (item.kind === "commands") {
        for (const decoded of item.batch.commands) {
          const row = toRow(decoded, this.#nextKey++, enter);
          rows.push(row);
          batchBytes += row.byteLength;
          batchRetained += row.bytes.length;
          if (row.kind === "unknown") batchUnknown++;
        }
        lastPeer = batch.peer;
      } else {
        const row = toReplyRow(item.sent, this.#nextKey++, enter);
        rows.push(row);
        batchRetained += row.bytes.length;
        replies++;
        if (row.kind === "reply")
          lastReply = { request: row.request, bytes: row.bytes, flags: row.flags, at: new Date(), peer: batch.peer };
      }
      this.#seenConnections.add(batch.connectionId);

      const last = groups.at(-1);
      if (last && last.connectionId === batch.connectionId) {
        groups[groups.length - 1] = {
          ...last,
          rows: last.rows.concat(rows),
          byteCount: last.byteCount + batchBytes,
          unknownCount: last.unknownCount + batchUnknown,
          replyCount: last.replyCount + (item.kind === "reply" ? 1 : 0),
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
          replyCount: item.kind === "reply" ? 1 : 0,
        });
      }
      rowCount += rows.length;
      bytes += batchBytes;
      retained += batchRetained;
      unknown += batchUnknown;
    }

    this.#retainedBytes += retained;
    this.ingested += incomingRows;
    this.lastReply = lastReply;
    this.stats = {
      commands: this.stats.commands + incoming,
      bytes: this.stats.bytes + bytes,
      unknown: this.stats.unknown + unknown,
      replies: this.stats.replies + replies,
      connections: this.#seenConnections.size,
      lastAt: incoming > 0 ? new Date() : this.stats.lastAt,
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
      let takeRetained = 0;
      let takeUnknown = 0;
      let takeReplies = 0;
      while (
        take < oldest.rows.length &&
        (take < excessRows || this.#retainedBytes - takeRetained > MAX_BYTES) &&
        rowCount - take > 1
      ) {
        const row = oldest.rows[take];
        if (row.kind === "reply") takeReplies++;
        else takeBytes += row.byteLength;
        takeRetained += row.bytes.length;
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
          replyCount: oldest.replyCount - takeReplies,
        };
      rowCount -= take;
      dropped += take;
      this.#retainedBytes -= takeRetained;
    }

    this.groups = groups;
    this.rowCount = rowCount;
    if (dropped > 0) this.dropped += dropped;
  }
}
