<!--
  Paper width selector (58 / 80 mm) as a segmented control. Optimistic: the
  choice shows at once and rolls back if the backend refuses it. The new width
  applies to receipts started afterwards, which a short bubble confirms.
-->
<script lang="ts">
  import { onDestroy } from "svelte";
  import { fly } from "svelte/transition";
  import { PAPERS, type Paper } from "../backend";
  import { i18n } from "../i18n";
  import { duration, easeOut } from "../motion";
  import Icon from "./Icon.svelte";

  let {
    paper,
    disabled = false,
    onChange,
  }: {
    paper: Paper;
    disabled?: boolean;
    /** Resolves when the backend accepted the change; rejects to roll back. */
    onChange: (paper: Paper) => Promise<void>;
  } = $props();

  const t = $derived(i18n.t.paper);
  let note = $state<{ text: string; failed: boolean } | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;

  function show(text: string, failed: boolean): void {
    note = { text, failed };
    clearTimeout(timer);
    timer = setTimeout(() => (note = null), 2600);
  }

  async function choose(next: Paper): Promise<void> {
    if (next === paper || disabled) return;
    try {
      await onChange(next);
      show(t.changed(t.names[next]), false);
    } catch {
      show(t.failed, true);
    }
  }

  function onKeyDown(event: KeyboardEvent): void {
    const index = PAPERS.indexOf(paper);
    const step = event.key === "ArrowRight" || event.key === "ArrowDown" ? 1 : event.key === "ArrowLeft" || event.key === "ArrowUp" ? -1 : 0;
    if (!step) return;
    event.preventDefault();
    const next = PAPERS[(index + step + PAPERS.length) % PAPERS.length];
    void choose(next);
    const group = event.currentTarget as HTMLElement;
    queueMicrotask(() => group.querySelector<HTMLElement>(`[data-paper="${next}"]`)?.focus());
  }

  onDestroy(() => clearTimeout(timer));
</script>

<div class="paper-select">
  <span class="label" aria-hidden="true">{t.label}</span>
  <!-- Radio group with roving focus (arrow keys move and select). -->
  <div
    class="segmented"
    role="radiogroup"
    aria-label={t.groupLabel}
    title={t.hint}
    tabindex="-1"
    onkeydown={onKeyDown}
  >
    <span class="thumb" style:translate={paper === "80mm" ? "100% 0" : "0 0"} aria-hidden="true"></span>
    {#each PAPERS as option (option)}
      <button
        type="button"
        role="radio"
        data-paper={option}
        aria-checked={paper === option}
        tabindex={paper === option ? 0 : -1}
        {disabled}
        onclick={() => choose(option)}
      >
        {t.names[option]}
      </button>
    {/each}
  </div>
  <span class="visually-hidden" aria-live="polite">{note?.text ?? ""}</span>
  {#if note}
    <span
      class="bubble"
      class:failed={note.failed}
      aria-hidden="true"
      transition:fly={{ y: -4, duration: duration("fast"), easing: easeOut }}
    >
      {#if note.failed}<Icon name="alert" size={12} />{/if}
      {note.text}
    </span>
  {/if}
</div>

<style>
  .paper-select {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
  }
  .label {
    color: var(--text-tertiary);
    font-size: var(--text-xs);
    font-weight: var(--weight-medium);
  }
  @container roll (max-width: 600px) {
    .label {
      display: none;
    }
  }
  .segmented {
    position: relative;
    display: grid;
    grid-template-columns: 1fr 1fr;
    padding: 2px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-sunken);
  }
  .segmented:focus {
    outline: none;
  }
  .thumb {
    position: absolute;
    top: 2px;
    bottom: 2px;
    left: 2px;
    width: calc(50% - 2px);
    border-radius: var(--radius-sm);
    background: var(--bg-raised);
    box-shadow: var(--shadow-sm), var(--inset-highlight), 0 0 0 1px var(--border);
    transition: translate var(--dur-base) var(--ease-out);
  }
  button {
    position: relative;
    height: 22px;
    padding: 0 var(--space-2-5, 10px);
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-tertiary);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    font-weight: 700;
    white-space: nowrap;
    cursor: pointer;
    font-variant-numeric: tabular-nums;
    transition: color var(--dur-fast) var(--ease-out);
  }
  button:hover:not(:disabled) {
    color: var(--text);
  }
  button[aria-checked="true"] {
    color: var(--text);
  }
  button:disabled {
    cursor: default;
    opacity: 0.5;
  }
  button:focus-visible {
    outline-offset: 0;
  }

  .bubble {
    position: absolute;
    top: calc(100% + 8px);
    right: 0;
    z-index: 20;
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    padding: var(--space-0-5) var(--space-2);
    border-radius: var(--radius-sm);
    background: var(--text);
    color: var(--bg-surface);
    font-size: var(--text-xs);
    font-weight: var(--weight-medium);
    white-space: nowrap;
    pointer-events: none;
    box-shadow: var(--shadow-md);
  }
  .bubble.failed {
    background: var(--bg-raised);
    color: var(--text);
    border: 1px solid var(--border-strong);
  }
</style>
