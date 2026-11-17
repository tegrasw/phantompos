<!--
  PhantomPOS mark: a ghost made of receipt paper coming out of a printer slot.
  Monochrome and drawn in `currentColor` (the accent by default). Same artwork
  as design/icon/mark.svg; keep the two in sync.
-->
<script lang="ts">
  let { size = 24 }: { size?: number } = $props();
</script>

<svg
  class="mark"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="currentColor"
  fill-rule="evenodd"
  aria-hidden="true"
>
  <path d="M4 3.5h16a2 2 0 0 1 0 4H4a2 2 0 0 1 0-4zM5.5 5a.5.5 0 0 0 0 1h13a.5.5 0 0 0 0-1z" />
  <path
    d="M6.5 9.6q0-.6.6-.6h9.8q.6 0 .6.6v10.95q0 .45-.25.08l-.87-1.26q-.255-.37-.51 0L15 20.63q-.25.37-.5 0l-.87-1.26q-.255-.37-.51 0l-.87 1.26q-.25.37-.5 0l-.87-1.26q-.255-.37-.51 0L9.5 20.63q-.25.37-.5 0l-.87-1.26q-.255-.37-.51 0l-.87 1.26q-.25.37-.25-.08zM9.3 12.85v1.9a.85.85 0 0 0 1.7 0v-1.9a.85.85 0 0 0-1.7 0zM13 12.85v1.9a.85.85 0 0 0 1.7 0v-1.9a.85.85 0 0 0-1.7 0z"
  />
</svg>

<style>
  .mark {
    display: block;
    flex: none;
    color: var(--accent);
  }
</style>
