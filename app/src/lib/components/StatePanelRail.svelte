<!--
  Collapsed printer state panel: a slim rail that keeps the state readable (a
  status dot, one mark per active fault, the open drawer) and reopens the
  panel.
-->
<script lang="ts">
  import { activeFaults, type PrinterStatus } from "../backend";
  import { messages } from "../messages";
  import Icon from "./Icon.svelte";

  let { printer, onExpand }: { printer: PrinterStatus; onExpand: () => void } = $props();
  const t = messages.printer;

  const faults = $derived(activeFaults(printer));
  const title = $derived(
    faults.length > 0 ? `${t.expand} · ${t.list(faults.map((f) => t.faults[f]))}` : t.expand,
  );
</script>

<button
  type="button"
  class="rail"
  data-fault={faults.length > 0}
  {title}
  aria-label={title}
  aria-controls="printer-state"
  onclick={onExpand}
>
  <span class="icon">
    <Icon name="printer" />
    <span class="dot" aria-hidden="true"></span>
  </span>
  {#each faults as fault (fault)}
    <span class="fault" aria-hidden="true">{t.faults[fault]}</span>
  {/each}
  {#if printer.drawerOpen}
    <span class="drawer" aria-hidden="true"><Icon name="drawerOpen" size={14} /></span>
  {/if}
  <span class="label">{t.railLabel}</span>
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
  .dot {
    position: absolute;
    top: 4px;
    right: 3px;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 2px var(--bg-surface);
  }
  [data-fault="true"] .dot {
    background: var(--fault);
  }
  .fault {
    writing-mode: vertical-rl;
    padding: var(--space-1-5) 0;
    width: 20px;
    border: 1px solid var(--fault-border);
    border-radius: var(--radius-pill);
    background: var(--fault-soft);
    color: var(--fault);
    font-size: var(--text-2xs);
    font-weight: var(--weight-semibold);
    line-height: 18px;
    white-space: nowrap;
  }
  .drawer {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: var(--radius-sm);
    background: var(--accent-soft);
    color: var(--accent);
  }
  .label {
    writing-mode: vertical-rl;
    font-size: var(--text-xs);
    font-weight: var(--weight-medium);
    letter-spacing: 0.02em;
  }
</style>
