<script lang="ts">
  import { onMount } from "svelte";
  import {
    connectBackend,
    runningInTauri,
    type BackendConnector,
    type BackendHandlers,
    type ConnectionsStatus,
    type ListenerStatus,
  } from "./lib/backend";
  import { CommandLog } from "./lib/commandLog.svelte";
  import CommandLogPanel from "./lib/components/CommandLog.svelte";
  import CommandLogRail from "./lib/components/CommandLogRail.svelte";
  import Header from "./lib/components/Header.svelte";
  import RollView from "./lib/components/RollView.svelte";
  import SidePanel from "./lib/components/SidePanel.svelte";
  import { messages } from "./lib/messages";
  import { printerStatus } from "./lib/status";

  const t = messages;

  let hasBackend = $state(runningInTauri());
  let listener = $state<ListenerStatus | null>(null);
  let connections = $state<ConnectionsStatus>({ active: 0, peers: [] });
  let backendError = $state<string | null>(null);
  /** Bumped (at most every ACTIVITY_MS) when commands arrive. */
  let activity = $state(0);

  const log = new CommandLog();
  const status = $derived(printerStatus(hasBackend, listener, backendError));

  // Layout ------------------------------------------------------------------
  const PANEL_MIN = 380;
  const ROLL_MIN = 260;
  const STORE_OPEN = "phantompos.commandLog.open";
  const STORE_WIDTH = "phantompos.commandLog.width";

  let viewportWidth = $state(window.innerWidth);
  let logOpen = $state(readStored(STORE_OPEN) !== "false");
  let logWidth = $state(
    Number(readStored(STORE_WIDTH)) ||
      Math.round(Math.min(680, Math.max(460, window.innerWidth * 0.5))),
  );
  const panelMax = $derived(Math.max(PANEL_MIN, viewportWidth - ROLL_MIN));
  /** Below this width the log stacks bytes under each command. */
  const COMPACT_BELOW = 520;
  const logCompact = $derived(Math.min(logWidth, panelMax) < COMPACT_BELOW);

  $effect(() => writeStored(STORE_OPEN, String(logOpen)));
  $effect(() => writeStored(STORE_WIDTH, String(logWidth)));

  function readStored(key: string): string | null {
    try {
      return localStorage.getItem(key);
    } catch {
      return null;
    }
  }
  function writeStored(key: string, value: string): void {
    try {
      localStorage.setItem(key, value);
    } catch {
      // Storage unavailable: the layout simply isn't remembered.
    }
  }

  // Backend -----------------------------------------------------------------
  const ACTIVITY_MS = 150;
  let lastActivity = 0;

  const handlers: BackendHandlers = {
    onListenerStatus: (next) => (listener = next),
    onConnectionsChanged: (next) => (connections = next),
    onCommandsReceived: (batch) => {
      log.push(batch);
      const now = performance.now();
      if (now - lastActivity >= ACTIVITY_MS) {
        lastActivity = now;
        activity++;
      }
    },
    onInitialState: (state) => {
      listener = state.listener;
      connections = state.connections;
      backendError = null;
    },
    onError: (error) => {
      backendError = `${t.header.stateUnavailable}: ${String(error)}`;
    },
  };

  onMount(() => {
    let disconnect: (() => void) | undefined;
    let disposed = false;
    const attach = (connect: BackendConnector): void => {
      if (!disposed) disconnect = connect(handlers);
    };

    if (hasBackend) {
      attach(connectBackend);
    } else if (import.meta.env.DEV && new URLSearchParams(location.search).has("demo")) {
      // Development-only mock backend (`pnpm dev`, then open /?demo). The
      // branch is statically false in production builds, so the module is
      // never bundled.
      hasBackend = true;
      void import("./lib/dev/demoBackend").then((demo) => attach(demo.connectDemo));
    }

    return () => {
      disposed = true;
      disconnect?.();
      log.dispose();
    };
  });
</script>

<svelte:window bind:innerWidth={viewportWidth} />

<div class="shell">
  <Header
    {status}
    {connections}
    {activity}
    {logOpen}
    onToggleLog={() => (logOpen = !logOpen)}
  />

  <div class="workspace">
    <RollView {status} stats={log.stats} />

    <SidePanel
      id="command-log"
      label={t.log.title}
      resizeLabel={t.log.resize}
      open={logOpen}
      bind:width={logWidth}
      minWidth={PANEL_MIN}
      maxWidth={panelMax}
    >
      {#snippet rail()}
        <CommandLogRail {log} onExpand={() => (logOpen = true)} />
      {/snippet}
      <CommandLogPanel {log} compact={logCompact} onCollapse={() => (logOpen = false)} />
    </SidePanel>
  </div>
</div>

<style>
  .shell {
    display: flex;
    flex-direction: column;
    height: 100vh;
    min-width: 0;
  }

  .workspace {
    flex: 1;
    display: flex;
    min-height: 0;
    min-width: 0;
  }
</style>
