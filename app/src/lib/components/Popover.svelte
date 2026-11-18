<!--
  Small floating card anchored to a trigger button: the header's address list
  and each receipt's save menu. It is moved to <body> and positioned with
  `position: fixed`, so clipping containers (the receipt caption, the roll's
  scroller) and transformed ancestors do not affect it. Opens below the
  anchor (above when there is no room), aligned to its start or end edge and
  kept inside the window.

  Dismissal: Esc (focus returns to the anchor), a pointer press outside,
  focus leaving it, the window resizing, or the anchor scrolling out of
  view. The first focusable element inside receives focus on open, so it is
  fully usable from the keyboard. Enters with a short fade and lift (none
  under reduced motion).
-->
<script lang="ts">
  import { onMount, type Snippet } from "svelte";
  import type { TransitionConfig } from "svelte/transition";
  import { duration, easeOut } from "../motion";

  let {
    anchor,
    align = "start",
    label,
    role = "dialog",
    onClose,
    children,
  }: {
    anchor: HTMLElement;
    align?: "start" | "end";
    /** Accessible name. */
    label: string;
    role?: "dialog" | "menu";
    onClose: (reason: "escape" | "outside" | "blur" | "layout") => void;
    children: Snippet;
  } = $props();

  const GAP = 6;
  const MARGIN = 8;

  let card: HTMLElement | undefined = $state();
  let position = $state<{ left: number; top: number; above: boolean } | null>(null);

  function portal(node: HTMLElement): { destroy(): void } {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  function place(): void {
    if (!card) return;
    const a = anchor.getBoundingClientRect();
    if (a.bottom < 0 || a.top > window.innerHeight || (a.width === 0 && a.height === 0)) {
      onClose("layout");
      return;
    }
    const w = card.offsetWidth;
    const h = card.offsetHeight;
    let left = align === "end" ? a.right - w : a.left;
    left = Math.max(MARGIN, Math.min(left, window.innerWidth - w - MARGIN));
    const below = a.bottom + GAP;
    const above = below + h > window.innerHeight - MARGIN && a.top - GAP - h >= MARGIN;
    position = { left, top: above ? a.top - GAP - h : below, above };
  }

  function popIn(_node: HTMLElement): TransitionConfig {
    const up = position?.above ? 1 : -1;
    return {
      duration: duration("fast"),
      easing: easeOut,
      css: (t) => `opacity: ${t}; transform: translateY(${(1 - t) * 4 * up}px) scale(${0.98 + t * 0.02});`,
    };
  }

  onMount(() => {
    place();
    const first = card?.querySelector<HTMLElement>(
      '[role="menuitem"]:not([disabled]), button:not([disabled]), [href], input, [tabindex]:not([tabindex="-1"])',
    );
    first?.focus({ preventScroll: true });

    const onPointerDown = (event: PointerEvent): void => {
      const target = event.target as Node;
      if (card?.contains(target) || anchor.contains(target)) return;
      onClose("outside");
    };
    const onScroll = (): void => place();
    const onResize = (): void => onClose("layout");
    document.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", onResize);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", onResize);
    };
  });

  function onKeyDown(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      onClose("escape");
      anchor.focus();
    }
  }

  function onFocusOut(event: FocusEvent): void {
    const next = event.relatedTarget as Node | null;
    if (next && (card?.contains(next) || anchor.contains(next))) return;
    // Focus moving to nothing (a click on an inert spot) is handled by the
    // pointer listener; only real focus moves close the card here.
    if (next) onClose("blur");
  }
</script>

<div
  class="popover"
  bind:this={card}
  use:portal
  {role}
  aria-label={label}
  tabindex="-1"
  style:left="{position?.left ?? -9999}px"
  style:top="{position?.top ?? -9999}px"
  onkeydown={onKeyDown}
  onfocusout={onFocusOut}
  in:popIn
>
  {@render children()}
</div>

<style>
  .popover {
    position: fixed;
    z-index: 50;
    min-width: 200px;
    max-width: min(360px, calc(100vw - 16px));
    padding: var(--space-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    box-shadow: var(--shadow-pop);
    color: var(--text);
    font-family: var(--font-ui);
    font-size: var(--text-md);
    line-height: var(--leading);
  }
  .popover:focus {
    outline: none;
  }
</style>
