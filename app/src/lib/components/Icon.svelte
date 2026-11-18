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
    | "scan"
    | "gear"
    | "globe"
    | "image"
    | "fileBin"
    | "more"
    | "refresh"
    | "shield"
    | "download"
    | "laptop";
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
  {:else if name === "gear"}
    <!-- Eight-tooth cog around an axle. -->
    <path d="M6.61 3.30L6.89 1.70L9.11 1.70L9.39 3.30A4.9 4.9 0 0 1 10.34 3.69L11.67 2.76L13.24 4.33L12.31 5.66A4.9 4.9 0 0 1 12.70 6.61L14.30 6.89L14.30 9.11L12.70 9.39A4.9 4.9 0 0 1 12.31 10.34L13.24 11.67L11.67 13.24L10.34 12.31A4.9 4.9 0 0 1 9.39 12.70L9.11 14.30L6.89 14.30L6.61 12.70A4.9 4.9 0 0 1 5.66 12.31L4.33 13.24L2.76 11.67L3.69 10.34A4.9 4.9 0 0 1 3.30 9.39L1.70 9.11L1.70 6.89L3.30 6.61A4.9 4.9 0 0 1 3.69 5.66L2.76 4.33L4.33 2.76L5.66 3.69A4.9 4.9 0 0 1 6.61 3.30Z" stroke-width="1.35" />
    <circle cx="8" cy="8" r="2" />
  {:else if name === "globe"}
    <!-- Network mode: reachable from other machines. -->
    <circle cx="8" cy="8" r="6.25" />
    <ellipse cx="8" cy="8" rx="2.6" ry="6.25" />
    <path d="M1.75 8h12.5" />
  {:else if name === "laptop"}
    <!-- Local only: this computer. -->
    <rect x="3" y="3.25" width="10" height="7" rx="1.25" />
    <path d="M1.5 12.75h13" />
  {:else if name === "image"}
    <rect x="2.25" y="2.75" width="11.5" height="10.5" rx="1.75" />
    <circle cx="5.9" cy="6.3" r="1.15" />
    <path d="M13.75 10.25 10.6 7.1l-6.1 6.15" />
  {:else if name === "fileBin"}
    <!-- A file holding raw bytes: "1 0". -->
    <path d="M4.25 1.75h4.9l3.1 3.1v8.4a1 1 0 0 1-1 1h-7a1 1 0 0 1-1-1V2.75a1 1 0 0 1 1-1z" />
    <path d="M9 1.75v3.25h3.25" />
    <path d="M5.75 8.5l.75-.5v4" />
    <rect x="8.4" y="8" width="2.1" height="4" rx="1.05" />
  {:else if name === "more"}
    <path d="M3.75 8h.01M8 8h.01M12.25 8h.01" stroke-width="2.4" />
  {:else if name === "refresh"}
    <path d="M13.25 8a5.25 5.25 0 1 1-1.54-3.71" />
    <path d="M13.25 2.5v2.75H10.5" />
  {:else if name === "shield"}
    <path d="M8 1.75l5.25 2v3.9c0 3.05-2.15 5.4-5.25 6.6-3.1-1.2-5.25-3.55-5.25-6.6v-3.9z" />
    <path d="M8 5.25v3" />
    <path d="M8 10.6v.1" stroke-width="1.75" />
  {:else if name === "download"}
    <path d="M8 2.25v7.75M4.75 6.75 8 10l3.25-3.25" />
    <path d="M2.75 11.25v1a1.25 1.25 0 0 0 1.25 1.25h8a1.25 1.25 0 0 0 1.25-1.25v-1" />
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
