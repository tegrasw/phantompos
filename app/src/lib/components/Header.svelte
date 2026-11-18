<!--
  Fixed application header: mark, endpoint status (state + address + copy, or
  the error in place of the address), live connection count with an activity
  light, and the panel toggles (printer state on the left, command log on the
  right). While an error state is simulated, a --fault chip beside the
  endpoint names it (and opens the printer state); it never uses the listener
  error colours, since the listener is fine.
-->
<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import { activeFaults, type ConnectionsStatus, type PrinterStatus } from "../backend";
  import { messages } from "../messages";
  import { duration, easeOut } from "../motion";
  import type { EndpointStatus } from "../status";
  import AppMark from "./AppMark.svelte";
  import CopyButton from "./CopyButton.svelte";
  import Icon from "./Icon.svelte";

  let {
    status,
    printer,
    connections,
    activity,
    logOpen,
    stateOpen,
    onToggleLog,
    onToggleState,
    onShowState,
  }: {
    status: EndpointStatus;
    printer: PrinterStatus;
    connections: ConnectionsStatus;
    /** Increments whenever data arrives; drives the activity light. */
    activity: number;
    logOpen: boolean;
    stateOpen: boolean;
    onToggleLog: () => void;
    onToggleState: () => void;
    onShowState: () => void;
  } = $props();

  const t = messages.header;
  const p = messages.printer;

  const faults = $derived(activeFaults(printer));

  const label = $derived.by(() => {
    switch (status.kind) {
      case "listening":
        return t.listening;
      case "stopped":
        return t.stopped;
      case "error":
      case "backendError":
        return t.error;
      case "starting":
        return t.starting;
      case "disconnected":
        return t.disconnected;
    }
  });

  const tone = $derived(
    status.kind === "listening"
      ? "ok"
      : status.kind === "error" || status.kind === "backendError"
        ? "error"
        : "idle",
  );

  const detail = $derived.by((): { text: string; mono: boolean } | null => {
    switch (status.kind) {
      case "listening":
      case "stopped":
        return { text: status.address, mono: true };
      case "error":
      case "backendError":
        return { text: status.message, mono: false };
      case "disconnected":
        return { text: t.disconnectedDetail, mono: false };
      case "starting":
        return null;
    }
  });

  const copyable = $derived(status.kind === "listening" || status.kind === "stopped");
  const hasBackend = $derived(status.kind !== "disconnected");
</script>

<header class="header">
  <div class="brand">
    <AppMark size={22} />
    <span class="name">{messages.appName}</span>
  </div>

  <div class="endpoint" class:no-copy={!copyable} data-tone={tone} role="status">
    <span class="indicator" aria-hidden="true">
      <span class="dot"></span>
      {#if status.kind === "listening"}<span class="pulse"></span>{/if}
    </span>
    {#key label}
      <span class="label" in:fly={{ y: 4, duration: duration("base"), easing: easeOut }}>
        {label}
      </span>
    {/key}
    {#if detail}
      <span class="divider" aria-hidden="true"></span>
      {#key detail.text}
        <span
          class="detail"
          class:mono={detail.mono}
          class:muted={status.kind === "stopped"}
          title={detail.text}
          in:fade={{ duration: duration("base") }}
        >
          {#if tone === "error"}<Icon name="alert" size={13} />{/if}
          <span class="detail-text">{detail.text}</span>
        </span>
      {/key}
    {/if}
    {#if status.kind === "listening" || status.kind === "stopped"}
      <CopyButton text={status.address} label={t.copyAddress} />
    {/if}
  </div>

  {#if hasBackend && faults.length > 0}
    {@const names = p.list(faults.map((f) => p.faults[f]))}
    <button
      type="button"
      class="fault"
      title={t.faultTitle(names)}
      aria-label={t.faultTitle(names)}
      aria-controls="printer-state"
      onclick={onShowState}
      transition:fly={{ x: -6, duration: duration("base"), easing: easeOut }}
    >
      <span class="fault-icon" aria-hidden="true"><Icon name="pause" size={10} /></span>
      {#key faults.join()}
        <span class="fault-text" in:fade={{ duration: duration("fast") }}>
          {t.faultChip(p.faults[faults[0]], faults.length - 1)}
        </span>
      {/key}
    </button>
  {/if}

  <span class="spacer"></span>

  {#if hasBackend}
    <div
      class="connections"
      class:active={connections.active > 0}
      title={t.connectedPeers(connections.peers)}
    >
      {#key activity}
        <span class="led" class:flash={activity > 0} aria-hidden="true"></span>
      {/key}
      <Icon name="plug" size={14} />
      {#key connections.active}
        <span class="count" in:fly={{ y: 4, duration: duration("fast"), easing: easeOut }}>
          {connections.active === 0 ? t.noConnections : t.connections(connections.active)}
        </span>
      {/key}
    </div>
    <span class="rule" aria-hidden="true"></span>
  {/if}

  {#if hasBackend}
    <button
      type="button"
      class="icon-btn"
      aria-pressed={stateOpen}
      aria-controls="printer-state"
      title={stateOpen ? t.hideState : t.showState}
      aria-label={stateOpen ? t.hideState : t.showState}
      onclick={onToggleState}
    >
      <Icon name="panelLeft" />
    </button>
  {/if}
  <button
    type="button"
    class="icon-btn"
    aria-pressed={logOpen}
    aria-controls="command-log"
    title={logOpen ? t.hideLog : t.showLog}
    aria-label={logOpen ? t.hideLog : t.showLog}
    onclick={onToggleLog}
  >
    <Icon name="panel" />
  </button>
</header>

<style>
  .header {
    flex: none;
    position: relative;
    z-index: 10;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    height: var(--header-h);
    padding: 0 var(--space-3) 0 var(--space-4);
    border-bottom: 1px solid var(--border);
    background: var(--bg-surface);
    min-width: 0;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    flex: none;
    padding-right: var(--space-1);
  }

  .name {
    font-size: var(--text-md);
    font-weight: var(--weight-semibold);
    letter-spacing: -0.01em;
  }

  /* Endpoint pill ------------------------------------------------------- */
  .endpoint {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
    height: 30px;
    padding: 0 var(--space-1) 0 var(--space-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-sunken);
    transition:
      background-color var(--dur-base) var(--ease-out),
      border-color var(--dur-base) var(--ease-out);
  }
  .endpoint.no-copy {
    padding-right: var(--space-3);
  }
  .endpoint[data-tone="error"] {
    background: var(--danger-soft);
    border-color: var(--danger-border);
  }

  .indicator {
    position: relative;
    display: grid;
    place-items: center;
    width: 8px;
    height: 8px;
    flex: none;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-tertiary);
    transition: background-color var(--dur-base) var(--ease-out);
  }
  [data-tone="ok"] .dot {
    background: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  [data-tone="error"] .dot {
    background: var(--danger);
    box-shadow: 0 0 0 3px var(--danger-soft);
  }
  .pulse {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    border: 1.5px solid var(--accent);
    animation: pulse 2.6s var(--ease-out) infinite;
  }
  @keyframes pulse {
    0% {
      transform: scale(1);
      opacity: 0.7;
    }
    70%,
    100% {
      transform: scale(2.6);
      opacity: 0;
    }
  }

  .label {
    font-size: var(--text-sm);
    font-weight: var(--weight-semibold);
    white-space: nowrap;
  }
  [data-tone="ok"] .label {
    color: var(--accent);
  }
  [data-tone="error"] .label {
    color: var(--danger);
  }
  [data-tone="idle"] .label {
    color: var(--text-secondary);
  }

  .divider {
    width: 1px;
    height: 14px;
    background: var(--border-strong);
    flex: none;
  }
  [data-tone="error"] .divider {
    background: var(--danger-border);
  }

  .detail {
    display: flex;
    align-items: center;
    gap: var(--space-1-5);
    min-width: 0;
    font-size: var(--text-sm);
    color: var(--text);
  }
  .detail-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .detail.mono {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    letter-spacing: -0.01em;
  }
  .detail.muted {
    color: var(--text-tertiary);
  }
  [data-tone="error"] .detail {
    color: var(--danger);
    font-weight: var(--weight-medium);
  }
  [data-tone="idle"] .detail:not(.mono) {
    color: var(--text-secondary);
  }

  .spacer {
    flex: 1;
    min-width: var(--space-2);
  }

  /* Simulated fault chip -------------------------------------------------- */
  .fault {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1-5);
    flex: none;
    height: 26px;
    padding: 0 var(--space-3) 0 var(--space-1);
    border: 1px solid var(--fault-border);
    border-radius: var(--radius-pill);
    background: var(--fault-soft);
    color: var(--fault);
    font-size: var(--text-sm);
    font-weight: var(--weight-semibold);
    white-space: nowrap;
    cursor: pointer;
    transition:
      background-color var(--dur-fast) var(--ease-out),
      box-shadow var(--dur-fast) var(--ease-out);
  }
  .fault:hover {
    background: color-mix(in srgb, var(--fault-soft), var(--fault) 8%);
    box-shadow: 0 0 12px -4px var(--fault-glow);
  }
  .fault-icon {
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--fault);
    color: var(--bg-surface);
  }
  .fault-icon :global(.icon) {
    stroke-width: 2.25;
  }

  /* Connections --------------------------------------------------------- */
  .connections {
    display: flex;
    align-items: center;
    gap: var(--space-1-5);
    flex: none;
    font-size: var(--text-sm);
    color: var(--text-tertiary);
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
  .connections.active {
    color: var(--text);
  }
  .connections.active :global(.icon) {
    color: var(--accent);
  }
  .count {
    display: inline-block;
  }

  .led {
    width: 6px;
    height: 6px;
    margin-right: var(--space-0-5);
    border-radius: 50%;
    background: var(--border-strong);
  }
  /* Colour-only flash: allowed under reduced motion as nothing moves. */
  .led.flash {
    animation: led 900ms ease-out;
  }
  @keyframes led {
    0%,
    15% {
      background: var(--accent);
      box-shadow: 0 0 0 3px var(--accent-soft), 0 0 8px var(--accent-glow);
    }
    100% {
      background: var(--border-strong);
      box-shadow: 0 0 0 0 transparent;
    }
  }

  .rule {
    width: 1px;
    height: 20px;
    background: var(--border);
    flex: none;
  }

  /* Narrow windows: the chip keeps its icon and the first fault only. */
  @media (max-width: 760px) {
    .connections .count {
      display: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .pulse {
      display: none;
    }
  }
</style>
