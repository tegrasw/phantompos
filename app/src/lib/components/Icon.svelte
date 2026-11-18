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
    | "panelLeft"
    | "printer"
    | "pause"
    | "chevronLeft"
    | "chevronRight"
    | "arrowDown"
    | "alert"
    | "terminal"
    | "list"
    | "plug"
    | "x"
    | "drawer"
    | "drawerOpen"
    | "receipt"
    | "zoom"
    | "scan";
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
  {:else if name === "panelLeft"}
    <rect x="1.75" y="2.75" width="12.5" height="10.5" rx="1.75" />
    <path d="M6.25 2.75v10.5" />
  {:else if name === "printer"}
    <!-- Thermal printer seen from the front: body, slot, paper leaving it. -->
    <path d="M4.25 6.25V2.75h7.5v3.5" />
    <rect x="1.75" y="6.25" width="12.5" height="5.5" rx="1.5" />
    <path d="M4.75 11.75v1.5h6.5v-1.5M11.5 8.75h.5" />
  {:else if name === "pause"}
    <path d="M5.75 3.75v8.5M10.25 3.75v8.5" />
  {:else if name === "chevronLeft"}
    <path d="M10 3.5L5.5 8l4.5 4.5" />
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
  {:else if name === "drawer"}
    <!-- Cash drawer, closed: cabinet with a flush drawer front. -->
    <rect x="1.75" y="4.25" width="12.5" height="9" rx="1.5" />
    <path d="M1.75 8.25h12.5M6.5 10.75h3" />
  {:else if name === "drawerOpen"}
    <!-- Cash drawer, open: the front slid out below the cabinet. -->
    <path d="M3.25 7.25V3.75A1.5 1.5 0 0 1 4.75 2.25h6.5a1.5 1.5 0 0 1 1.5 1.5v3.5" />
    <rect x="1.25" y="7.25" width="13.5" height="6.5" rx="1.5" />
    <path d="M6.25 10.5h3.5" />
  {:else if name === "zoom"}
    <!-- Magnifier with a plus: look closer. -->
    <circle cx="7" cy="7" r="4.25" />
    <path d="M10.25 10.25l3 3M7 5.25v3.5M5.25 7h3.5" />
  {:else if name === "scan"}
    <!-- Viewfinder corners around a code. -->
    <path d="M2.25 5.25v-1.5a1.5 1.5 0 0 1 1.5-1.5h1.5M10.75 2.25h1.5a1.5 1.5 0 0 1 1.5 1.5v1.5M13.75 10.75v1.5a1.5 1.5 0 0 1-1.5 1.5h-1.5M5.25 13.75h-1.5a1.5 1.5 0 0 1-1.5-1.5v-1.5" />
    <path d="M5.25 5.25h2v2h-2zM8.75 8.75h2v2h-2zM8.75 5.25h2M5.25 10.75h2" />
  {:else if name === "receipt"}
    <path d="M3.75 2.25h8.5v11.5l-1.4-1-1.42 1-1.43-1-1.42 1-1.43-1-1.4 1z" />
    <path d="M6 5.5h4M6 8h4" />
  {/if}
</svg>

<style>
  .icon {
    display: block;
    flex: none;
  }
</style>
