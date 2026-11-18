<!--
  Renders `toasts` (toast.svelte.ts): small cards stacked at the bottom right
  of the window, above everything but modal dialogs. Success uses the accent
  (a check); failures are neutral with an alert glyph, since red is reserved
  for listener errors. Each card slides in, closes itself after its timeout
  (paused while hovered or focused) and can be dismissed. A polite live
  region announces it.
-->
<script lang="ts">
  import { onDestroy } from "svelte";
  import { flip } from "svelte/animate";
  import { fly } from "svelte/transition";
  import { i18n } from "../i18n";
  import { duration, easeOut } from "../motion";
  import { toasts, type Toast } from "../toast.svelte";
  import Icon from "./Icon.svelte";

  const t = $derived(i18n.t.toast);

  /** Remaining time per toast, so hovering pauses instead of restarting. */
  const timers = new Map<number, { handle: ReturnType<typeof setTimeout> | null; left: number; since: number }>();

  function start(toast: Toast): void {
    if (toast.timeout <= 0 || timers.has(toast.id)) return;
    const entry = { handle: null as ReturnType<typeof setTimeout> | null, left: toast.timeout, since: 0 };
    timers.set(toast.id, entry);
    resume(toast.id);
  }
  function pause(id: number): void {
    const entry = timers.get(id);
    if (!entry?.handle) return;
    clearTimeout(entry.handle);
    entry.handle = null;
    entry.left -= performance.now() - entry.since;
  }
  function resume(id: number): void {
    const entry = timers.get(id);
    if (!entry || entry.handle) return;
    entry.since = performance.now();
    entry.handle = setTimeout(() => dismiss(id), Math.max(800, entry.left));
  }
  function dismiss(id: number): void {
    const entry = timers.get(id);
    if (entry?.handle) clearTimeout(entry.handle);
    timers.delete(id);
    toasts.dismiss(id);
  }

  $effect(() => {
    for (const toast of toasts.items) start(toast);
    const live = new Set(toasts.items.map((toast) => toast.id));
    for (const [id, entry] of timers) {
      if (live.has(id)) continue;
      if (entry.handle) clearTimeout(entry.handle);
      timers.delete(id);
    }
  });

  onDestroy(() => {
    for (const entry of timers.values()) if (entry.handle) clearTimeout(entry.handle);
  });
</script>

<section class="toaster" aria-label={t.region}>
  {#each toasts.items as toast (toast.id)}
    <div
      class="toast"
      data-tone={toast.tone}
      role={toast.tone === "neutral" ? "alert" : "status"}
      onpointerenter={() => pause(toast.id)}
      onpointerleave={() => resume(toast.id)}
      onfocusin={() => pause(toast.id)}
      onfocusout={() => resume(toast.id)}
      in:fly={{ y: 12, duration: duration("slow"), easing: easeOut }}
      out:fly={{ x: 24, duration: duration("base"), easing: easeOut }}
      animate:flip={{ duration: duration("base"), easing: easeOut }}
    >
      <span class="glyph" aria-hidden="true">
        <Icon name={toast.tone === "success" ? "check" : "alert"} size={13} />
      </span>
      <span class="text">
        <span class="title">{toast.title}</span>
        {#if toast.detail}
          <span class="detail" title={toast.detailTitle ?? toast.detail}>{toast.detail}</span>
        {/if}
      </span>
      <button type="button" class="close" aria-label={t.dismiss} title={t.dismiss} onclick={() => dismiss(toast.id)}>
        <Icon name="x" size={12} />
      </button>
    </div>
  {/each}
</section>

<style>
  .toaster {
    position: fixed;
    right: var(--space-4);
    bottom: var(--space-4);
    z-index: 40;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: var(--space-2);
    width: min(380px, calc(100vw - 2 * var(--space-4)));
    pointer-events: none;
  }
  .toast {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2-5, 10px);
    width: 100%;
    padding: var(--space-2-5, 10px) var(--space-2) var(--space-2-5, 10px) var(--space-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    box-shadow: var(--shadow-pop), var(--inset-highlight);
    pointer-events: auto;
  }
  .glyph {
    display: grid;
    place-items: center;
    flex: none;
    width: 22px;
    height: 22px;
    margin-top: -1px;
    border-radius: 50%;
    background: var(--bg-active);
    color: var(--text-secondary);
  }
  [data-tone="success"] .glyph {
    background: var(--accent);
    color: var(--accent-fg);
  }
  [data-tone="success"] .glyph :global(.icon) {
    stroke-width: 2.25;
  }
  .text {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    padding-top: 1px;
  }
  .title {
    color: var(--text);
    font-size: var(--text-md);
    font-weight: var(--weight-semibold);
    line-height: var(--leading-tight);
    overflow-wrap: anywhere;
  }
  .detail {
    margin-top: 3px;
    color: var(--text-secondary);
    font-size: var(--text-xs);
    line-height: var(--leading-tight);
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow-wrap: anywhere;
  }
  .close {
    display: grid;
    place-items: center;
    flex: none;
    width: 22px;
    height: 22px;
    padding: 0;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-tertiary);
    cursor: pointer;
    transition:
      background-color var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }
  .close:hover {
    background: var(--bg-hover);
    color: var(--text);
  }
</style>
