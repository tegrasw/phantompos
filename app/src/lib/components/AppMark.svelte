<!--
  PhantomPOS mark: a receipt slip on the accent tile. The torn zigzag edge
  doubles as a ghost's hem — the "phantom" in the name.
-->
<script lang="ts">
  let { size = 24 }: { size?: number } = $props();
</script>

<svg class="mark" width={size} height={size} viewBox="0 0 24 24" aria-hidden="true">
  <rect width="24" height="24" rx="6.5" class="tile" />
  <path
    class="slip"
    d="M7 5.25a.75.75 0 0 1 .75-.75h8.5a.75.75 0 0 1 .75.75V18.5l-1.67-1.3-1.66 1.3-1.67-1.3-1.67 1.3-1.66-1.3L7 18.5z"
  />
  <rect x="9" y="7.5" width="6" height="1.4" rx="0.7" class="ink" />
  <rect x="9" y="10.3" width="4" height="1.4" rx="0.7" class="ink" />
  <rect x="9" y="13.1" width="5" height="1.4" rx="0.7" class="ink" />
</svg>

<style>
  .mark {
    display: block;
    flex: none;
  }
  .tile {
    fill: var(--accent);
  }
  .slip {
    fill: var(--bg-surface);
  }
  .ink {
    fill: var(--accent);
    opacity: 0.85;
  }
</style>
