<!--
  The explicit way out of a listener error: "Use port N" (only when the
  backend suggested a free port), "Try again", and optionally a link to the
  settings. The app never changes the port by itself; these buttons are the
  only path. While a request runs, its button shows a spinner and both are
  disabled. If "Try again" comes back with the same error, a short note says
  so (otherwise nothing visible would change).
-->
<script lang="ts">
  import { onDestroy } from "svelte";
  import { fade } from "svelte/transition";
  import { i18n } from "../i18n";
  import { duration } from "../motion";
  import type { ListenerControls, ListenerError } from "../status";
  import Icon from "./Icon.svelte";

  let {
    status,
    controls,
    compact = false,
    settingsLink = false,
  }: {
    status: ListenerError;
    controls: ListenerControls;
    /** Header size: shorter labels, smaller buttons. */
    compact?: boolean;
    settingsLink?: boolean;
  } = $props();

  const t = $derived(i18n.t.listener);
  let pending = $state<"port" | "retry" | null>(null);
  let note = $state<string | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;

  async function run(kind: "port" | "retry", action: () => Promise<void>): Promise<void> {
    if (pending) return;
    pending = kind;
    note = null;
    try {
      await action();
      if (kind === "retry") {
        // The new `listener:status` usually lands before the command
        // resolves; give it one more frame.
        await new Promise((resolve) => requestAnimationFrame(resolve));
        if (status.kind === "error") flash(t.stillUnavailable);
      }
    } catch {
      // App reports the failure; the error state stays as it is.
    } finally {
      pending = null;
    }
  }

  function flash(text: string): void {
    note = text;
    clearTimeout(timer);
    timer = setTimeout(() => (note = null), 2800);
  }

  onDestroy(() => clearTimeout(timer));
</script>

<div class="actions" class:compact role="group" aria-label={i18n.t.header.error}>
  {#if status.suggestedPort !== null}
    {@const port = status.suggestedPort}
    <button
      type="button"
      class="btn primary"
      title={t.usePortTitle(port)}
      disabled={pending !== null}
      aria-busy={pending === "port"}
      onclick={() => run("port", () => controls.usePort(port))}
    >
      {#if pending === "port"}<span class="spinner" aria-hidden="true"></span>{/if}
      {compact ? t.usePortShort(port) : t.usePort(port)}
    </button>
  {/if}
  <button
    type="button"
    class="btn"
    title={t.retryTitle}
    aria-label={t.retry}
    disabled={pending !== null}
    aria-busy={pending === "retry"}
    onclick={() => run("retry", controls.retry)}
  >
    {#if pending === "retry"}<span class="spinner" aria-hidden="true"></span>{:else}<Icon name="refresh" size={13} />{/if}
    <span class="retry-label">{t.retry}</span>
  </button>
  {#if settingsLink}
    <button type="button" class="link-btn settings" onclick={controls.openSettings}>
      <Icon name="gear" size={13} />{t.openSettings}
    </button>
  {/if}
  <span class="visually-hidden" aria-live="polite">{note ?? ""}</span>
  {#if note}
    <span class="note" aria-hidden="true" transition:fade={{ duration: duration("fast") }}>{note}</span>
  {/if}
</div>

<style>
  .actions {
    position: relative;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }
  .settings {
    margin-left: var(--space-2);
    font-size: var(--text-sm);
  }
  .note {
    color: var(--text-secondary);
    font-size: var(--text-xs);
    font-weight: var(--weight-medium);
  }

  /* Header: compact buttons on one line; the note floats below. */
  .compact {
    flex-wrap: nowrap;
    gap: var(--space-1-5);
  }
  .compact .btn {
    height: 26px;
    padding: 0 var(--space-2-5, 10px);
    font-size: var(--text-xs);
  }
  .compact .note {
    position: absolute;
    top: calc(100% + 8px);
    right: 0;
    padding: var(--space-0-5) var(--space-2);
    border-radius: var(--radius-sm);
    background: var(--text);
    color: var(--bg-surface);
    white-space: nowrap;
    box-shadow: var(--shadow-md);
  }
  @media (max-width: 900px) {
    .compact .retry-label {
      display: none;
    }
    .compact .btn:not(.primary) {
      width: 26px;
      padding: 0;
      justify-content: center;
    }
  }
</style>
