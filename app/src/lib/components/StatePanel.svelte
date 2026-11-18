<!--
  Printer state panel (left side): an illustration and one-line verdict, the
  three simulated error states as switches, the cash drawer, and a summary of
  what the printer currently reports to the host, so each switch's
  consequence is visible. Changes are optimistic (App rolls back on error).
-->
<script lang="ts">
  import { fade } from "svelte/transition";
  import { activeFaults, PRINTER_FAULTS, type DrawerKicked, type PrinterFault, type PrinterStatus } from "../backend";
  import type { LastReply } from "../commandLog.svelte";
  import { formatClock } from "../format";
  import { hexByte } from "../escpos";
  import { messages } from "../messages";
  import { duration } from "../motion";
  import DrawerIndicator from "./DrawerIndicator.svelte";
  import Icon from "./Icon.svelte";
  import PrinterArt from "./PrinterArt.svelte";
  import StateSwitch from "./StateSwitch.svelte";

  let {
    printer,
    drawer,
    printing,
    lastReply,
    error,
    onToggle,
    onCloseDrawer,
    onCollapse,
  }: {
    printer: PrinterStatus;
    drawer: { kicks: number; last: DrawerKicked | null };
    /** Receipts currently printing. */
    printing: boolean;
    lastReply: LastReply | null;
    /** Last failure to change the state, if any. */
    error: string | null;
    onToggle: (fault: PrinterFault) => void;
    onCloseDrawer: () => void;
    onCollapse: () => void;
  } = $props();

  const t = messages.printer;
  const r = t.reports;

  const faults = $derived(activeFaults(printer));
  const halted = $derived(faults.length > 0);
  const offlineReported = $derived(halted);

  const reports = $derived([
    { key: "status", label: r.connection, value: offlineReported ? r.offline : r.online, bad: offlineReported },
    { key: "paper", label: r.paper, value: printer.paperOut ? r.paperEnd : r.paperPresent, bad: printer.paperOut },
    { key: "cover", label: r.cover, value: printer.coverOpen ? r.coverOpen : r.coverClosed, bad: printer.coverOpen },
    {
      key: "drawer",
      label: r.drawer,
      value: printer.drawerOpen ? r.drawerOpen : r.drawerClosed,
      bad: false,
      accent: printer.drawerOpen,
    },
    { key: "printing", label: r.printing, value: halted ? r.printingOff : r.printingOn, bad: halted },
  ]);

  const verdict = $derived(
    halted
      ? { title: t.stateHalted, detail: t.stateHaltedDetail }
      : { title: t.stateReady, detail: t.stateReadyDetail },
  );
</script>

<div class="state-panel" data-halted={halted}>
  <div class="toolbar">
    <h2 class="heading">{t.title}</h2>
    <span class="spacer"></span>
    <button type="button" class="icon-btn" title={t.collapse} aria-label={t.collapse} onclick={onCollapse}>
      <Icon name="chevronLeft" />
    </button>
  </div>

  <div class="scroll">
    <section class="hero">
      <PrinterArt
        paperOut={printer.paperOut}
        coverOpen={printer.coverOpen}
        offline={printer.offline}
        {printing}
      />
      {#key halted}
        <p class="verdict" role="status" in:fade={{ duration: duration("base") }}>
          <span class="verdict-dot" aria-hidden="true"></span>
          <span class="verdict-title">{verdict.title}</span>
          <span class="verdict-detail">{verdict.detail}</span>
        </p>
      {/key}
    </section>

    <section class="section" aria-labelledby="state-switches">
      <h3 class="section-title" id="state-switches">{t.statesLabel}</h3>
      <div class="switches">
        {#each PRINTER_FAULTS as fault (fault)}
          <StateSwitch
            id="switch-{fault}"
            checked={printer[fault]}
            label={t.faults[fault]}
            hint={t.faultHints[fault]}
            onToggle={() => onToggle(fault)}
          />
        {/each}
      </div>
      {#if error}
        <p class="error" role="alert" transition:fade={{ duration: duration("fast") }}>
          <Icon name="alert" size={12} />{error}
        </p>
      {/if}
    </section>

    <section class="section">
      <DrawerIndicator open={printer.drawerOpen} kicks={drawer.kicks} lastKick={drawer.last} onClose={onCloseDrawer} />
    </section>

    <section class="section" aria-labelledby="state-reports">
      <h3 class="section-title" id="state-reports">{t.reportsLabel}</h3>
      <dl class="reports">
        {#each reports as row (row.key)}
          <div class="report" class:bad={row.bad} class:accent={row.accent}>
            <dt>{row.label}</dt>
            <dd><span class="report-dot" aria-hidden="true"></span>{row.value}</dd>
          </div>
        {/each}
      </dl>

      <div class="last">
        <span class="last-label">{t.lastReply}</span>
        {#if lastReply}
          <span class="last-value" title={lastReply.peer}>
            <span class="last-request">{lastReply.request}</span>
            <span class="last-arrow" aria-hidden="true">&rarr;</span>
            <span class="last-bytes">{lastReply.bytes.map(hexByte).join(" ")}</span>
            <time class="last-time">{formatClock(lastReply.at)}</time>
          </span>
        {:else}
          <span class="last-none">{t.noReplies}</span>
        {/if}
      </div>
      <p class="hint">{t.answered}</p>
    </section>
  </div>
</div>

<style>
  .state-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-width: 0;
  }

  .toolbar {
    flex: none;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    height: 44px;
    padding: 0 var(--space-2) 0 var(--space-4);
    border-bottom: 1px solid var(--border-subtle);
  }
  .heading {
    margin: 0;
    font-size: var(--text-md);
    font-weight: var(--weight-semibold);
  }
  .spacer {
    flex: 1;
  }

  .scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    overscroll-behavior: contain;
    padding-bottom: var(--space-6);
  }

  /* Hero ----------------------------------------------------------------- */
  .hero {
    padding: var(--space-5) var(--space-4) var(--space-3);
    background: radial-gradient(120% 90% at 50% 0%, var(--bg-sunken), transparent 70%);
  }
  .verdict {
    display: grid;
    grid-template-columns: auto 1fr;
    column-gap: var(--space-2);
    align-items: center;
    margin: var(--space-3) 0 0;
    padding: var(--space-2) var(--space-3);
    border: 1px solid var(--accent-border);
    border-radius: var(--radius-md);
    background: var(--accent-soft);
  }
  [data-halted="true"] .verdict {
    border-color: var(--fault-border);
    background: var(--fault-soft);
  }
  .verdict-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  [data-halted="true"] .verdict-dot {
    background: var(--fault);
    box-shadow: 0 0 0 3px var(--fault-soft);
  }
  .verdict-title {
    color: var(--accent);
    font-size: var(--text-sm);
    font-weight: var(--weight-semibold);
  }
  [data-halted="true"] .verdict-title {
    color: var(--fault);
  }
  .verdict-detail {
    grid-column: 2;
    color: var(--text-secondary);
    font-size: var(--text-xs);
    line-height: var(--leading-tight);
  }

  /* Sections ------------------------------------------------------------- */
  .section {
    padding: var(--space-3) var(--space-3) 0;
  }
  .section-title {
    margin: 0 0 var(--space-1-5) var(--space-1);
    color: var(--text-tertiary);
    font-size: var(--text-2xs);
    font-weight: var(--weight-semibold);
    letter-spacing: var(--tracking-caps);
    text-transform: uppercase;
  }
  .switches {
    display: flex;
    flex-direction: column;
    gap: var(--space-0-5);
  }
  .error {
    display: flex;
    align-items: center;
    gap: var(--space-1-5);
    margin: var(--space-2) var(--space-1) 0;
    /* Neutral on purpose: red is reserved for listener errors. */
    color: var(--text-secondary);
    font-size: var(--text-xs);
    font-weight: var(--weight-medium);
  }

  /* Reports -------------------------------------------------------------- */
  .reports {
    margin: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-sunken);
    overflow: hidden;
  }
  .report {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
    height: 28px;
    padding: 0 var(--space-3);
  }
  .report + .report {
    border-top: 1px solid var(--border-subtle);
  }
  .report dt {
    color: var(--text-secondary);
    font-size: var(--text-xs);
  }
  .report dd {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1-5);
    margin: 0;
    color: var(--text);
    font-size: var(--text-xs);
    font-weight: var(--weight-semibold);
    transition: color var(--dur-base) var(--ease-out);
  }
  .report-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--border-strong);
    transition: background-color var(--dur-base) var(--ease-out);
  }
  .report.bad dd {
    color: var(--fault);
  }
  .report.bad .report-dot {
    background: var(--fault);
  }
  .report.accent dd {
    color: var(--accent);
  }
  .report.accent .report-dot {
    background: var(--accent);
  }

  .last {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-top: var(--space-2);
    padding: var(--space-2) var(--space-3);
    border: 1px dashed var(--border);
    border-radius: var(--radius-md);
  }
  .last-label {
    color: var(--text-tertiary);
    font-size: var(--text-2xs);
    font-weight: var(--weight-semibold);
    letter-spacing: var(--tracking-caps);
    text-transform: uppercase;
  }
  .last-value {
    display: flex;
    align-items: baseline;
    gap: var(--space-1-5);
    min-width: 0;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    white-space: nowrap;
  }
  .last-request {
    color: var(--text);
    font-weight: 700;
  }
  .last-arrow {
    color: var(--text-tertiary);
  }
  .last-bytes {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--accent);
    font-weight: 700;
  }
  .last-time {
    margin-left: auto;
    color: var(--text-tertiary);
    font-size: var(--text-2xs);
  }
  .last-none {
    color: var(--text-tertiary);
    font-size: var(--text-xs);
  }
  .hint {
    margin: var(--space-2) var(--space-1) 0;
    color: var(--text-tertiary);
    font-size: var(--text-xs);
    line-height: var(--leading);
    text-wrap: pretty;
  }
</style>
