// The paper roll: completed receipts in arrival order plus the in-progress
// receipt of each printing connection.
//
// `receipt:updated` snapshots can arrive many times per second per
// connection; events are queued and applied at most once per
// FLUSH_INTERVAL_MS (a timer, not requestAnimationFrame, which stops while the
// window is hidden), in order, so a burst causes one render. An in-progress
// receipt keeps its key when it completes, so the same component settles into
// the roll instead of being replaced.
//
// Data discarded while an error state is simulated (`print:suppressed`) goes
// into the same ordered flow as a notice, where it would have printed.
// Consecutive events of the same connection and reason (nothing else closed
// on the roll in between) extend one notice: a running byte total, the number
// of socket reads and the time range. Notices share the cap with receipts
// (they are rare once coalesced) and are cleared with the roll.

import type { PrintSuppressed, Receipt, ReceiptCompleted, ReceiptUpdated } from "./backend";

/** Entries (completed receipts and notices) kept; the oldest go beyond this. */
export const MAX_RECEIPTS = 200;
const FLUSH_INTERVAL_MS = 16;

export interface RollItem {
  /** Stable key for rendering (kept from in-progress to completed). */
  key: number;
  /** Backend id; null while in progress. */
  id: number | null;
  connectionId: number;
  peer: string;
  receipt: Receipt;
  live: boolean;
  startedAt: Date;
  completedAt: Date | null;
}

/** Data received in an error state and not printed. */
export interface SuppressedNotice {
  notice: true;
  key: number;
  connectionId: number;
  peer: string;
  reason: string;
  bytes: number;
  /** Socket reads coalesced into this notice. */
  reads: number;
  firstAt: Date;
  lastAt: Date;
}

/** One entry of the completed flow. */
export type RollEntry = RollItem | SuppressedNotice;

export function isNotice(entry: RollEntry): entry is SuppressedNotice {
  return "notice" in entry;
}

type RollEvent =
  | { kind: "updated"; payload: ReceiptUpdated }
  | { kind: "completed"; payload: ReceiptCompleted }
  | { kind: "suppressed"; payload: PrintSuppressed; at: Date };

export class Roll {
  /** Completed receipts and notices, oldest first. Replaced (never mutated) on change. */
  completed: RollEntry[] = $state.raw([]);
  /** Completed receipts among `completed`. */
  receiptCount = $derived(this.completed.reduce((n, entry) => (isNotice(entry) ? n : n + 1), 0));
  /** In-progress receipts, in the order they started. */
  live: RollItem[] = $state.raw([]);
  /** Completed receipts removed by the cap since the last clear. */
  dropped = $state(0);
  /** Receipts completed since launch (monotonic; for "N new" counters). */
  completedTotal = $state(0);

  #queue: RollEvent[] = [];
  #timer: ReturnType<typeof setTimeout> | null = null;
  #nextKey = 1;
  /** Ids are unique and increasing: anything not above this is a duplicate. */
  #lastId = -Infinity;

  update(payload: ReceiptUpdated): void {
    this.#enqueue({ kind: "updated", payload });
  }

  complete(payload: ReceiptCompleted): void {
    this.#enqueue({ kind: "completed", payload });
  }

  suppressed(payload: PrintSuppressed): void {
    this.#enqueue({ kind: "suppressed", payload, at: new Date() });
  }

  /** Removes the completed receipts and notices; receipts still printing stay. */
  clear(): void {
    this.#flush();
    this.completed = [];
    this.dropped = 0;
  }

  dispose(): void {
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = null;
    this.#queue = [];
  }

  #enqueue(event: RollEvent): void {
    this.#queue.push(event);
    this.#timer ??= setTimeout(() => this.#flush(), FLUSH_INTERVAL_MS);
  }

  #flush(): void {
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = null;
    const events = this.#queue;
    if (events.length === 0) return;
    this.#queue = [];

    const live = this.live.slice();
    let completed: RollEntry[] | null = null;
    let added = 0;

    for (const event of events) {
      const { connectionId, peer } = event.payload;
      if (event.kind === "suppressed") {
        completed ??= this.completed.slice();
        const { reason, bytes } = event.payload;
        const last = completed.at(-1);
        if (last && isNotice(last) && last.connectionId === connectionId && last.reason === reason) {
          completed[completed.length - 1] = {
            ...last,
            peer,
            bytes: last.bytes + bytes,
            reads: last.reads + 1,
            lastAt: event.at,
          };
        } else {
          completed.push({
            notice: true,
            key: this.#nextKey++,
            connectionId,
            peer,
            reason,
            bytes,
            reads: 1,
            firstAt: event.at,
            lastAt: event.at,
          });
        }
        continue;
      }
      const index = live.findIndex((item) => item.connectionId === connectionId);
      if (event.kind === "updated") {
        const receipt = event.payload.receipt;
        if (index >= 0) live[index] = { ...live[index], peer, receipt };
        else
          live.push({
            key: this.#nextKey++,
            id: null,
            connectionId,
            peer,
            receipt,
            live: true,
            startedAt: new Date(),
            completedAt: null,
          });
        continue;
      }

      const { id, receipt } = event.payload;
      const previous = index >= 0 ? live.splice(index, 1)[0] : null;
      if (id <= this.#lastId) continue;
      this.#lastId = id;
      completed ??= this.completed.slice();
      completed.push({
        key: previous?.key ?? this.#nextKey++,
        id,
        connectionId,
        peer,
        receipt: { ...receipt, cut: receipt.cut ?? null },
        live: false,
        startedAt: previous?.startedAt ?? new Date(),
        completedAt: new Date(),
      });
      added++;
    }

    this.live = live;
    if (completed) {
      const excess = completed.length - MAX_RECEIPTS;
      if (excess > 0) {
        // Only receipts count as "removed" for the user; old notices just go.
        const removed = completed.splice(0, excess);
        this.dropped += removed.filter((entry) => !isNotice(entry)).length;
      }
      this.completed = completed;
      this.completedTotal += added;
    }
  }
}
