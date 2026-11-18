<!-- Collapsed command log: a slim rail that keeps the count visible and reopens the panel. -->
<script lang="ts">
  import type { CommandLog } from "../commandLog.svelte";
  import { formatCount } from "../format";
  import { i18n } from "../i18n";
  import Icon from "./Icon.svelte";

  let { log, onExpand }: { log: CommandLog; onExpand: () => void } = $props();
  const t = $derived(i18n.t.log);
</script>

<button type="button" class="rail" title={t.expand} aria-label={t.expand} aria-controls="command-log" onclick={onExpand}>
  <span class="icon">
    <Icon name="list" />
    {#if log.retainedUnknown > 0}<span class="warn-dot" aria-hidden="true"></span>{/if}
  </span>
  {#if log.rowCount > 0}
    <span class="badge">{formatCount(log.rowCount)}</span>
  {/if}
  <span class="label">{t.title}</span>
</button>

<style>
  .rail {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-2);
    width: 100%;
    height: 100%;
    padding: var(--space-3) 0;
    border: 0;
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    transition:
      background-color var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }
  .rail:hover {
    background: var(--bg-hover);
    color: var(--text);
  }
  .rail:focus-visible {
    outline-offset: -2px;
  }
  .icon {
    position: relative;
    display: grid;
    place-items: center;
    width: var(--control-h);
    height: var(--control-h);
  }
  .warn-dot {
    position: absolute;
    top: 4px;
    right: 3px;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--warning);
    box-shadow: 0 0 0 2px var(--bg-surface);
  }
  .badge {
    max-width: calc(var(--rail-w) - 8px);
    padding: 0 var(--space-1);
    border-radius: var(--radius-pill);
    background: var(--accent-soft);
    color: var(--accent);
    font-size: var(--text-2xs);
    font-weight: var(--weight-semibold);
    line-height: 16px;
    font-variant-numeric: tabular-nums;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .label {
    writing-mode: vertical-rl;
    font-size: var(--text-xs);
    font-weight: var(--weight-medium);
    letter-spacing: 0.02em;
  }
</style>
