<!--
  One receipt on the roll: a caption (number, time, paper, cut) above a sheet
  of thermal paper. The sheet's edges show how the paper was separated: a
  serrated tear at a full cut, the same with an uncut tab at a partial cut, a
  soft end when the connection closed without a cut, and a straight edge
  running into the printer slot while it is still printing. The bitmap itself
  is drawn by ReceiptTiles (canvas, one pixel per printer dot). QR codes and
  barcodes get a transparent button on top that opens a close-up
  (CodePreview), so a code stays scannable even when the roll is scaled down.
-->
<script lang="ts" module>
  /** How one edge of a sheet looks. */
  export type Edge = "straight" | "full" | "partial" | "soft" | "feed";
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import type { ExportFormat, ImageElement } from "../backend";
  import { formatClock } from "../format";
  import { i18n } from "../i18n";
  import { isCode, isImage } from "../receipt/images";
  import { ReceiptTiles } from "../receipt/tiles";
  import type { RollItem } from "../roll.svelte";
  import Icon from "./Icon.svelte";
  import ReceiptActions from "./ReceiptActions.svelte";

  let {
    item,
    zoom,
    topEdge,
    inSlot,
    hasNext,
    onInspect,
    onSave,
  }: {
    item: RollItem;
    /** CSS pixels per printer dot. */
    zoom: number;
    topEdge: Edge;
    /** The last receipt while printing: its end disappears into the slot. */
    inSlot: boolean;
    /** A receipt follows (a partial cut's tab connects to it). */
    hasNext: boolean;
    /** Opens the close-up of a QR code or barcode. */
    onInspect?: (image: ImageElement, source: string) => void;
    /** Saves this (completed) receipt; absent while printing. */
    onSave?: (item: RollItem, format: ExportFormat) => Promise<void>;
  } = $props();

  const t = $derived(i18n.t.roll);

  let sheet: HTMLElement | undefined = $state();
  let tiles: ReceiptTiles | null = null;
  /** False on the first frame, so a new sheet feeds out from zero height. */
  let entered = $state(false);
  let tearing = $state(false);

  const receipt = $derived(item.receipt);
  const bottomEdge: Edge = $derived(
    item.live ? "feed" : receipt.cut === "full" ? "full" : receipt.cut === "partial" ? "partial" : "soft",
  );
  const cutLabel = $derived(
    item.live ? t.printing : receipt.cut === "full" ? t.cutFull : receipt.cut === "partial" ? t.cutPartial : t.cutNoneShort,
  );
  const paperName = $derived(i18n.t.paper.names[receipt.paper] ?? receipt.paper);
  const time = $derived(formatClock(item.completedAt ?? item.startedAt));
  const codes = $derived(receipt.elements.filter(isImage).filter(isCode));
  const source = $derived(i18n.t.code.source(item.id));
  const kindName = (image: ImageElement): string => i18n.t.code.kinds[image.kind] ?? i18n.t.code.fallbackKind;

  onMount(() => {
    if (!sheet) return;
    // Bitmaps are kept for tiles near the scrolling roll's viewport.
    tiles = new ReceiptTiles(sheet, sheet.closest("[data-scroll-root]"));
    const frame = requestAnimationFrame(() => (entered = true));
    return () => {
      cancelAnimationFrame(frame);
      tiles?.destroy();
      tiles = null;
    };
  });

  $effect(() => {
    tiles?.set(receipt, zoom);
  });

  // Live → completed: a short tear as the sheet leaves the slot.
  let wasLive: boolean | null = null;
  $effect(() => {
    const live = item.live;
    if (wasLive === true && !live) {
      tearing = true;
      const timer = setTimeout(() => (tearing = false), 520);
      wasLive = live;
      return () => clearTimeout(timer);
    }
    wasLive = live;
  });
</script>

<article
  class="receipt"
  class:live={item.live}
  class:in-slot={inSlot}
  class:tearing
  title={t.receiptTitle(item.peer, item.connectionId)}
>
  <header class="meta" style:width="{receipt.width * zoom}px">
    {#if item.live}
      <span class="printing"><span class="dot" aria-hidden="true"></span>{t.printing}</span>
    {:else if item.id !== null}
      <span class="number">{t.receiptNumber(item.id)}</span>
    {/if}
    <time class="time">{time}</time>
    <span class="spacer"></span>
    <span class="detail">{paperName}</span>
    {#if !item.live}
      <span class="sep" aria-hidden="true">·</span>
      <span class="detail" class:soft={receipt.cut == null}>{cutLabel}</span>
    {/if}
    {#if !item.live && item.id !== null && onSave}
      {@const id = item.id}
      <ReceiptActions {id} width={receipt.width} height={receipt.height} onSave={(format) => onSave(item, format)} />
    {/if}
  </header>

  <div
    class="frame"
    style:width="{receipt.width * zoom}px"
    style:height="{entered ? receipt.height * zoom : 0}px"
  >
    <div class="shadow" aria-hidden="true"></div>
    <div
      class="sheet"
      bind:this={sheet}
      data-top={topEdge}
      data-bottom={bottomEdge}
      role="img"
      aria-label={t.receiptLabel(item.id, paperName, item.live ? t.printing : cutLabel)}
    ></div>
    {#if codes.length > 0 && onInspect}
      <div class="codes">
        {#each codes as code, i (i)}
          <button
            type="button"
            class="code"
            style:left="{code.x * zoom}px"
            style:top="{code.y * zoom}px"
            style:width="{code.width * zoom}px"
            style:height="{code.height * zoom}px"
            aria-label={i18n.t.code.inspect(kindName(code))}
            title={i18n.t.code.inspect(kindName(code))}
            onclick={() => onInspect?.(code, source)}
          >
            <span class="badge" aria-hidden="true"><Icon name="zoom" size={12} /></span>
          </button>
        {/each}
      </div>
    {/if}
    {#if bottomEdge === "partial" && hasNext}
      <div class="bridge" aria-hidden="true"></div>
    {/if}
  </div>
</article>

<style>
  .receipt {
    --tooth-w: 8px;
    --tooth-h: 4px;
    --tab-w: 26px;
    --meta-h: 28px;
    --gap-after: 18px;
    display: flex;
    flex-direction: column;
    align-items: center;
    flex: none;
    margin-bottom: var(--gap-after);
    transition: margin-bottom var(--dur-slow) var(--ease-out);
  }
  .receipt.in-slot {
    margin-bottom: 0;
  }

  /* Caption -------------------------------------------------------------- */
  .meta {
    display: flex;
    align-items: center;
    gap: var(--space-1-5);
    height: var(--meta-h);
    min-width: 0;
    max-width: 100%;
    padding: 0 2px var(--space-1-5);
    align-self: center;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    color: var(--text-tertiary);
    white-space: nowrap;
    overflow: hidden;
    font-variant-numeric: tabular-nums;
  }
  .number {
    color: var(--text-secondary);
    font-weight: 700;
  }
  .spacer {
    flex: 1;
  }
  .detail.soft {
    font-style: italic;
  }
  .sep {
    opacity: 0.6;
  }
  .printing {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1-5);
    color: var(--accent);
    font-weight: 700;
  }
  .printing .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
    animation: blink 1.1s ease-in-out infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0.35;
    }
  }

  /* Paper ---------------------------------------------------------------- */
  .frame {
    position: relative;
    flex: none;
    max-width: none;
    transition: height var(--dur-slow) var(--ease-out);
  }

  .shadow {
    position: absolute;
    inset: var(--tooth-h) 1px;
    border-radius: 2px;
    box-shadow: var(--paper-shadow);
  }

  .sheet {
    --edge-straight: linear-gradient(#000 0 0);
    --edge-up: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='8' height='4'%3E%3Cpath d='M0 4H8L4 0Z'/%3E%3C/svg%3E");
    --edge-down: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='8' height='4'%3E%3Cpath d='M0 0H8L4 4Z'/%3E%3C/svg%3E");
    --mask-top: var(--edge-straight);
    --mask-top-tab: none;
    --mask-body: linear-gradient(#000 0 0);
    --mask-bottom-tab: none;
    --mask-bottom: var(--edge-straight);

    position: absolute;
    inset: 0;
    overflow: hidden;
    background-color: var(--paper);
    background-image:
      linear-gradient(
        90deg,
        var(--paper-curl) 0,
        transparent 7%,
        transparent 93%,
        var(--paper-curl) 100%
      ),
      var(--paper-grain);
    background-size:
      100% 100%,
      180px 180px;
    -webkit-mask:
      var(--mask-top) top left / var(--tooth-w) var(--tooth-h) repeat-x,
      var(--mask-top-tab) top center / var(--tab-w) var(--tooth-h) no-repeat,
      var(--mask-body) center / 100% calc(100% - 2 * var(--tooth-h) + 0.5px) no-repeat,
      var(--mask-bottom-tab) bottom center / var(--tab-w) var(--tooth-h) no-repeat,
      var(--mask-bottom) bottom left / var(--tooth-w) var(--tooth-h) repeat-x;
    mask:
      var(--mask-top) top left / var(--tooth-w) var(--tooth-h) repeat-x,
      var(--mask-top-tab) top center / var(--tab-w) var(--tooth-h) no-repeat,
      var(--mask-body) center / 100% calc(100% - 2 * var(--tooth-h) + 0.5px) no-repeat,
      var(--mask-bottom-tab) bottom center / var(--tab-w) var(--tooth-h) no-repeat,
      var(--mask-bottom) bottom left / var(--tooth-w) var(--tooth-h) repeat-x;
  }
  .sheet[data-top="full"],
  .sheet[data-top="partial"] {
    --mask-top: var(--edge-up);
  }
  .sheet[data-top="partial"] {
    --mask-top-tab: var(--edge-straight);
  }
  .sheet[data-bottom="full"],
  .sheet[data-bottom="partial"] {
    --mask-bottom: var(--edge-down);
  }
  .sheet[data-bottom="partial"] {
    --mask-bottom-tab: var(--edge-straight);
  }
  /* No cut: the paper simply runs out of print, fading at its end. */
  .sheet[data-bottom="soft"] {
    --mask-body: linear-gradient(#000 calc(100% - 10px), rgb(0 0 0 / 0.55));
    --mask-bottom: linear-gradient(rgb(0 0 0 / 0.55), rgb(0 0 0 / 0.3));
  }
  /* Still printing but not at the slot (another connection is): fades out. */
  .sheet[data-bottom="feed"] {
    --mask-body: linear-gradient(#000 calc(100% - 28px), rgb(0 0 0 / 0.15));
    --mask-bottom: linear-gradient(rgb(0 0 0 / 0.15) 0 0);
  }
  .in-slot .sheet[data-bottom="feed"] {
    --mask-body: linear-gradient(#000 0 0);
    --mask-bottom: var(--edge-straight);
  }
  .in-slot .shadow {
    bottom: 0;
  }

  .sheet :global(canvas) {
    position: absolute;
    left: 0;
    display: block;
    image-rendering: var(--receipt-rendering, pixelated);
  }

  /* Code close-up buttons ------------------------------------------------ */
  .codes {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
  }
  .code {
    position: absolute;
    padding: 0;
    border: 0;
    border-radius: 3px;
    background: transparent;
    cursor: zoom-in;
    pointer-events: auto;
  }
  .code::before {
    content: "";
    position: absolute;
    inset: -5px;
    border-radius: 6px;
    box-shadow:
      0 0 0 1.5px var(--paper-accent),
      0 0 0 5px color-mix(in srgb, var(--paper-accent) 14%, transparent);
    opacity: 0;
    transition: opacity var(--dur-fast) var(--ease-out);
  }
  .code:focus-visible {
    outline: none;
  }
  .badge {
    position: absolute;
    top: -11px;
    right: -11px;
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--paper-accent);
    color: #fff;
    box-shadow: 0 2px 6px rgb(0 0 0 / 0.25);
    opacity: 0;
    transform: scale(0.6);
    transition:
      opacity var(--dur-fast) var(--ease-out),
      transform var(--dur-base) var(--ease-out);
  }
  .code:hover::before,
  .code:focus-visible::before,
  .code:hover .badge,
  .code:focus-visible .badge {
    opacity: 1;
  }
  .code:hover .badge,
  .code:focus-visible .badge {
    transform: none;
  }

  /* Partial cut: the uncut tab still joins this receipt to the next. */
  .bridge {
    position: absolute;
    top: 100%;
    left: 50%;
    width: var(--tab-w);
    height: calc(var(--gap-after) + var(--meta-h));
    translate: -50% 0;
    background-color: var(--paper);
    background-image: var(--paper-grain);
    background-size: 180px 180px;
    box-shadow:
      inset 1px 0 0 var(--paper-edge),
      inset -1px 0 0 var(--paper-edge);
  }

  /* Motion ---------------------------------------------------------------- */
  .tearing .frame {
    animation: tear 520ms var(--ease-out);
  }
  @keyframes tear {
    0% {
      transform: none;
    }
    28% {
      transform: translateY(-5px) rotate(-0.5deg);
    }
    60% {
      transform: translateY(1px) rotate(0.15deg);
    }
    100% {
      transform: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .printing .dot,
    .tearing .frame {
      animation: none;
    }
  }
</style>
