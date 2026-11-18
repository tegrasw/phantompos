<!--
  Data received while an error state was simulated: logged, not printed. Sits
  in the roll where it would have printed. Deliberately quiet (a dashed note on
  the desk, not paper) and tinted with --fault; consecutive reads of the same
  connection and reason are coalesced by the Roll, so the total and the time
  range grow in place.
-->
<script lang="ts">
  import { fade } from "svelte/transition";
  import { formatBytes, formatClock } from "../format";
  import { messages } from "../messages";
  import { duration } from "../motion";
  import type { SuppressedNotice } from "../roll.svelte";
  import Icon from "./Icon.svelte";

  let { notice }: { notice: SuppressedNotice } = $props();
  const t = messages.roll;

  const bytes = $derived(notice.bytes < 1024 ? t.bytes(notice.bytes) : formatBytes(notice.bytes));
  const reason = $derived(messages.printer.faultsLower[notice.reason] ?? notice.reason);
  const first = $derived(formatClock(notice.firstAt));
  const last = $derived(formatClock(notice.lastAt));
  const range = $derived(first === last ? first : `${first}–${last}`);
</script>

<div class="notice" role="note" aria-label={t.suppressedLabel(bytes, reason, notice.peer)} in:fade={{ duration: duration("base") }}>
  <span class="icon" aria-hidden="true"><Icon name="pause" size={10} /></span>
  <span class="text">
    <span class="title">{t.suppressedTitle(bytes, reason)}</span>
    <span class="detail">
      <span class="peer" title={t.suppressedDetailTitle(notice.peer, notice.connectionId)}>{t.suppressedDetail(notice.peer, notice.connectionId)}</span>
      <span class="sep" aria-hidden="true">&middot;</span>
      <time>{range}</time>
      {#if notice.reads > 1}
        <span class="sep" aria-hidden="true">&middot;</span>
        <span>{t.suppressedReads(notice.reads)}</span>
      {/if}
    </span>
  </span>
</div>

<style>
  .notice {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
    width: min(var(--paper-span, 100%), 100%);
    margin: var(--space-2) auto;
    padding: var(--space-2) var(--space-3);
    border: 1px dashed var(--fault-border);
    border-radius: var(--radius-md);
    background: color-mix(in srgb, var(--bg-app) 70%, var(--fault-soft));
  }
  .icon {
    display: grid;
    place-items: center;
    flex: none;
    width: 16px;
    height: 16px;
    margin-top: 1px;
    border-radius: 50%;
    background: var(--fault-soft);
    box-shadow: inset 0 0 0 1px var(--fault-border);
    color: var(--fault);
  }
  .icon :global(.icon) {
    stroke-width: 2.25;
  }
  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .title {
    color: var(--fault);
    font-size: var(--text-sm);
    font-weight: var(--weight-semibold);
    font-variant-numeric: tabular-nums;
  }
  .detail {
    display: flex;
    flex-wrap: wrap;
    column-gap: var(--space-1-5);
    color: var(--text-tertiary);
    font-size: var(--text-xs);
    font-variant-numeric: tabular-nums;
  }
  .peer {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .sep {
    opacity: 0.6;
  }
</style>
