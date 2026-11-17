<!--
  Icon button that copies `text` and confirms in place: the copy glyph turns
  into a check and a small "Copied" bubble appears, then both settle back.
-->
<script lang="ts">
  import { onDestroy } from "svelte";
  import { fly, scale } from "svelte/transition";
  import { copyText } from "../clipboard";
  import { messages } from "../messages";
  import { duration, easeOut } from "../motion";
  import Icon from "./Icon.svelte";

  let {
    text,
    label,
    bubble = "below",
  }: { text: string; label: string; bubble?: "below" | "above" } = $props();

  const t = messages.header;
  let status = $state<"idle" | "copied" | "failed">("idle");
  let timer: ReturnType<typeof setTimeout> | undefined;

  async function copy(): Promise<void> {
    const ok = await copyText(text);
    status = ok ? "copied" : "failed";
    clearTimeout(timer);
    timer = setTimeout(() => (status = "idle"), 1600);
  }

  onDestroy(() => clearTimeout(timer));
</script>

<span class="copy">
  <button
    type="button"
    class="icon-btn"
    class:done={status === "copied"}
    title={label}
    aria-label={label}
    onclick={copy}
  >
    {#key status === "copied"}
      <span class="glyph" in:scale={{ start: 0.5, duration: duration("base"), easing: easeOut }}>
        <Icon name={status === "copied" ? "check" : "copy"} size={14} />
      </span>
    {/key}
  </button>
  <span class="visually-hidden" aria-live="polite">
    {status === "copied" ? t.copied : status === "failed" ? t.copyFailed : ""}
  </span>
  {#if status !== "idle"}
    <span
      class="bubble {bubble}"
      class:failed={status === "failed"}
      aria-hidden="true"
      transition:fly={{ y: bubble === "below" ? -4 : 4, duration: duration("fast"), easing: easeOut }}
    >
      {status === "copied" ? t.copied : t.copyFailed}
    </span>
  {/if}
</span>

<style>
  .copy {
    position: relative;
    display: inline-flex;
  }
  .icon-btn {
    width: 24px;
    height: 24px;
  }
  .glyph {
    display: grid;
    place-items: center;
  }
  .done,
  .done:hover:not(:disabled) {
    color: var(--accent);
  }
  .bubble {
    position: absolute;
    left: 50%;
    translate: -50% 0;
    z-index: 20;
    padding: var(--space-0-5) var(--space-2);
    border-radius: var(--radius-sm);
    background: var(--text);
    color: var(--bg-surface);
    font-size: var(--text-xs);
    font-weight: var(--weight-medium);
    white-space: nowrap;
    pointer-events: none;
    box-shadow: var(--shadow-md);
  }
  .bubble.below {
    top: calc(100% + 6px);
  }
  .bubble.above {
    bottom: calc(100% + 6px);
  }
  .bubble.failed {
    background: var(--danger);
    color: #fff;
  }
</style>
