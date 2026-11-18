<script lang="ts">
  import { onMount } from "svelte";
  import {
    connectBackend,
    runningInTauri,
    type BackendConnector,
    type BackendHandlers,
    type BackendSession,
    NORMAL_STATUS,
    type ConnectionsStatus,
    type DrawerKicked,
    type ListenerStatus,
    type Paper,
    type PrinterFault,
    type PrinterStatus,
  } from "./lib/backend";
  import { CommandLog } from "./lib/commandLog.svelte";
  import CommandLogPanel from "./lib/components/CommandLog.svelte";
  import CommandLogRail from "./lib/components/CommandLogRail.svelte";
  import Header from "./lib/components/Header.svelte";
  import RollView from "./lib/components/RollView.svelte";
  import SidePanel from "./lib/components/SidePanel.svelte";
  import StatePanel from "./lib/components/StatePanel.svelte";
  import StatePanelRail from "./lib/components/StatePanelRail.svelte";
  import { messages } from "./lib/messages";
  import { Roll } from "./lib/roll.svelte";
  import { endpointStatus } from "./lib/status";

  const t = messages;

  let hasBackend = $state(runningInTauri());
  let listener = $state<ListenerStatus | null>(null);
  let connections = $state<ConnectionsStatus>({ active: 0, peers: [] });
  let backendError = $state<string | null>(null);
  /** Bumped (at most every ACTIVITY_MS) when commands arrive. */
  let activity = $state(0);

  const log = new CommandLog();
  const roll = new Roll();
  /** Paper for new receipts (memory only until the settings of stage 6). */
  let paper = $state<Paper>("80mm");
  /** The simulated printer state (`printer:status`). */
  let printer = $state<PrinterStatus>({ ...NORMAL_STATUS });
  /** Drawer kicks since launch (replays the pulse) and the latest one. */
  let drawer = $state<{ kicks: number; last: DrawerKicked | null }>({ kicks: 0, last: null });
  let printerError = $state<string | null>(null);
  let session: BackendSession | null = null;
  const status = $derived(endpointStatus(hasBackend, listener, backendError));

  // Layout ------------------------------------------------------------------
  // Printer state on the left (fixed width, collapses to a rail), command log
  // on the right (resizable). When the window cannot fit both open beside a
  // usable roll, opening one collapses the other; shrinking the window
  // collapses the one opened earlier.
  const PANEL_MIN = 380;
  const ROLL_MIN = 260;
  const STATE_W = 272; // keep in sync with --state-w
  const RAIL_W = 44; // keep in sync with --rail-w
  const STORE_OPEN = "phantompos.commandLog.open";
  const STORE_WIDTH = "phantompos.commandLog.width";
  const STORE_STATE_OPEN = "phantompos.statePanel.open";

  let viewportWidth = $state(window.innerWidth);
  let logOpen = $state(readStored(STORE_OPEN) !== "false");
  let stateOpen = $state(readStored(STORE_STATE_OPEN) !== "false");
  /** The panel opened most recently wins when both cannot fit. */
  let lastOpened: "state" | "log" = "log";
  const fitsBoth = $derived(viewportWidth >= STATE_W + PANEL_MIN + ROLL_MIN);
  let logWidth = $state(
    Number(readStored(STORE_WIDTH)) ||
      // 46 %: at the default 1200 px window an 80 mm receipt keeps one CSS
      // pixel per dot beside the log (crisp codes, see RollView).
      Math.round(Math.min(680, Math.max(460, window.innerWidth * 0.46))),
  );
  const panelMax = $derived(Math.max(PANEL_MIN, viewportWidth - ROLL_MIN - (stateOpen ? STATE_W : RAIL_W)));
  /** Below this width the log stacks bytes under each command. */
  const COMPACT_BELOW = 520;
  const logCompact = $derived(Math.min(logWidth, panelMax) < COMPACT_BELOW);

  $effect(() => writeStored(STORE_OPEN, String(logOpen)));
  $effect(() => writeStored(STORE_WIDTH, String(logWidth)));
  $effect(() => writeStored(STORE_STATE_OPEN, String(stateOpen)));

  $effect(() => {
    if (fitsBoth || !logOpen || !stateOpen) return;
    if (lastOpened === "state") logOpen = false;
    else stateOpen = false;
  });

  function setLogOpen(open: boolean): void {
    logOpen = open;
    if (!open) return;
    lastOpened = "log";
    if (!fitsBoth) stateOpen = false;
  }
  function setStateOpen(open: boolean): void {
    stateOpen = open;
    if (!open) return;
    lastOpened = "state";
    if (!fitsBoth) logOpen = false;
  }

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
    onReceiptUpdated: (update) => roll.update(update),
    onReceiptCompleted: (receipt) => roll.complete(receipt),
    onDrawerKicked: (kick) => {
      drawer = { kicks: drawer.kicks + 1, last: kick };
      // `printer:status` follows; open the indicator without waiting for it.
      printer = { ...printer, drawerOpen: true };
    },
    onPrinterStatus: (next) => (printer = next),
    onStatusSent: (sent) => log.pushReply(sent),
    onPrintSuppressed: (suppressed) => roll.suppressed(suppressed),
    onInitialState: (state) => {
      listener = state.listener;
      connections = state.connections;
      // Tolerate a backend that predates these fields.
      if (state.paper) paper = state.paper;
      if (state.status) printer = { ...NORMAL_STATUS, ...state.status };
      backendError = null;
    },
    onError: (error) => {
      backendError = `${t.header.stateUnavailable}: ${String(error)}`;
    },
  };

  /** Optimistic: shows the new paper at once, rolls back if refused. */
  async function changePaper(next: Paper): Promise<void> {
    const previous = paper;
    paper = next;
    try {
      if (!session) throw new Error("No backend");
      paper = await session.setPaper(next);
    } catch (error) {
      paper = previous;
      throw error;
    }
  }

  /**
   * Optimistic: shows the change at once; on failure only the fields this
   * call changed go back (anything `printer:status` set meanwhile stays).
   */
  async function changePrinter(patch: Partial<PrinterStatus>, failure: string): Promise<void> {
    const keys = Object.keys(patch) as (keyof PrinterStatus)[];
    const previous = Object.fromEntries(keys.map((key) => [key, printer[key]])) as Partial<PrinterStatus>;
    printer = { ...printer, ...patch };
    printerError = null;
    try {
      if (!session) throw new Error("No backend");
      printer = await session.setPrinterStatus(patch);
    } catch {
      const reverted = { ...printer };
      for (const key of keys) if (printer[key] === patch[key]) reverted[key] = previous[key] as boolean;
      printer = reverted;
      printerError = failure;
    }
  }

  function toggleFault(fault: PrinterFault): void {
    void changePrinter({ [fault]: !printer[fault] }, messages.printer.changeFailed);
  }

  function closeDrawer(): void {
    if (printer.drawerOpen) void changePrinter({ drawerOpen: false }, messages.drawer.closeFailed);
  }

  onMount(() => {
    let disposed = false;
    const attach = (connect: BackendConnector): void => {
      if (!disposed) session = connect(handlers);
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
      session?.disconnect();
      session = null;
      log.dispose();
      roll.dispose();
    };
  });
</script>

<svelte:window bind:innerWidth={viewportWidth} />

<div class="shell">
  <Header
    {status}
    {printer}
    {connections}
    {activity}
    {logOpen}
    {stateOpen}
    onToggleLog={() => setLogOpen(!logOpen)}
    onToggleState={() => setStateOpen(!stateOpen)}
    onShowState={() => setStateOpen(true)}
  />

  <div class="workspace">
    {#if hasBackend}
      <SidePanel
        id="printer-state"
        label={t.printer.title}
        side="left"
        resizable={false}
        open={stateOpen}
        width={STATE_W}
        minWidth={STATE_W}
        maxWidth={STATE_W}
      >
        {#snippet rail()}
          <StatePanelRail {printer} onExpand={() => setStateOpen(true)} />
        {/snippet}
        <StatePanel
          {printer}
          {drawer}
          printing={roll.live.length > 0}
          lastReply={log.lastReply}
          error={printerError}
          onToggle={toggleFault}
          onCloseDrawer={closeDrawer}
          onCollapse={() => setStateOpen(false)}
        />
      </SidePanel>
    {/if}

    <RollView {status} {printer} stats={log.stats} {roll} {paper} onPaperChange={changePaper} />

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
        <CommandLogRail {log} onExpand={() => setLogOpen(true)} />
      {/snippet}
      <CommandLogPanel {log} compact={logCompact} onCollapse={() => setLogOpen(false)} />
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
