// Bounded, batched log of received byte chunks.
//
// Incoming chunks are queued and flushed into reactive state at most once per
// FLUSH_INTERVAL_MS, so a burst of socket reads causes one render, not hundreds.
// A timer is used instead of requestAnimationFrame because rAF is suspended
// while the window is hidden, which would let the queue grow without bound.
// Retention is capped by chunk count and by total bytes.

import type { BytesReceived } from "./backend";
import { hexDump } from "./hexdump";

export interface LogEntry {
  /** Monotonic key for keyed rendering. */
  key: number;
  receivedAt: Date;
  connectionId: number;
  peer: string;
  byteCount: number;
  /** Pre-rendered hex dump, computed once on arrival. */
  dump: string;
}

export const MAX_ENTRIES = 2000;
export const MAX_BYTES = 2 * 1024 * 1024;
const FLUSH_INTERVAL_MS = 16;

export class ByteLog {
  /** Retained entries, oldest first. Replaced (never mutated) on each flush. */
  entries: LogEntry[] = $state.raw([]);
  /** Number of chunks discarded because of the retention caps. */
  dropped = $state(0);

  #pending: LogEntry[] = [];
  #retainedBytes = 0;
  #nextKey = 0;
  #timer: ReturnType<typeof setTimeout> | null = null;

  push(chunk: BytesReceived): void {
    this.#pending.push({
      key: this.#nextKey++,
      receivedAt: new Date(),
      connectionId: chunk.connectionId,
      peer: chunk.peer,
      byteCount: chunk.bytes.length,
      dump: hexDump(chunk.bytes),
    });
    this.#timer ??= setTimeout(() => this.#flush(), FLUSH_INTERVAL_MS);
  }

  clear(): void {
    this.#cancelTimer();
    this.#pending = [];
    this.#retainedBytes = 0;
    this.entries = [];
    this.dropped = 0;
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
    if (this.#pending.length === 0) return;

    const next = this.entries.concat(this.#pending);
    for (const entry of this.#pending) this.#retainedBytes += entry.byteCount;
    this.#pending = [];

    // Drop oldest entries until both caps hold (always keep the newest one).
    let drop = Math.max(0, next.length - MAX_ENTRIES);
    let bytes = this.#retainedBytes;
    for (let i = 0; i < drop; i++) bytes -= next[i].byteCount;
    while (bytes > MAX_BYTES && drop < next.length - 1) {
      bytes -= next[drop].byteCount;
      drop++;
    }

    this.#retainedBytes = bytes;
    this.entries = drop > 0 ? next.slice(drop) : next;
    if (drop > 0) this.dropped += drop;
  }
}
