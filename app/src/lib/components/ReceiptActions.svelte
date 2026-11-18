<!--
  Save menu of a completed receipt, in its caption row (SPEC "Ações": save the
  receipt as PNG, export the raw bytes). A small download button that stays
  discreet until the receipt is hovered or focused (always visible on touch
  screens and while busy), and opens a two-item menu: "Save as PNG" and
  "Export raw bytes". Keyboard: Enter/Space/ArrowDown open it on the first
  item, arrows move, Esc closes and returns focus. While a save runs, the
  button shows a spinner and is disabled.
-->
<script lang="ts">
  import type { ExportFormat } from "../backend";
  import { i18n } from "../i18n";
  import Icon from "./Icon.svelte";
  import Popover from "./Popover.svelte";

  let {
    id,
    width,
    height,
    onSave,
  }: {
    /** Backend id of the completed receipt. */
    id: number;
    /** Receipt size in dots (shown in the PNG item). */
    width: number;
    height: number;
    /** Resolves when the save finished, was cancelled or failed (App reports it). */
    onSave: (format: ExportFormat) => Promise<void>;
  } = $props();

  const t = $derived(i18n.t.receiptExport);
  let trigger: HTMLButtonElement | undefined = $state();
  let open = $state(false);
  let busy = $state(false);

  async function choose(format: ExportFormat): Promise<void> {
    open = false;
    trigger?.focus();
    busy = true;
    try {
      await onSave(format);
    } finally {
      busy = false;
    }
  }

  function onTriggerKey(event: KeyboardEvent): void {
    if (event.key === "ArrowDown" && !open && !busy) {
      event.preventDefault();
      open = true;
    }
  }

  function onMenuKey(event: KeyboardEvent): void {
    const items = Array.from(
      (event.currentTarget as HTMLElement).querySelectorAll<HTMLElement>('[role="menuitem"]'),
    );
    const index = items.indexOf(document.activeElement as HTMLElement);
    let next = -1;
    if (event.key === "ArrowDown") next = (index + 1) % items.length;
    else if (event.key === "ArrowUp") next = (index - 1 + items.length) % items.length;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = items.length - 1;
    else if (event.key === "Tab") {
      open = false;
      return;
    } else return;
    event.preventDefault();
    items[next]?.focus();
  }
</script>

<span class="actions" class:active={open || busy}>
  <button
    type="button"
    class="trigger"
    bind:this={trigger}
    aria-haspopup="menu"
    aria-expanded={open}
    aria-label={t.menu(id)}
    aria-busy={busy}
    title={busy ? t.saving : t.menuTitle}
    disabled={busy}
    onclick={() => (open = !open)}
    onkeydown={onTriggerKey}
  >
    {#if busy}<span class="spinner" aria-hidden="true"></span>{:else}<Icon name="download" size={13} />{/if}
  </button>
  {#if busy}<span class="visually-hidden" role="status">{t.saving}</span>{/if}
</span>

{#if open && trigger}
  <Popover anchor={trigger} align="end" role="menu" label={t.menu(id)} onClose={() => (open = false)}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="menu" onkeydown={onMenuKey}>
      <button type="button" role="menuitem" class="item" onclick={() => choose("png")}>
        <span class="glyph"><Icon name="image" size={15} /></span>
        <span class="text">
          <span class="title">{t.png}</span>
          <span class="detail">{t.pngDetail(width, height)}</span>
        </span>
      </button>
      <button type="button" role="menuitem" class="item" onclick={() => choose("bin")}>
        <span class="glyph"><Icon name="fileBin" size={15} /></span>
        <span class="text">
          <span class="title">{t.bin}</span>
          <span class="detail">{t.binDetail}</span>
        </span>
      </button>
    </div>
  </Popover>
{/if}

<style>
  .actions {
    display: inline-flex;
    flex: none;
    margin-left: var(--space-0-5);
    font-family: var(--font-ui);
  }
  .trigger {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    padding: 0;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-tertiary);
    cursor: pointer;
    opacity: 0;
    transition:
      opacity var(--dur-fast) var(--ease-out),
      background-color var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }
  /* Discreet until the receipt is hovered or something in it has focus. */
  :global(.receipt:hover) .trigger,
  :global(.receipt:focus-within) .trigger,
  .active .trigger,
  .trigger:focus-visible {
    opacity: 1;
  }
  @media (hover: none) {
    .trigger {
      opacity: 1;
    }
  }
  .trigger:hover:not(:disabled),
  .trigger[aria-expanded="true"] {
    background: var(--bg-active);
    color: var(--text);
  }
  .trigger:focus-visible {
    /* The caption clips overflow: keep the ring inside the button. */
    outline-offset: -2px;
  }
  .trigger:disabled {
    cursor: default;
    color: var(--accent);
  }
  .trigger :global(.spinner) {
    width: 11px;
    height: 11px;
  }

  .menu {
    display: flex;
    flex-direction: column;
    gap: 1px;
    width: 280px;
    max-width: 100%;
  }
  .item {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2-5, 10px);
    width: 100%;
    padding: var(--space-2) var(--space-2-5, 10px) var(--space-2) var(--space-2);
    border: 0;
    border-radius: var(--radius-md);
    background: transparent;
    text-align: left;
    cursor: pointer;
    transition: background-color var(--dur-instant) var(--ease-out);
  }
  .item:hover,
  .item:focus-visible {
    background: var(--bg-hover);
    outline: none;
  }
  .item:focus-visible {
    box-shadow: inset 0 0 0 1.5px var(--focus-ring);
  }
  .glyph {
    display: grid;
    place-items: center;
    flex: none;
    width: 28px;
    height: 28px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-sunken);
    color: var(--text-secondary);
  }
  .item:hover .glyph,
  .item:focus-visible .glyph {
    border-color: var(--accent-border);
    background: var(--accent-soft);
    color: var(--accent);
  }
  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .title {
    color: var(--text);
    font-size: var(--text-md);
    font-weight: var(--weight-medium);
    line-height: var(--leading-tight);
  }
  .detail {
    margin-top: 2px;
    color: var(--text-tertiary);
    font-size: var(--text-xs);
    line-height: var(--leading-tight);
  }
</style>
