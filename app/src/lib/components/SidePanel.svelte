<!--
  Collapsible side panel docked to the left or right edge of the workspace.

  Open: full content at `width`; with `resizable`, a drag/keyboard resize
  handle on its inner edge. Collapsed: a slim rail (rendered by the caller)
  that reopens the panel. The content keeps its width while the panel
  animates, so nothing reflows mid-transition. The command log docks on the
  right (resizable), the printer state on the left (fixed width).
-->
<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    id,
    label,
    side = "right",
    resizable = true,
    resizeLabel = "",
    open,
    width = $bindable(),
    minWidth,
    maxWidth,
    rail,
    children,
  }: {
    id: string;
    /** Accessible name of the panel region. */
    label: string;
    side?: "left" | "right";
    resizable?: boolean;
    resizeLabel?: string;
    open: boolean;
    width: number;
    minWidth: number;
    maxWidth: number;
    rail: Snippet;
    children: Snippet;
  } = $props();

  const effective = $derived(Math.round(Math.max(minWidth, Math.min(width, maxWidth))));
  let dragging = $state(false);

  function setWidth(next: number): void {
    width = Math.round(Math.max(minWidth, Math.min(next, maxWidth)));
  }

  function onPointerDown(event: PointerEvent): void {
    if (event.button !== 0) return;
    const handle = event.currentTarget as HTMLElement;
    const startX = event.clientX;
    const startWidth = effective;
    dragging = true;
    handle.setPointerCapture(event.pointerId);

    const sign = side === "right" ? 1 : -1;
    const move = (e: PointerEvent): void => setWidth(startWidth + sign * (startX - e.clientX));
    const end = (): void => {
      dragging = false;
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", end);
      handle.removeEventListener("pointercancel", end);
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", end);
    handle.addEventListener("pointercancel", end);
    event.preventDefault();
  }

  function onKeyDown(event: KeyboardEvent): void {
    const step = (event.shiftKey ? 64 : 16) * (side === "right" ? 1 : -1);
    if (event.key === "ArrowLeft") setWidth(effective + step);
    else if (event.key === "ArrowRight") setWidth(effective - step);
    else if (event.key === "Home") setWidth(maxWidth);
    else if (event.key === "End") setWidth(minWidth);
    else return;
    event.preventDefault();
  }
</script>

<aside
  {id}
  class="panel"
  data-side={side}
  class:open
  class:dragging
  aria-label={label}
  style:width={open ? `${effective}px` : "var(--rail-w)"}
>
  {#if open && resizable}
    <!-- A focusable separator is an interactive ARIA widget (window splitter). -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
    <div
      class="resizer"
      role="separator"
      aria-orientation="vertical"
      aria-label={resizeLabel}
      aria-valuenow={effective}
      aria-valuemin={minWidth}
      aria-valuemax={maxWidth}
      tabindex="0"
      onpointerdown={onPointerDown}
      onkeydown={onKeyDown}
    ></div>
  {/if}

  <div class="rail" inert={open}>
    {@render rail()}
  </div>

  <div class="content" style:width="{effective}px" inert={!open}>
    {@render children()}
  </div>
</aside>

<style>
  .panel {
    position: relative;
    flex: none;
    height: 100%;
    overflow: hidden;
    border-left: 1px solid var(--border);
    background: var(--bg-surface);
    transition: width var(--dur-slow) var(--ease-in-out);
  }
  .panel[data-side="left"] {
    border-left: 0;
    border-right: 1px solid var(--border);
  }
  .panel.dragging {
    transition: none;
    user-select: none;
  }

  .rail,
  .content {
    position: absolute;
    top: 0;
    bottom: 0;
    transition:
      opacity var(--dur-base) var(--ease-out),
      visibility 0s linear var(--dur-base);
  }
  .rail {
    left: 0;
    width: var(--rail-w);
  }
  .content {
    left: 0;
    display: flex;
    flex-direction: column;
  }

  .panel:not(.open) .content,
  .panel.open .rail {
    opacity: 0;
    visibility: hidden;
  }
  .panel.open .content,
  .panel:not(.open) .rail {
    transition-delay: 0s;
  }

  .resizer {
    position: absolute;
    z-index: 5;
    top: 0;
    bottom: 0;
    left: 0;
    width: 6px;
    cursor: col-resize;
    touch-action: none;
  }
  [data-side="left"] .resizer {
    left: auto;
    right: 0;
  }
  [data-side="left"] .resizer::after {
    left: auto;
    right: 0;
  }
  .resizer::after {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    width: 2px;
    background: transparent;
    transition: background-color var(--dur-fast) var(--ease-out);
  }
  .resizer:hover::after,
  .dragging .resizer::after {
    background: var(--accent);
  }
  .resizer:focus-visible {
    outline: none;
  }
  .resizer:focus-visible::after {
    width: 2px;
    background: var(--focus-ring);
  }
</style>
