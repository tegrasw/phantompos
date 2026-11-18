<!--
  One simulated-state switch: a full-width row (name, what it reports) with a
  toggle on the right. The whole row is the control (role="switch"), so Tab
  reaches it and Space or Enter flips it. "On" means the fault is simulated,
  so the on state uses the --fault colour rather than the accent.
-->
<script lang="ts">
  let {
    id,
    checked,
    label,
    hint,
    onToggle,
  }: {
    id: string;
    checked: boolean;
    label: string;
    hint: string;
    onToggle: () => void;
  } = $props();
</script>

<button
  type="button"
  class="switch"
  role="switch"
  aria-checked={checked}
  aria-describedby="{id}-hint"
  onclick={onToggle}
>
  <span class="text">
    <span class="label">{label}</span>
    <span class="hint" id="{id}-hint">{hint}</span>
  </span>
  <span class="track" aria-hidden="true"><span class="knob"></span></span>
</button>

<style>
  .switch {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    width: 100%;
    min-height: 46px;
    padding: var(--space-1-5) var(--space-3);
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    background: transparent;
    text-align: left;
    cursor: pointer;
    transition:
      background-color var(--dur-base) var(--ease-out),
      border-color var(--dur-base) var(--ease-out);
  }
  .switch:hover {
    background: var(--bg-hover);
  }
  .switch:focus-visible {
    outline-offset: -1px;
  }
  .switch[aria-checked="true"] {
    border-color: var(--fault-border);
    background: var(--fault-soft);
  }

  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .label {
    color: var(--text);
    font-size: var(--text-md);
    font-weight: var(--weight-medium);
    line-height: var(--leading-tight);
  }
  [aria-checked="true"] .label {
    color: var(--fault);
    font-weight: var(--weight-semibold);
  }
  .hint {
    margin-top: 2px;
    color: var(--text-tertiary);
    font-size: var(--text-xs);
    line-height: var(--leading-tight);
  }

  .track {
    position: relative;
    flex: none;
    width: 30px;
    height: 18px;
    border-radius: var(--radius-pill);
    background: var(--bg-active);
    box-shadow: inset 0 0 0 1px var(--border-strong);
    transition:
      background-color var(--dur-base) var(--ease-out),
      box-shadow var(--dur-base) var(--ease-out);
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--bg-raised);
    box-shadow:
      0 0 0 0.5px rgb(0 0 0 / 0.12),
      0 1px 2px rgb(0 0 0 / 0.25);
    transition: transform var(--dur-base) var(--ease-out);
  }
  [aria-checked="true"] .track {
    background: var(--fault);
    box-shadow: inset 0 0 0 1px var(--fault), 0 0 10px -2px var(--fault-glow);
  }
  [aria-checked="true"] .knob {
    transform: translateX(12px);
  }
</style>
