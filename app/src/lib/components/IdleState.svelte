<!--
  Empty roll: an honest idle state showing where to send prints, how to send a
  test, and what this session has received so far.
-->
<script lang="ts">
  import { fade } from "svelte/transition";
  import type { SessionStats } from "../commandLog.svelte";
  import { formatBytes, formatClock, formatCount } from "../format";
  import { i18n } from "../i18n";
  import { duration } from "../motion";
  import { loopbackAddress, type EndpointStatus, type ListenerControls } from "../status";
  import CopyButton from "./CopyButton.svelte";
  import Icon from "./Icon.svelte";
  import ListenerActions from "./ListenerActions.svelte";

  let {
    status,
    stats,
    controls,
    fault = null,
  }: {
    status: EndpointStatus;
    stats: SessionStats;
    controls: ListenerControls;
    /** Names of the simulated error states, when any is on. */
    fault?: string | null;
  } = $props();

  const t = $derived(i18n.t.roll);
  const l = $derived(i18n.t.listener);

  const tone = $derived(
    status.kind === "listening"
      ? "ok"
      : status.kind === "error" || status.kind === "backendError"
        ? "error"
        : "idle",
  );

  const title = $derived.by(() => {
    switch (status.kind) {
      case "listening":
        return stats.commands > 0 ? t.readyAgainTitle : t.readyTitle;
      case "stopped":
        return t.stoppedTitle;
      case "error":
        return l.errorTitle[status.code](status.port);
      case "backendError":
        return i18n.t.header.stateUnavailable;
      case "starting":
        return t.startingTitle;
      case "disconnected":
        return t.disconnectedTitle;
    }
  });

  const body = $derived.by(() => {
    switch (status.kind) {
      case "listening":
        return status.addresses.length > 1 ? t.readyBodyNetwork : t.readyBody;
      case "stopped":
        return t.stoppedBody;
      case "error":
        return l.errorBody[status.code];
      case "backendError":
        return status.message;
      case "starting":
        return t.startingBody;
      case "disconnected":
        return t.disconnectedBody;
    }
  });

  const reach = $derived(status.kind === "listening" || status.kind === "stopped" ? status : null);
</script>

<div class="empty" data-tone={tone}>
  {#key status.kind}
    <div class="idle" in:fade={{ duration: duration("slow") }}>
      <svg class="art" class:fault={!!fault && status.kind === "listening"} viewBox={tone === "error" ? "0 0 168 48" : "0 0 168 116"} aria-hidden="true">
        <!-- Paper slip feeding out of the printer. -->
        {#if tone !== "error"}
          <g class="slip">
            <path
              class="paper"
              d="M42 34h84v68l-7 -4.5-7 4.5-7-4.5-7 4.5-7-4.5-7 4.5-7-4.5-7 4.5-7-4.5-7 4.5-7-4.5z"
            />
            <rect class="ink" x="66" y="46" width="36" height="4" rx="2" />
            <rect class="ink faint" x="54" y="58" width="38" height="3" rx="1.5" />
            <rect class="ink faint" x="100" y="58" width="14" height="3" rx="1.5" />
            <rect class="ink faint" x="54" y="67" width="30" height="3" rx="1.5" />
            <rect class="ink faint" x="100" y="67" width="14" height="3" rx="1.5" />
            <rect class="ink faint" x="54" y="76" width="34" height="3" rx="1.5" />
            <rect class="ink faint" x="100" y="76" width="14" height="3" rx="1.5" />
            <rect class="ink" x="54" y="86" width="60" height="3" rx="1.5" />
          </g>
        {/if}
        <!-- Printer body. -->
        <rect class="body" x="16" y="6" width="136" height="34" rx="9" />
        <rect class="slot" x="34" y="30" width="100" height="4" rx="2" />
        <circle class="led" cx="136" cy="18" r="3.5" />
        <rect class="vent" x="30" y="15" width="22" height="2.5" rx="1.25" />
      </svg>

      <h1 class="title">{title}</h1>
      <p class="body-text">{body}</p>

      {#if fault && status.kind === "listening"}
        <p class="fault" transition:fade={{ duration: duration("base") }}>
          <span class="fault-icon" aria-hidden="true"><Icon name="pause" size={10} /></span>
          {t.idleFault(fault)}
        </p>
      {/if}

      {#if reach}
        <div class="card">
          <div class="field">
            <span class="field-label">
              {reach.addresses.length > 1 ? t.addressesLabel : t.addressLabel}
              {#if reach.network}
                <button type="button" class="network" onclick={controls.openSettings} title={i18n.t.header.networkTitle}>
                  <Icon name="globe" size={11} />{t.networkBadge}
                </button>
              {/if}
            </span>
            {#each reach.addresses as address (address)}
              <div class="field-row" class:list={reach.addresses.length > 1}>
                <code class="address">{address}</code>
                <CopyButton text={address} label={i18n.t.header.copyAddressOf(address)} bubble="above" />
              </div>
            {:else}
              <p class="no-address">
                <span class="no-address-value">{t.noAddressValue}</span>
                <span class="no-address-hint">{t.noAddressHint(loopbackAddress(reach.port))}</span>
              </p>
            {/each}
          </div>
          {#if status.kind === "listening"}
            {@const command = t.testCommand(reach.host, reach.port)}
            <div class="field">
              <span class="field-label"><Icon name="terminal" size={13} />{t.testLabel}</span>
              <div class="field-row command">
                <code class="cmd">{command}</code>
                <CopyButton text={command} label={t.copyCommand} bubble="above" />
              </div>
            </div>
          {/if}
        </div>
      {:else if status.kind === "error"}
        <div class="callout">
          <ListenerActions {status} {controls} settingsLink />
          {#if status.code === "other" && status.message}
            <p class="details"><span class="caps-label">{l.details}</span> <code>{status.message}</code></p>
          {/if}
        </div>
      {/if}

      {#if stats.commands > 0}
        <section class="session" aria-label={t.sessionTitle} in:fade={{ duration: duration("base") }}>
          <h2 class="session-title">{t.sessionTitle}</h2>
          <dl class="stats">
            <div class="stat">
              <dt>{t.statCommands}</dt>
              <dd>{formatCount(stats.commands)}</dd>
            </div>
            <div class="stat">
              <dt>{t.statBytes}</dt>
              <dd>{formatBytes(stats.bytes)}</dd>
            </div>
            <div class="stat" class:warn={stats.unknown > 0}>
              <dt>{t.statUnknown}</dt>
              <dd>{formatCount(stats.unknown)}</dd>
            </div>
            {#if stats.replies > 0}
              <div class="stat">
                <dt>{t.statReplies}</dt>
                <dd>{formatCount(stats.replies)}</dd>
              </div>
            {/if}
            {#if stats.lastAt}
              <div class="stat" title={stats.lastPeer ? t.lastFrom(stats.lastPeer) : undefined}>
                <dt>{t.statLast}</dt>
                <dd>{formatClock(stats.lastAt)}</dd>
              </div>
            {/if}
          </dl>
        </section>
      {/if}
    </div>
  {/key}
</div>

<style>
  .empty {
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow-y: auto;
    display: flex;
  }

  .idle {
    width: 100%;
    max-width: 548px;
    margin: auto;
    padding: var(--space-10) var(--space-6);
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
  }

  /* Illustration --------------------------------------------------------- */
  .art {
    width: 156px;
    height: auto;
    margin-bottom: var(--space-5);
    overflow: visible;
  }
  .body {
    fill: var(--bg-raised);
    stroke: var(--border-strong);
    stroke-width: 1;
    filter: drop-shadow(0 6px 10px rgb(0 0 0 / 0.08));
  }
  .slot {
    fill: var(--text);
    opacity: 0.75;
  }
  .vent {
    fill: var(--border-strong);
  }
  .led {
    fill: var(--text-tertiary);
    transition: fill var(--dur-base) var(--ease-out);
  }
  [data-tone="ok"] .led {
    fill: var(--accent);
    filter: drop-shadow(0 0 4px var(--accent-glow));
  }
  [data-tone="error"] .led {
    fill: var(--danger);
  }
  .art.fault .led {
    fill: var(--fault);
    filter: drop-shadow(0 0 4px var(--fault-glow));
  }
  .paper {
    fill: var(--paper);
    stroke: var(--paper-edge);
    stroke-width: 1;
  }
  .ink {
    fill: var(--paper-ink);
    opacity: 0.8;
  }
  .ink.faint {
    opacity: 0.32;
  }
  .slip {
    animation: feed 900ms var(--ease-out) 120ms both;
  }
  @media (prefers-reduced-motion: reduce) {
    .slip {
      animation: none;
    }
  }
  [data-tone="idle"] .slip {
    opacity: 0.55;
  }
  @keyframes feed {
    from {
      transform: translateY(-14px);
      clip-path: inset(0 0 100% 0);
    }
    to {
      transform: translateY(0);
      clip-path: inset(0 0 0 0);
    }
  }

  /* Copy ------------------------------------------------------------------ */
  .title {
    margin: 0 0 var(--space-2);
    font-size: var(--text-xl);
    font-weight: var(--weight-semibold);
    letter-spacing: -0.015em;
    line-height: var(--leading-tight);
    text-wrap: balance;
  }
  [data-tone="error"] .title {
    color: var(--danger);
  }
  .body-text {
    margin: 0 0 var(--space-6);
    max-width: 46ch;
    color: var(--text-secondary);
    font-size: var(--text-md);
    line-height: var(--leading);
    text-wrap: pretty;
  }

  /* Card ------------------------------------------------------------------ */
  .card {
    width: 100%;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-surface);
    box-shadow: var(--shadow-md);
    text-align: left;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-1-5);
    padding: var(--space-3) var(--space-3) var(--space-3) var(--space-4);
    min-width: 0;
  }
  .field + .field {
    border-top: 1px solid var(--border-subtle);
  }
  .field-label {
    display: flex;
    align-items: center;
    gap: var(--space-1-5);
    color: var(--text-tertiary);
    font-size: var(--text-2xs);
    font-weight: var(--weight-semibold);
    letter-spacing: var(--tracking-caps);
    text-transform: uppercase;
  }
  .field-row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }
  .address {
    flex: 1;
    font-family: var(--font-mono);
    font-size: var(--text-lg);
    font-weight: 700;
    letter-spacing: -0.01em;
    color: var(--text);
  }
  [data-tone="idle"] .address {
    color: var(--text-tertiary);
  }
  .command {
    padding: var(--space-1-5) var(--space-1) var(--space-1-5) var(--space-3);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    background: var(--bg-sunken);
  }
  .cmd {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    color: var(--text-secondary);
    user-select: all;
  }

  .fault {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: calc(-1 * var(--space-2)) 0 var(--space-5);
    padding: var(--space-1-5) var(--space-3) var(--space-1-5) var(--space-2);
    border: 1px dashed var(--fault-border);
    border-radius: var(--radius-pill);
    background: var(--fault-soft);
    color: var(--fault);
    font-size: var(--text-sm);
    font-weight: var(--weight-medium);
    text-align: left;
  }
  .fault-icon {
    display: grid;
    place-items: center;
    flex: none;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--fault);
    color: var(--bg-surface);
  }
  .fault-icon :global(.icon) {
    stroke-width: 2.25;
  }

  /* Listener error: the way out, centred under the explanation. */
  .callout {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-3);
    width: 100%;
  }
  .callout :global(.actions) {
    justify-content: center;
  }
  .details {
    max-width: 100%;
    margin: 0;
    color: var(--text-tertiary);
    font-size: var(--text-xs);
    overflow-wrap: anywhere;
  }
  .details code {
    font-family: var(--font-mono);
  }

  /* Network mode: a neutral outlined badge beside the label. */
  .network {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    margin-left: auto;
    height: 18px;
    padding: 0 var(--space-1-5);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-pill);
    background: var(--bg-raised);
    color: var(--text);
    font-size: var(--text-2xs);
    font-weight: var(--weight-semibold);
    letter-spacing: 0.01em;
    text-transform: none;
    white-space: nowrap;
    flex: none;
    cursor: pointer;
  }
  .network:hover {
    border-color: var(--text-tertiary);
  }
  .field-row.list .address {
    font-size: var(--text-md);
  }
  .no-address {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
  }
  .no-address-value {
    color: var(--text);
    font-size: var(--text-md);
    font-weight: var(--weight-semibold);
  }
  .no-address-hint {
    color: var(--text-secondary);
    font-size: var(--text-sm);
    line-height: var(--leading);
  }

  /* Session stats --------------------------------------------------------- */
  .session {
    width: 100%;
    margin-top: var(--space-6);
    text-align: left;
  }
  .session-title {
    margin: 0 0 var(--space-2);
    color: var(--text-tertiary);
    font-size: var(--text-2xs);
    font-weight: var(--weight-semibold);
    letter-spacing: var(--tracking-caps);
    text-transform: uppercase;
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(96px, 1fr));
    gap: 1px;
    margin: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--border);
    overflow: hidden;
  }
  .stat {
    padding: var(--space-2) var(--space-3);
    background: var(--bg-surface);
  }
  .stat dt {
    color: var(--text-tertiary);
    font-size: var(--text-xs);
  }
  .stat dd {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-md);
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
  .stat.warn dd {
    color: var(--warning);
  }

  @media (max-height: 560px) {
    .idle {
      padding-top: var(--space-6);
      padding-bottom: var(--space-6);
    }
    .art {
      width: 120px;
      margin-bottom: var(--space-3);
    }
    .body-text {
      margin-bottom: var(--space-4);
    }
  }
</style>
