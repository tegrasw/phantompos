<!--
  Small inline icon set, drawn on a 16×16 grid with 1.5px strokes in
  currentColor. Decorative by default (aria-hidden); pass `label` when the icon
  carries meaning on its own.
-->
<script lang="ts" module>
  export type IconName =
    | "copy"
    | "check"
    | "trash"
    | "panel"
    | "chevronRight"
    | "arrowDown"
    | "alert"
    | "terminal"
    | "list"
    | "plug"
    | "x";
</script>

<script lang="ts">
  let {
    name,
    size = 16,
    label,
  }: { name: IconName; size?: number; label?: string } = $props();
</script>

<svg
  class="icon"
  width={size}
  height={size}
  viewBox="0 0 16 16"
  fill="none"
  stroke="currentColor"
  stroke-width="1.5"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden={label ? undefined : "true"}
  role={label ? "img" : undefined}
  aria-label={label}
>
  {#if name === "copy"}
    <rect x="5.25" y="5.25" width="8" height="8" rx="1.75" />
    <path d="M10.75 5.25V4a1.25 1.25 0 0 0-1.25-1.25H4A1.25 1.25 0 0 0 2.75 4v5.5A1.25 1.25 0 0 0 4 10.75h1.25" />
  {:else if name === "check"}
    <path d="M3.25 8.5l3 3 6.5-7" />
  {:else if name === "trash"}
    <path d="M2.75 4.25h10.5M6.25 4.25V3a.75.75 0 0 1 .75-.75h2a.75.75 0 0 1 .75.75v1.25" />
    <path d="M4 4.25l.6 8.4a1.25 1.25 0 0 0 1.25 1.1h4.3a1.25 1.25 0 0 0 1.25-1.1l.6-8.4" />
    <path d="M6.75 7v4M9.25 7v4" />
  {:else if name === "panel"}
    <rect x="1.75" y="2.75" width="12.5" height="10.5" rx="1.75" />
    <path d="M9.75 2.75v10.5" />
  {:else if name === "chevronRight"}
    <path d="M6 3.5l4.5 4.5L6 12.5" />
  {:else if name === "arrowDown"}
    <path d="M8 2.75v10M3.75 8.75 8 13l4.25-4.25" />
  {:else if name === "alert"}
    <path d="M7.13 2.6a1 1 0 0 1 1.74 0l5.3 9.4a1 1 0 0 1-.87 1.5H2.7a1 1 0 0 1-.87-1.5z" />
    <path d="M8 6.25v3" />
    <path d="M8 11.4v.1" stroke-width="1.75" />
  {:else if name === "terminal"}
    <rect x="1.75" y="2.75" width="12.5" height="10.5" rx="1.75" />
    <path d="M4.75 6.25 6.75 8l-2 1.75M8.25 10.25h3" />
  {:else if name === "list"}
    <path d="M6 4.25h7.25M6 8h7.25M6 11.75h7.25" />
    <path d="M2.75 4.25h.5M2.75 8h.5M2.75 11.75h.5" stroke-width="1.75" />
  {:else if name === "plug"}
    <path d="M5.75 2.25v2.5M10.25 2.25v2.5" />
    <path d="M3.75 4.75h8.5v2.5a4.25 4.25 0 0 1-8.5 0z" />
    <path d="M8 11.5v2.25" />
  {:else if name === "x"}
    <path d="M4 4l8 8M12 4l-8 8" />
  {/if}
</svg>

<style>
  .icon {
    display: block;
    flex: none;
  }
</style>
