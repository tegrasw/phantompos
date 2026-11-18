<!--
  Central "desk" with the paper roll. A toolbar (paper width, receipt count,
  clear) sits on top. With no receipts the idle state explains how to print;
  otherwise receipts stack in arrival order above a printer slot at the
  bottom, the newest feeding out of it while it prints. Data discarded while
  an error state is simulated shows as a quiet notice in the same flow, and
  the slot (LED, label) shows the fault. The view follows new output only
  while the user is at the bottom (like the log).

  Scale: the canvas bitmap has one pixel per printer dot and is enlarged
  without smoothing. All receipts share one zoom, computed for 80 mm paper,
  so 58 and 80 mm receipts keep their true relative widths. The zoom is at
  most one CSS pixel per dot and prefers a whole number of device pixels per
  dot, so 1-bit images (QR modules, barcode bars) keep exact proportions: it
  rounds up when the paper still fits with a narrow gutter, otherwise down
  when that costs little. In windows too narrow for one device pixel per dot
  it shrinks to fit (then smoothed, since dropping dots would lose thin
  lines); codes stay readable through their close-up (CodePreview).
-->
<script lang="ts">
  import { flip } from "svelte/animate";
  import CodePreview, { type InspectedCode } from "./CodePreview.svelte";
  import { fade, fly } from "svelte/transition";
  import { activeFaults, type ExportFormat, type Paper, type PrinterStatus } from "../backend";
  import type { SessionStats } from "../commandLog.svelte";
  import { formatBytes } from "../format";
  import { i18n } from "../i18n";
  import { duration, easeOut } from "../motion";
  import { isNotice, type Roll, type RollEntry, type RollItem } from "../roll.svelte";
  import type { EndpointStatus, ListenerControls } from "../status";
  import Icon from "./Icon.svelte";
  import IdleState from "./IdleState.svelte";
  import PaperSelect from "./PaperSelect.svelte";
  import ReceiptSheet, { type Edge } from "./ReceiptSheet.svelte";
  import SuppressedNotice from "./SuppressedNotice.svelte";

  let {
    status,
    printer,
    stats,
    roll,
    paper,
    controls,
    onPaperChange,
    onClear,
    onSave,
  }: {
    status: EndpointStatus;
    printer: PrinterStatus;
    stats: SessionStats;
    roll: Roll;
    paper: Paper;
    controls: ListenerControls;
    onPaperChange: (paper: Paper) => Promise<void>;
    /** After the roll was cleared here (the backend is told). */
    onClear: () => void;
    onSave: (item: RollItem, format: ExportFormat) => Promise<void>;
  } = $props();

  const t = $derived(i18n.t.roll);
  /** Widest paper, in dots: the zoom is computed for it. */
  const MAX_PAPER_DOTS = 576;
  const MAX_ZOOM = 1;
  /** Narrowest gutter beside the paper when rounding the zoom up. */
  const TIGHT_GUTTER = 12;
  const STICKY_THRESHOLD_PX = 32;

  const hasBackend = $derived(status.kind !== "disconnected");
  const items: RollEntry[] = $derived([...roll.completed, ...roll.live]);
  const empty = $derived(items.length === 0);
  const printing = $derived(roll.live.length);
  const faults = $derived(activeFaults(printer));

  // Zoom ----------------------------------------------------------------------
  let rollWidth = $state(800);
  /** Width of the scrolling roll (without its scrollbar); 0 while empty. */
  let scrollWidth = $state(0);
  let dpr = $state(window.devicePixelRatio || 1);

  $effect(() => {
    // Re-arm on every change: the query matches only the current ratio.
    const query = matchMedia(`(resolution: ${dpr}dppx)`);
    const update = (): void => {
      dpr = window.devicePixelRatio || 1;
    };
    query.addEventListener("change", update, { once: true });
    return () => query.removeEventListener("change", update);
  });

  const scale = $derived.by(() => {
    const width = scrollWidth || rollWidth;
    const gutter = width < 640 ? 16 : 32;
    const fit = Math.max(0.25, (width - 2 * gutter) / MAX_PAPER_DOTS);
    let device = Math.min(MAX_ZOOM, fit) * dpr;
    const up = Math.ceil(device - 1e-6);
    if (up / dpr <= MAX_ZOOM && (up / dpr) * MAX_PAPER_DOTS <= width - 2 * TIGHT_GUTTER) {
      device = up;
    } else if (device >= 1) {
      const down = Math.floor(device);
      if (down >= device * 0.8) device = down;
    }
    return { zoom: device / dpr, pixelated: device >= 1 };
  });

  // Centring by flexbox can leave a sheet half a pixel off the device grid,
  // which resamples every dot even at an integer zoom: place sheets on whole
  // device pixels instead.
  let stackWidth = $state(0);
  const STACK_PAD_X = 16; // keep in sync with .stack padding
  function paperLeft(dots: number): number {
    const free = stackWidth - 2 * STACK_PAD_X - dots * scale.zoom;
    return Math.floor((free / 2) * dpr) / dpr;
  }

  // Code close-up -------------------------------------------------------------
  let inspected: InspectedCode | null = $state(null);

  // Follow the newest output ------------------------------------------------
  let scroller: HTMLElement | undefined = $state();
  let stack: HTMLElement | undefined = $state();
  let follow = $state(true);
  let detachedAt = $state(0);
  const newSinceDetach = $derived(follow ? 0 : Math.max(0, roll.completedTotal - detachedAt));

  function toBottom(): void {
    if (scroller) scroller.scrollTop = scroller.scrollHeight;
  }

  $effect(() => {
    // Sheets grow (feed animation, live snapshots) without changing `items`;
    // a ResizeObserver keeps the bottom in view through all of it.
    if (!stack) return;
    const observer = new ResizeObserver(() => {
      if (follow) toBottom();
    });
    observer.observe(stack);
    return () => observer.disconnect();
  });

  $effect(() => {
    void items;
    if (follow) toBottom();
  });

  function detach(): void {
    if (!follow) return;
    follow = false;
    detachedAt = roll.completedTotal;
  }
  // Only user intent detaches (wheel, keys, scrollbar): sheets grow between
  // our own scroll adjustments, so a scroll event alone proves nothing.
  // Reaching the bottom again re-attaches.
  function onScroll(): void {
    if (!scroller || follow) return;
    const distance = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight;
    if (distance <= STICKY_THRESHOLD_PX) follow = true;
  }
  function onWheel(event: WheelEvent): void {
    if (event.deltaY < 0) detach();
  }
  function onKeyDown(event: KeyboardEvent): void {
    if (["ArrowUp", "PageUp", "Home"].includes(event.key)) detach();
  }
  function onPointerDown(event: PointerEvent): void {
    if (event.target === scroller) detach();
  }
  function jumpToLatest(): void {
    follow = true;
    scroller?.scrollTo({ top: scroller.scrollHeight, behavior: duration("base") ? "smooth" : "auto" });
  }

  function clear(): void {
    roll.clear();
    follow = true;
    onClear();
  }

  // Edges: a sheet's top edge mirrors how the previous sheet was separated.
  function bottomEdgeOf(item: RollItem): Edge {
    if (item.live) return "feed";
    return item.receipt.cut === "full" ? "full" : item.receipt.cut === "partial" ? "partial" : "soft";
  }
  // Notices are not paper: edges look through them to the receipt before.
  function topEdgeOf(index: number): Edge {
    let previous: RollEntry | undefined;
    for (let i = index - 1; i >= 0 && !previous; i--) if (!isNotice(items[i])) previous = items[i];
    if (!previous || isNotice(previous)) return "straight";
    const edge = bottomEdgeOf(previous);
    return edge === "full" || edge === "partial" ? edge : "straight";
  }
  function hasNextSheet(index: number): boolean {
    const next = items[index + 1];
    return next !== undefined && !isNotice(next);
  }

  const slotState = $derived(
    status.kind !== "listening" ? "idle" : faults.length > 0 ? "fault" : printing > 0 ? "printing" : "ready",
  );
  const slotLabel = $derived.by(() => {
    switch (slotState) {
      case "idle":
        return t.slotOffline;
      case "fault":
        return t.slotFault(i18n.t.printer.faults[faults[0]], faults.length - 1);
      case "printing":
        return t.slotPrinting(printing);
      default:
        return t.slotReady;
    }
  });
</script>

<main
  class="roll"
  bind:clientWidth={rollWidth}
  style:--receipt-rendering={scale.pixelated ? "pixelated" : "auto"}
  style:--paper-span="{MAX_PAPER_DOTS * scale.zoom}px"
>
  {#if hasBackend}
    <div class="toolbar">
      <span class="heading">
        <Icon name="receipt" size={15} />
        <span class="heading-text">{t.toolbarLabel}</span>
      </span>
      {#if roll.receiptCount > 0}
        <span class="count" in:fade={{ duration: duration("fast") }}>{t.receiptCount(roll.receiptCount)}</span>
      {/if}
      <span class="spacer"></span>
      <PaperSelect {paper} onChange={onPaperChange} />
      <span class="rule" aria-hidden="true"></span>
      <button
        type="button"
        class="icon-btn"
        title={roll.completed.length > 0 ? t.clear : t.clearDisabled}
        aria-label={t.clear}
        disabled={roll.completed.length === 0}
        onclick={clear}
      >
        <Icon name="trash" />
      </button>
    </div>
  {/if}

  {#if empty}
    <IdleState
      {status}
      {stats}
      {controls}
      fault={faults.length > 0 ? i18n.t.printer.list(faults.map((f) => i18n.t.printer.faults[f])) : null}
    />
  {:else}
    <!-- The listeners only observe scroll intent; the roll stays a focusable,
         keyboard-scrollable region. -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
    <div
      class="scroll"
      data-scroll-root
      bind:this={scroller}
      bind:clientWidth={scrollWidth}
      onscroll={onScroll}
      onwheel={onWheel}
      onkeydown={onKeyDown}
      onpointerdown={onPointerDown}
      tabindex="0"
      role="region"
      aria-label={t.rollLabel}
      in:fade={{ duration: duration("base") }}
    >
      <div class="stack" bind:this={stack} bind:clientWidth={stackWidth}>
        {#if roll.dropped > 0}
          <p class="dropped">{t.dropped(roll.dropped)}</p>
        {/if}
        {#each items as entry, index (entry.key)}
          <div
            class="item"
            class:notice-item={isNotice(entry)}
            style:--paper-left={isNotice(entry) ? undefined : `${paperLeft(entry.receipt.width)}px`}
            animate:flip={{ duration: duration("slow"), easing: easeOut }}
          >
            {#if isNotice(entry)}
              <SuppressedNotice notice={entry} />
            {:else}
              <ReceiptSheet
                item={entry}
                zoom={scale.zoom}
                topEdge={topEdgeOf(index)}
                inSlot={entry.live && index === items.length - 1}
                hasNext={hasNextSheet(index)}
                onInspect={(image, source) => (inspected = { image, source })}
                {onSave}
              />
            {/if}
          </div>
        {/each}
      </div>

      <div class="printer" data-state={slotState}>
        <div class="mouth" aria-hidden="true"></div>
        <div class="lip">
          <span class="led" aria-hidden="true"></span>
          <span class="state" role="status">{slotLabel}</span>
          <span class="lip-spacer"></span>
          {#if slotState === "fault"}
            <span class="lip-halted" in:fade={{ duration: duration("fast") }}>
              <Icon name="pause" size={10} />{t.slotHalted}
            </span>
          {:else}
            <span class="lip-stats" aria-hidden="true">
              <span>{t.slotCommands(stats.commands)}</span>
              <span>{formatBytes(stats.bytes)}</span>
            </span>
          {/if}
        </div>
      </div>
    </div>

    {#if !follow}
      <button
        type="button"
        class="jump"
        onclick={jumpToLatest}
        transition:fly={{ y: 8, duration: duration("base"), easing: easeOut }}
      >
        <Icon name="arrowDown" size={14} />
        {newSinceDetach > 0 ? t.newReceipts(newSinceDetach) : t.jumpToLatest}
      </button>
    {/if}
  {/if}

  <CodePreview code={inspected} onClose={() => (inspected = null)} />
</main>

<style>
  .roll {
    position: relative;
    flex: 1;
    min-width: 0;
    height: 100%;
    display: flex;
    flex-direction: column;
    container: roll / inline-size;
    background-color: var(--bg-app);
    background-image: radial-gradient(var(--desk-dot) 1px, transparent 1.2px);
    background-size: 16px 16px;
    background-position: -8px -8px;
  }

  /* Toolbar ---------------------------------------------------------------- */
  .toolbar {
    position: relative;
    z-index: 4;
    flex: none;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    height: 44px;
    padding: 0 var(--space-3) 0 var(--space-4);
    border-bottom: 1px solid var(--border);
    background: color-mix(in srgb, var(--bg-surface) 82%, transparent);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    min-width: 0;
  }
  .heading {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1-5);
    color: var(--text-secondary);
    font-size: var(--text-sm);
    font-weight: var(--weight-semibold);
  }
  .count {
    padding: 0 var(--space-1-5);
    border-radius: var(--radius-pill);
    background: var(--bg-active);
    color: var(--text-secondary);
    font-size: var(--text-2xs);
    font-weight: var(--weight-semibold);
    line-height: 18px;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .spacer {
    flex: 1;
    min-width: 0;
  }
  .rule {
    width: 1px;
    height: 18px;
    background: var(--border);
    flex: none;
  }

  /* Roll ------------------------------------------------------------------- */
  .scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    display: flex;
    flex-direction: column;
    outline: none;
  }
  .scroll:focus-visible {
    box-shadow: inset 0 0 0 2px var(--focus-ring);
  }
  .stack {
    margin-top: auto;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: var(--space-6) var(--space-4) 0;
  }
  .item {
    display: flex;
    justify-content: flex-start;
    width: 100%;
  }
  /* May be negative when the paper is wider than the padded stack. */
  .item:not(.notice-item) > :global(*) {
    margin-left: var(--paper-left, 0);
  }
  .dropped {
    max-width: 44ch;
    margin: 0 0 var(--space-4);
    color: var(--text-tertiary);
    font-size: var(--text-xs);
    text-align: center;
  }

  /* Printer slot ----------------------------------------------------------- */
  .printer {
    position: sticky;
    bottom: 0;
    z-index: 3;
    flex: none;
    display: flex;
    flex-direction: column;
    align-items: center;
    margin-top: -10px;
    pointer-events: none;
  }
  /* Dark throat the paper comes out of; its shadow falls on the paper. */
  .mouth {
    width: min(calc(var(--paper-span) + 28px), calc(100% - 16px));
    height: 10px;
    border-radius: 6px 6px 0 0;
    background: linear-gradient(to bottom, transparent, rgb(0 0 0 / 0.14));
  }
  .lip {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    width: min(calc(var(--paper-span) + 72px), 100%);
    height: 38px;
    padding: 0 var(--space-4);
    border: 1px solid var(--border-strong);
    border-bottom: 0;
    border-radius: 12px 12px 0 0;
    background: linear-gradient(to bottom, var(--bg-raised), var(--bg-surface));
    box-shadow:
      0 -1px 0 rgb(0 0 0 / 0.04),
      0 -8px 24px -12px rgb(0 0 0 / 0.35),
      var(--inset-highlight);
    position: relative;
    pointer-events: auto;
  }
  /* The slot itself: a dark groove along the top of the lip. */
  .lip::before {
    content: "";
    position: absolute;
    top: -1px;
    left: 50%;
    width: min(calc(var(--paper-span) + 14px), calc(100% - 40px));
    height: 4px;
    translate: -50% 0;
    border-radius: 0 0 3px 3px;
    background: linear-gradient(to bottom, var(--slot), color-mix(in srgb, var(--slot) 70%, transparent));
    box-shadow: 0 1px 0 var(--inset-light, rgb(255 255 255 / 0.5));
  }
  .led {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--border-strong);
    flex: none;
    transition:
      background-color var(--dur-base) var(--ease-out),
      box-shadow var(--dur-base) var(--ease-out);
  }
  [data-state="ready"] .led {
    background: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  [data-state="printing"] .led {
    background: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft), 0 0 10px var(--accent-glow);
    animation: led 0.9s ease-in-out infinite;
  }
  @keyframes led {
    50% {
      opacity: 0.4;
    }
  }
  .state {
    color: var(--text-secondary);
    font-size: var(--text-xs);
    font-weight: var(--weight-semibold);
    white-space: nowrap;
  }
  [data-state="printing"] .state {
    color: var(--accent);
  }
  /* Simulated fault: a steady --fault LED (blinking like a real error LED),
     the fault named on the lip and a faint hatch over the throat. */
  [data-state="fault"] .led {
    background: var(--fault);
    box-shadow: 0 0 0 3px var(--fault-soft), 0 0 10px var(--fault-glow);
    animation: blink 1.6s steps(1, end) infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0.35;
    }
  }
  [data-state="fault"] .state {
    color: var(--fault);
  }
  [data-state="fault"] .mouth {
    background:
      repeating-linear-gradient(135deg, var(--fault-soft) 0 6px, transparent 6px 12px),
      linear-gradient(to bottom, transparent, rgb(0 0 0 / 0.14));
  }
  [data-state="fault"] .lip {
    border-color: var(--fault-border);
  }
  .lip-halted {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    padding: 0 var(--space-2) 0 var(--space-1-5);
    border: 1px solid var(--fault-border);
    border-radius: var(--radius-pill);
    background: var(--fault-soft);
    color: var(--fault);
    font-size: var(--text-2xs);
    font-weight: var(--weight-semibold);
    line-height: 18px;
    white-space: nowrap;
  }
  .lip-spacer {
    flex: 1;
  }
  .lip-stats {
    display: inline-flex;
    gap: var(--space-3);
    color: var(--text-tertiary);
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }

  .jump {
    position: absolute;
    left: 50%;
    bottom: 54px;
    z-index: 5;
    translate: -50% 0;
    display: inline-flex;
    align-items: center;
    gap: var(--space-1-5);
    height: 30px;
    padding: 0 var(--space-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    background: var(--bg-raised);
    box-shadow: var(--shadow-pop);
    color: var(--text);
    font-size: var(--text-sm);
    font-weight: var(--weight-medium);
    cursor: pointer;
    white-space: nowrap;
  }
  .jump:hover {
    border-color: var(--border-strong);
  }

  @container roll (max-width: 560px) {
    .heading-text,
    .lip-stats {
      display: none;
    }
    .toolbar {
      padding-left: var(--space-3);
    }
  }
  @container roll (max-width: 420px) {
    .count {
      display: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    [data-state="printing"] .led,
    [data-state="fault"] .led {
      animation: none;
    }
  }
</style>
