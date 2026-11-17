<!--
  Central "desk" with the paper roll. A toolbar (paper width, cash drawer,
  receipt count, clear) sits on top. With no receipts the idle state explains
  how to print; otherwise receipts stack in arrival order above a printer slot
  at the bottom, the newest feeding out of it while it prints. The view
  follows new output only while the user is at the bottom (like the log).

  Scale: the canvas bitmap has one pixel per printer dot and is enlarged
  without smoothing. All receipts share one zoom, computed for 80 mm paper,
  so 58 and 80 mm receipts keep their true relative widths. The zoom is at
  most one CSS pixel per dot, snapped to whole device pixels per dot when that
  costs little, and shrinks to fit narrow windows (then smoothed, since
  dropping dots would lose thin lines).
-->
<script lang="ts">
  import { flip } from "svelte/animate";
  import { fade, fly } from "svelte/transition";
  import type { DrawerKicked, Paper } from "../backend";
  import type { SessionStats } from "../commandLog.svelte";
  import { formatBytes, formatCount } from "../format";
  import { messages } from "../messages";
  import { duration, easeOut } from "../motion";
  import type { Roll, RollItem } from "../roll.svelte";
  import type { PrinterStatus } from "../status";
  import DrawerButton from "./DrawerButton.svelte";
  import Icon from "./Icon.svelte";
  import IdleState from "./IdleState.svelte";
  import PaperSelect from "./PaperSelect.svelte";
  import ReceiptSheet, { type Edge } from "./ReceiptSheet.svelte";

  let {
    status,
    stats,
    roll,
    paper,
    onPaperChange,
    drawer,
    onCloseDrawer,
  }: {
    status: PrinterStatus;
    stats: SessionStats;
    roll: Roll;
    paper: Paper;
    onPaperChange: (paper: Paper) => Promise<void>;
    drawer: { open: boolean; kicks: number; last: DrawerKicked | null };
    onCloseDrawer: () => void;
  } = $props();

  const t = messages.roll;
  /** Widest paper, in dots: the zoom is computed for it. */
  const MAX_PAPER_DOTS = 576;
  const MAX_ZOOM = 1;
  const STICKY_THRESHOLD_PX = 32;

  const hasBackend = $derived(status.kind !== "disconnected");
  const items: RollItem[] = $derived([...roll.completed, ...roll.live]);
  const empty = $derived(items.length === 0);
  const printing = $derived(roll.live.length);

  // Zoom ----------------------------------------------------------------------
  let rollWidth = $state(800);
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
    const gutter = rollWidth < 640 ? 16 : 32;
    const fit = Math.max(0.25, (rollWidth - 2 * gutter) / MAX_PAPER_DOTS);
    let device = Math.min(MAX_ZOOM, fit) * dpr;
    if (device >= 1) {
      const snapped = Math.floor(device);
      if (snapped >= device * 0.8) device = snapped;
    }
    return { zoom: device / dpr, pixelated: device >= 1 };
  });

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
  }

  // Edges: a sheet's top edge mirrors how the previous sheet was separated.
  function bottomEdgeOf(item: RollItem): Edge {
    if (item.live) return "feed";
    return item.receipt.cut === "full" ? "full" : item.receipt.cut === "partial" ? "partial" : "soft";
  }
  function topEdgeOf(index: number): Edge {
    const previous = items[index - 1];
    if (!previous) return "straight";
    const edge = bottomEdgeOf(previous);
    return edge === "full" || edge === "partial" ? edge : "straight";
  }

  const slotLabel = $derived(
    status.kind !== "listening" ? t.slotOffline : printing > 0 ? t.slotPrinting(printing) : t.slotReady,
  );
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
      {#if roll.completed.length > 0}
        <span class="count" in:fade={{ duration: duration("fast") }}>{t.receiptCount(roll.completed.length)}</span>
      {/if}
      <span class="spacer"></span>
      <PaperSelect {paper} onChange={onPaperChange} />
      <span class="rule" aria-hidden="true"></span>
      <DrawerButton open={drawer.open} kicks={drawer.kicks} lastKick={drawer.last} onClose={onCloseDrawer} />
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
    <IdleState {status} {stats} />
  {:else}
    <!-- The listeners only observe scroll intent; the roll stays a focusable,
         keyboard-scrollable region. -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
    <div
      class="scroll"
      data-scroll-root
      bind:this={scroller}
      onscroll={onScroll}
      onwheel={onWheel}
      onkeydown={onKeyDown}
      onpointerdown={onPointerDown}
      tabindex="0"
      role="region"
      aria-label={t.rollLabel}
      in:fade={{ duration: duration("base") }}
    >
      <div class="stack" bind:this={stack}>
        {#if roll.dropped > 0}
          <p class="dropped">{t.dropped(roll.dropped)}</p>
        {/if}
        {#each items as item, index (item.key)}
          <div class="item" animate:flip={{ duration: duration("slow"), easing: easeOut }}>
            <ReceiptSheet
              {item}
              zoom={scale.zoom}
              topEdge={topEdgeOf(index)}
              inSlot={item.live && index === items.length - 1}
              hasNext={index < items.length - 1}
            />
          </div>
        {/each}
      </div>

      <div class="printer" data-state={printing > 0 ? "printing" : status.kind === "listening" ? "ready" : "idle"}>
        <div class="mouth" aria-hidden="true"></div>
        <div class="lip">
          <span class="led" aria-hidden="true"></span>
          <span class="state" role="status">{slotLabel}</span>
          <span class="lip-spacer"></span>
          <span class="lip-stats" aria-hidden="true">
            <span>{formatCount(stats.commands)} cmd</span>
            <span>{formatBytes(stats.bytes)}</span>
          </span>
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
    padding: 0 var(--space-2) 0 var(--space-4);
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
    justify-content: center;
    width: 100%;
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
    [data-state="printing"] .led {
      animation: none;
    }
  }
</style>
