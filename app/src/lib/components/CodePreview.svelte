<!--
  Close look at a QR code or barcode from the roll: the element's own 1-bit
  bitmap on pure white, scaled by a whole number of device pixels per dot so
  every module keeps its exact proportions (the roll may be scaled to fit the
  window). Big enough for a phone camera to read from the screen.

  A native modal <dialog>: the rest of the window is inert while it is open,
  focus starts on Close and returns to the code's button afterwards, Esc and a
  click outside the card close it. Opens and closes with a short fade and
  lift (none under reduced motion).
-->
<script lang="ts" module>
  import type { ImageElement } from "../backend";

  export interface InspectedCode {
    image: ImageElement;
    /** Which receipt it came from, e.g. "Receipt #4". */
    source: string;
  }
</script>

<script lang="ts">
  import { messages } from "../messages";
  import { duration } from "../motion";
  import { drawScaled } from "../receipt/images";
  import Icon from "./Icon.svelte";

  let { code, onClose }: { code: InspectedCode | null; onClose: () => void } = $props();

  const t = messages.code;
  /** Largest code edge on screen, in CSS pixels. */
  const MAX_EDGE = 440;

  let dialog: HTMLDialogElement | undefined = $state();
  let canvas: HTMLCanvasElement | undefined = $state();
  let shown = $state(false);
  let closing: ReturnType<typeof setTimeout> | null = null;

  const layout = $derived.by(() => {
    if (!code) return null;
    const { width, height } = code.image;
    const dpr = window.devicePixelRatio || 1;
    const maxWidth = Math.min(MAX_EDGE * 1.4, window.innerWidth - 112);
    const maxHeight = Math.min(MAX_EDGE, window.innerHeight - 236);
    // Whole device pixels per dot, at least one.
    const k = Math.max(1, Math.floor(Math.min((maxWidth * dpr) / width, (maxHeight * dpr) / height)));
    return { k, cssWidth: (width * k) / dpr, cssHeight: (height * k) / dpr };
  });

  const title = $derived(code ? (t.titles[code.image.kind] ?? t.fallbackTitle) : "");
  const detail = $derived(
    code && layout
      ? [code.source, t.size(code.image.width, code.image.height), t.scale(layout.k)].join(" · ")
      : "",
  );

  $effect(() => {
    if (!code || !dialog) return;
    if (closing !== null) {
      clearTimeout(closing);
      closing = null;
    }
    if (!dialog.open) dialog.showModal();
    const frame = requestAnimationFrame(() => (shown = true));
    return () => cancelAnimationFrame(frame);
  });

  $effect(() => {
    if (code && canvas && layout) drawScaled(canvas, code.image, layout.k);
  });

  function close(): void {
    if (!dialog?.open || closing !== null) return;
    shown = false;
    closing = setTimeout(() => {
      closing = null;
      dialog?.close();
      onClose();
    }, duration("base"));
  }

  function onCancel(event: Event): void {
    // Esc: animate out instead of vanishing.
    event.preventDefault();
    close();
  }

  function onClick(event: MouseEvent): void {
    // The dialog box is the card itself, so a click that lands on the
    // dialog element is on the backdrop.
    if (event.target === dialog) close();
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
<dialog
  class="preview"
  bind:this={dialog}
  data-open={shown ? "" : undefined}
  aria-labelledby="code-preview-title"
  aria-describedby="code-preview-hint"
  oncancel={onCancel}
  onclick={onClick}
>
  {#if code && layout}
    <div class="card">
      <header class="head">
        <span class="glyph"><Icon name="scan" size={16} /></span>
        <div class="titles">
          <h2 id="code-preview-title">{title}</h2>
          <p class="detail">{detail}</p>
        </div>
        <button type="button" class="icon-btn" aria-label={t.close} title={t.close} onclick={close}>
          <Icon name="x" />
        </button>
      </header>
      <div class="stage">
        <div class="paper">
          <canvas
            bind:this={canvas}
            style:width="{layout.cssWidth}px"
            style:height="{layout.cssHeight}px"
            aria-label={title}
          ></canvas>
        </div>
      </div>
      <p class="hint" id="code-preview-hint">{t.hint}</p>
    </div>
  {/if}
</dialog>

<style>
  .preview {
    max-width: calc(100vw - 32px);
    max-height: calc(100vh - 32px);
    padding: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    box-shadow: var(--shadow-pop);
    color: var(--text);
    overflow: auto;
    opacity: 0;
    transform: translateY(8px) scale(0.97);
    transition:
      opacity var(--dur-base) var(--ease-out),
      transform var(--dur-base) var(--ease-out);
  }
  .preview[data-open] {
    opacity: 1;
    transform: none;
  }
  .preview::backdrop {
    background: var(--scrim);
    backdrop-filter: blur(3px);
    -webkit-backdrop-filter: blur(3px);
    opacity: 0;
    transition: opacity var(--dur-base) var(--ease-out);
  }
  .preview[data-open]::backdrop {
    opacity: 1;
  }
  .preview:focus-visible {
    outline: none;
  }

  .card {
    display: flex;
    flex-direction: column;
    min-width: min(320px, calc(100vw - 32px));
  }

  .head {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-3) var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--border-subtle);
  }
  .glyph {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border: 1px solid var(--accent-border);
    border-radius: var(--radius-md);
    background: var(--accent-soft);
    color: var(--accent);
    flex: none;
  }
  .titles {
    flex: 1;
    min-width: 0;
  }
  h2 {
    margin: 0;
    font-size: var(--text-lg);
    font-weight: var(--weight-semibold);
    line-height: var(--leading-tight);
  }
  .detail {
    margin: 2px 0 0;
    color: var(--text-tertiary);
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .stage {
    display: grid;
    place-items: center;
    padding: var(--space-6);
    background-color: var(--bg-sunken);
    background-image: radial-gradient(var(--desk-dot) 1px, transparent 1.2px);
    background-size: 16px 16px;
  }
  /* Pure white in both themes: scanners want the strongest contrast, and
     the generous padding is the code's quiet zone. */
  .paper {
    /* Wider at the sides: barcodes need about ten modules of quiet zone. */
    padding: var(--space-6) var(--space-10);
    border-radius: var(--radius-md);
    background: #fff;
    box-shadow: var(--paper-shadow);
    line-height: 0;
  }
  canvas {
    display: block;
    image-rendering: pixelated;
  }

  .hint {
    box-sizing: border-box;
    /* Wraps to the card's width instead of widening it. */
    width: 0;
    min-width: 100%;
    margin: 0;
    padding: var(--space-3) var(--space-4) var(--space-4);
    color: var(--text-secondary);
    font-size: var(--text-sm);
    line-height: var(--leading);
  }
</style>
