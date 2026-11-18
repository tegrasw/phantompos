<!--
  Cash drawer indicator in the printer state panel. Closed: a quiet row. On
  `drawer:kicked` the drawer slides open with a ring pulse (and pulses again on
  every further kick); it stays open until the user closes it here, which
  calls `set_printer_status({ drawerOpen: false })`. An open drawer never
  affects printing, so it uses the accent, not the fault colour.
-->
<script lang="ts">
  import type { DrawerKicked } from "../backend";
  import { i18n } from "../i18n";
  import Icon from "./Icon.svelte";

  let {
    open,
    kicks,
    lastKick,
    onClose,
  }: {
    open: boolean;
    /** Increments on every kick; replays the pulse. */
    kicks: number;
    lastKick: DrawerKicked | null;
    onClose: () => void;
  } = $props();

  const t = $derived(i18n.t.drawer);
  const detail = $derived(open && lastKick ? t.kicked(lastKick.pin, lastKick.onMs, lastKick.offMs) : t.closedHint);
  const detailTitle = $derived(
    open && lastKick ? t.kickedBy(lastKick.peer, lastKick.pin, lastKick.onMs, lastKick.offMs) : t.closedHint,
  );
</script>

<div class="drawer" class:open>
  {#key kicks}
    <span class="glyph" class:kick={kicks > 0 && open}>
      <Icon name={open ? "drawerOpen" : "drawer"} size={16} />
      {#if kicks > 0 && open}<span class="ring" aria-hidden="true"></span>{/if}
    </span>
    {#if kicks > 0 && open}<span class="halo" aria-hidden="true"></span>{/if}
  {/key}
  <span class="text">
    <span class="title">{t.title}</span>
    <span class="detail" title={detailTitle}>{detail}</span>
  </span>
  {#if open}
    <button type="button" class="close" aria-label={t.closeLabel} title={t.closeLabel} onclick={onClose}>
      {t.close}
    </button>
  {:else}
    <span class="state">{t.closed}</span>
  {/if}
</div>
<span class="visually-hidden" aria-live="polite">{open ? t.announceOpen : ""}</span>

<style>
  .drawer {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: 52px;
    padding: var(--space-2) var(--space-2) var(--space-2) var(--space-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-raised);
    box-shadow: var(--shadow-sm), var(--inset-highlight);
    transition:
      background-color var(--dur-base) var(--ease-out),
      border-color var(--dur-base) var(--ease-out),
      box-shadow var(--dur-base) var(--ease-out);
  }
  .drawer.open {
    border-color: var(--accent-border);
    background: color-mix(in srgb, var(--bg-raised), var(--accent) 7%);
    box-shadow: 0 0 0 3px var(--accent-soft), 0 0 16px -4px var(--accent-glow);
  }

  .glyph {
    position: relative;
    display: grid;
    place-items: center;
    flex: none;
    width: 30px;
    height: 30px;
    border-radius: var(--radius-sm);
    background: var(--bg-sunken);
    color: var(--text-tertiary);
    transition:
      background-color var(--dur-base) var(--ease-out),
      color var(--dur-base) var(--ease-out);
  }
  .open .glyph {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .title {
    color: var(--text);
    font-size: var(--text-md);
    font-weight: var(--weight-medium);
    line-height: var(--leading-tight);
  }
  .open .title {
    color: var(--accent);
    font-weight: var(--weight-semibold);
  }
  .detail {
    margin-top: 2px;
    color: var(--text-tertiary);
    font-size: var(--text-xs);
    line-height: var(--leading-tight);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .state {
    flex: none;
    padding: 0 var(--space-2);
    color: var(--text-tertiary);
    font-size: var(--text-xs);
    font-weight: var(--weight-medium);
  }
  .close {
    flex: none;
    height: 26px;
    padding: 0 var(--space-3);
    border: 1px solid var(--accent-border);
    border-radius: var(--radius-sm);
    background: var(--bg-raised);
    color: var(--accent);
    font-size: var(--text-sm);
    font-weight: var(--weight-semibold);
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-out);
  }
  .close:hover {
    background: var(--accent-soft);
  }
  .close:active {
    transform: translateY(0.5px);
  }

  /* The drawer front slides out, with a little overshoot. */
  .kick :global(.icon) {
    animation: slide-out 560ms var(--ease-out);
  }
  .ring {
    position: absolute;
    inset: 1px;
    border-radius: 50%;
    border: 1.5px solid var(--accent);
    animation: ring 900ms var(--ease-out) forwards;
    pointer-events: none;
  }
  .halo {
    position: absolute;
    inset: -1px;
    border-radius: inherit;
    pointer-events: none;
    animation: flash 1100ms var(--ease-out) forwards;
  }
  @keyframes slide-out {
    0% {
      transform: translateY(-3px) scale(0.9);
    }
    45% {
      transform: translateY(1.5px) scale(1.08);
    }
    70% {
      transform: translateY(-0.5px);
    }
    100% {
      transform: none;
    }
  }
  @keyframes ring {
    0% {
      transform: scale(0.6);
      opacity: 0.9;
    }
    100% {
      transform: scale(2);
      opacity: 0;
    }
  }
  @keyframes flash {
    0%,
    20% {
      box-shadow: 0 0 0 5px var(--accent-border), 0 0 24px 2px var(--accent-glow);
    }
    100% {
      box-shadow: 0 0 0 0 transparent;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .kick :global(.icon),
    .ring,
    .halo {
      animation: none;
      display: none;
    }
  }
</style>
