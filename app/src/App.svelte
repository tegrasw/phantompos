<script lang="ts">
  import { onMount } from "svelte";
  import {
    connectBackend,
    formatAddress,
    runningInTauri,
    type ConnectionsStatus,
    type ListenerStatus,
  } from "./lib/backend";
  import { ByteLog } from "./lib/byteLog.svelte";
  import { copyText } from "./lib/clipboard";
  import { messages } from "./lib/messages";

  const t = messages;
  const inTauri = runningInTauri();

  let listener = $state<ListenerStatus | null>(null);
  let connections = $state<ConnectionsStatus>({ active: 0, peers: [] });
  let backendError = $state<string | null>(null);
  let copyFeedback = $state<string | null>(null);

  const log = new ByteLog();

  // Auto-scroll: follow new entries only while the user is at the bottom.
  let scroller: HTMLElement | undefined = $state();
  let followTail = $state(true);
  const STICKY_THRESHOLD_PX = 32;

  const address = $derived(
    listener ? formatAddress(listener.address, listener.port) : null,
  );
  // Suggest loopback in the empty-state hint even when bound to 0.0.0.0.
  const hintHost = $derived(
    !listener || listener.address === "0.0.0.0" || listener.address === "::"
      ? "127.0.0.1"
      : listener.address,
  );
  const hintPort = $derived(listener?.port ?? 9100);

  onMount(() => {
    if (!inTauri) return;
    const disconnect = connectBackend({
      onListenerStatus: (status) => (listener = status),
      onConnectionsChanged: (status) => (connections = status),
      onBytesReceived: (chunk) => log.push(chunk),
      onInitialState: (state) => {
        listener = state.listener;
        connections = state.connections;
        backendError = null;
      },
      onError: (error) => {
        backendError = `${t.header.stateUnavailable}: ${String(error)}`;
      },
    });
    return () => {
      disconnect();
      log.dispose();
    };
  });

  $effect(() => {
    // Re-run whenever the rendered entries change; effects run after the DOM
    // has been updated, so scrollHeight already includes the new rows.
    void log.entries;
    if (followTail && scroller) scroller.scrollTop = scroller.scrollHeight;
  });

  function onScroll(): void {
    if (!scroller) return;
    const distance =
      scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight;
    followTail = distance <= STICKY_THRESHOLD_PX;
  }

  function jumpToLatest(): void {
    followTail = true;
    if (scroller) scroller.scrollTop = scroller.scrollHeight;
  }

  function clearLog(): void {
    log.clear();
    followTail = true;
  }

  let copyTimer: ReturnType<typeof setTimeout> | undefined;
  async function copyAddress(): Promise<void> {
    if (!address) return;
    const ok = await copyText(address);
    copyFeedback = ok ? t.header.copied : t.header.copyFailed;
    clearTimeout(copyTimer);
    copyTimer = setTimeout(() => (copyFeedback = null), 1500);
  }

  function formatTime(date: Date): string {
    const hh = String(date.getHours()).padStart(2, "0");
    const mm = String(date.getMinutes()).padStart(2, "0");
    const ss = String(date.getSeconds()).padStart(2, "0");
    const ms = String(date.getMilliseconds()).padStart(3, "0");
    return `${hh}:${mm}:${ss}.${ms}`;
  }
</script>

<div class="app">
  <header class="header">
    <span class="brand">{t.appName}</span>

    {#if !inTauri}
      <span class="status status-error">
        <span class="dot" aria-hidden="true"></span>
        <span class="status-label">{t.header.notTauri}</span>
      </span>
      <span class="detail">{t.header.notTauriDetail}</span>
    {:else if backendError}
      <span class="status status-error">
        <span class="dot" aria-hidden="true"></span>
        <span class="status-label">{t.header.error}</span>
      </span>
      <span class="detail error-text" title={backendError}>{backendError}</span>
    {:else if !listener}
      <span class="status status-pending">
        <span class="dot" aria-hidden="true"></span>
        <span class="status-label">{t.header.starting}</span>
      </span>
    {:else if listener.state === "error"}
      <span class="status status-error">
        <span class="dot" aria-hidden="true"></span>
        <span class="status-label">{t.header.error}</span>
      </span>
      <span class="detail error-text" title={listener.message}>
        {listener.message ?? address}
      </span>
    {:else}
      <span
        class="status"
        class:status-ok={listener.state === "listening"}
        class:status-stopped={listener.state === "stopped"}
      >
        <span class="dot" aria-hidden="true"></span>
        <span class="status-label">
          {listener.state === "listening" ? t.header.listening : t.header.stopped}
        </span>
      </span>
      <code class="address">{address}</code>
      <button
        type="button"
        class="btn btn-small"
        title={t.header.copyTitle}
        onclick={copyAddress}
      >
        {copyFeedback ?? t.header.copy}
      </button>
    {/if}

    <span class="spacer"></span>

    {#if inTauri}
      <span class="connections" title={connections.peers.join("\n")}>
        {t.header.connections(connections.active)}
      </span>
    {/if}
  </header>

  <section class="log">
    <div class="log-toolbar">
      <h1 class="log-title">{t.log.title}</h1>
      <span class="spacer"></span>
      {#if !followTail && log.entries.length > 0}
        <button type="button" class="btn btn-small" onclick={jumpToLatest}>
          {t.log.jumpToLatest}
        </button>
      {/if}
      <button
        type="button"
        class="btn btn-small"
        onclick={clearLog}
        disabled={log.entries.length === 0}
      >
        {t.log.clear}
      </button>
    </div>

    <div class="log-scroll" bind:this={scroller} onscroll={onScroll}>
      {#if log.entries.length === 0}
        <div class="empty">
          <p class="empty-title">{t.log.emptyTitle}</p>
          <p>{t.log.emptyHint}</p>
          <code class="empty-command">{t.log.emptyCommand(hintHost, hintPort)}</code>
        </div>
      {:else}
        {#if log.dropped > 0}
          <p class="truncated">{t.log.truncated(log.dropped)}</p>
        {/if}
        {#each log.entries as entry (entry.key)}
          <article class="entry">
            <div class="entry-meta">
              <time>{formatTime(entry.receivedAt)}</time>
              <span class="peer">{entry.peer}</span>
              <span class="conn">{t.log.connectionLabel(entry.connectionId)}</span>
              <span class="count">{t.log.byteCount(entry.byteCount)}</span>
            </div>
            <pre class="dump">{entry.dump}</pre>
          </article>
        {/each}
      {/if}
    </div>
  </section>
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }

  .header {
    flex: none;
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.5rem 1rem;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
    min-width: 0;
  }

  .brand {
    font-weight: 600;
  }

  .status {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    white-space: nowrap;
  }

  .dot {
    width: 0.6rem;
    height: 0.6rem;
    border-radius: 50%;
    background: var(--muted);
  }

  .status-ok .dot {
    background: var(--ok);
  }

  .status-error .dot {
    background: var(--error);
  }

  .status-pending .dot {
    background: var(--warn);
  }

  .address {
    font-family: var(--mono);
  }

  .detail {
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .error-text {
    color: var(--error);
  }

  .connections {
    color: var(--muted);
    white-space: nowrap;
  }

  .spacer {
    flex: 1;
  }

  .log {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .log-toolbar {
    flex: none;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.4rem 1rem;
    border-bottom: 1px solid var(--border);
  }

  .log-title {
    margin: 0;
    font-size: 0.85rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
  }

  .log-scroll {
    flex: 1;
    overflow: auto;
    padding: 0.5rem 1rem 1rem;
  }

  .empty {
    max-width: 36rem;
    margin: 3rem auto;
    text-align: center;
    color: var(--muted);
  }

  .empty-title {
    font-size: 1.1rem;
    color: var(--text);
  }

  .empty-command {
    display: inline-block;
    margin-top: 0.25rem;
    padding: 0.4rem 0.75rem;
    border-radius: 6px;
    background: var(--surface);
    border: 1px solid var(--border);
    font-family: var(--mono);
    color: var(--text);
    user-select: all;
  }

  .truncated {
    margin: 0 0 0.5rem;
    font-size: 0.8rem;
    color: var(--muted);
  }

  .entry {
    padding: 0.4rem 0;
    border-bottom: 1px solid var(--border);
  }

  .entry-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem;
    font-size: 0.8rem;
    color: var(--muted);
    font-family: var(--mono);
  }

  .entry-meta .count {
    color: var(--text);
  }

  .dump {
    margin: 0.25rem 0 0;
    font-family: var(--mono);
    font-size: 0.8rem;
    line-height: 1.35;
    white-space: pre;
    overflow-x: auto;
  }

  .btn {
    font: inherit;
    color: var(--text);
    background: var(--button);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.25rem 0.75rem;
    cursor: pointer;
  }

  .btn:hover:not(:disabled) {
    background: var(--button-hover);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .btn-small {
    font-size: 0.8rem;
    padding: 0.15rem 0.6rem;
  }
</style>
