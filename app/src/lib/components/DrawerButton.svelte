<!--
  Cash drawer indicator. Closed: a quiet chip. On `drawer:kicked` it slides
  open with a ring pulse (and pulses again on every further kick); it stays
  open until the user clicks it, which closes the drawer.
-->
<script lang="ts">
  import type { DrawerKicked } from "../backend";
  import { messages } from "../messages";
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

  const t = messages.drawer;

  const title = $derived(
    open
      ? `${t.openTitle}${lastKick ? ` ${t.kickedBy(lastKick.peer, lastKick.pin, lastKick.onMs, lastKick.offMs)}` : ""}`
      : t.closedTitle,
  );

  function onclick(): void {
    if (open) onClose();
  }
</script>

<button
  type="button"
  class="drawer"
  class:open
  aria-label={open ? t.closeLabel : t.closedLabel}
  aria-disabled={!open}
  {title}
  {onclick}
>
  {#key kicks}
    <span class="glyph" class:kick={kicks > 0 && open}>
      <Icon name={open ? "drawerOpen" : "drawer"} size={15} />
      {#if kicks > 0 && open}<span class="ring" aria-hidden="true"></span>{/if}
    </span>
    {#if kicks > 0 && open}<span class="halo" aria-hidden="true"></span>{/if}
  {/key}
  <span class="text">{open ? t.open : t.closed}</span>
  {#if open}<span class="close" aria-hidden="true"><Icon name="x" size={11} /></span>{/if}
</button>
<span class="visually-hidden" aria-live="polite">{open ? t.announceOpen : ""}</span>

<style>
  .drawer {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: var(--space-1-5);
    height: var(--control-h);
    padding: 0 var(--space-2);
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--text-tertiary);
    font-size: var(--text-xs);
    font-weight: var(--weight-medium);
    white-space: nowrap;
    cursor: default;
    transition:
      background-color var(--dur-base) var(--ease-out),
      border-color var(--dur-base) var(--ease-out),
      color var(--dur-base) var(--ease-out),
      box-shadow var(--dur-base) var(--ease-out);
  }
  .drawer.open {
    border-color: var(--accent-border);
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: var(--weight-semibold);
    cursor: pointer;
    box-shadow: 0 0 0 3px var(--accent-soft), 0 0 14px -2px var(--accent-glow);
  }
  .drawer.open:hover {
    background: color-mix(in srgb, var(--accent-soft), var(--accent) 8%);
  }
  .drawer.open:active {
    transform: translateY(0.5px);
  }

  .glyph {
    position: relative;
    display: grid;
    place-items: center;
  }
  /* The drawer front slides out, with a little overshoot. */
  .kick :global(.icon) {
    animation: slide-out 560ms var(--ease-out);
  }
  .ring {
    position: absolute;
    inset: -3px;
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
      transform: scale(2.4);
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

  .close {
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    margin-right: -3px;
    border-radius: 50%;
    opacity: 0.75;
  }
  .drawer.open:hover .close {
    background: var(--accent-soft);
    opacity: 1;
  }

  @container roll (max-width: 520px) {
    .text {
      display: none;
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
