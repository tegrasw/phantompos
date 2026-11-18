<!--
  Small front view of the simulated printer that mirrors its state: a slip
  feeding out while it can print, the cover tilting open (showing the roll, or
  the empty spindle when the paper is out) and a status LED: teal when ready,
  --fault (blinking, like a real error LED) while any fault is simulated.
  Purely decorative; the panel states the same in text.
-->
<script lang="ts">
  let {
    paperOut,
    coverOpen,
    offline,
    printing,
  }: { paperOut: boolean; coverOpen: boolean; offline: boolean; printing: boolean } = $props();

  const fault = $derived(paperOut || coverOpen || offline);
</script>

<svg
  class="art"
  viewBox="0 0 220 112"
  aria-hidden="true"
  data-fault={fault}
  data-cover={coverOpen ? "open" : "closed"}
  data-paper={paperOut ? "out" : "in"}
  data-printing={printing && !fault}
>
  <!-- Desk shadow. -->
  <ellipse class="shadow" cx="110" cy="104" rx="88" ry="5" />

  <!-- Slip leaving the slot (behind the cover). -->
  <g class="slip">
    <path
      class="paper"
      d="M70 46V12l5.5 3.5 5.5-3.5 5.5 3.5 5.5-3.5 5.5 3.5 5.5-3.5 5.5 3.5 5.5-3.5 5.5 3.5 5.5-3.5 5.5 3.5 5.5-3.5 5.5 3.5 5.5-3.5V46z"
    />
    <rect class="ink" x="88" y="22" width="44" height="3" rx="1.5" />
    <rect class="ink faint" x="80" y="30" width="34" height="2.5" rx="1.25" />
    <rect class="ink faint" x="124" y="30" width="16" height="2.5" rx="1.25" />
    <rect class="ink faint" x="80" y="37" width="28" height="2.5" rx="1.25" />
    <rect class="ink faint" x="124" y="37" width="16" height="2.5" rx="1.25" />
  </g>

  <!-- Paper compartment, visible while the cover is open. -->
  <g class="bay">
    <rect class="cavity" x="34" y="42" width="152" height="20" rx="6" />
    <rect class="spindle" x="44" y="50.5" width="132" height="3" rx="1.5" />
    <rect class="roll" x="50" y="44" width="120" height="16" rx="8" />
    <path class="roll-edge" d="M150 44.5a8 7.5 0 0 1 0 15" />
  </g>

  <!-- Body. -->
  <rect class="body" x="22" y="54" width="176" height="46" rx="11" />
  <rect class="vent" x="40" y="80" width="34" height="3" rx="1.5" />
  <rect class="vent" x="40" y="87" width="22" height="3" rx="1.5" />
  <rect class="button" x="140" y="77" width="18" height="8" rx="4" />
  <circle class="led" cx="176" cy="81" r="3.5" />

  <!-- Cover, hinged at the back left. -->
  <g class="cover">
    <rect class="lid" x="22" y="36" width="176" height="24" rx="11" />
    <rect class="slot" x="62" y="38.5" width="96" height="3" rx="1.5" />
    <rect class="lid-shine" x="34" y="45" width="152" height="1.5" rx="0.75" />
  </g>
</svg>

<style>
  .art {
    display: block;
    width: 100%;
    max-width: 196px;
    height: auto;
    margin: 0 auto;
    overflow: visible;
  }

  .shadow {
    fill: var(--text);
    opacity: 0.07;
  }
  .body,
  .lid {
    fill: var(--bg-raised);
    stroke: var(--border-strong);
    stroke-width: 1;
  }
  .lid {
    fill: color-mix(in srgb, var(--bg-raised), var(--text) 3%);
  }
  .lid-shine {
    fill: var(--border-subtle);
  }
  .slot,
  .cavity {
    fill: var(--slot);
  }
  .slot {
    opacity: 0.85;
  }
  .vent {
    fill: var(--border);
  }
  .button {
    fill: var(--bg-sunken);
    stroke: var(--border-strong);
    stroke-width: 1;
  }
  .spindle {
    fill: var(--text-tertiary);
  }
  .roll {
    fill: var(--paper);
    stroke: var(--paper-edge);
    stroke-width: 1;
  }
  .roll-edge {
    fill: none;
    stroke: var(--paper-edge);
    stroke-width: 1;
  }
  .paper {
    fill: var(--paper);
    stroke: var(--paper-edge);
    stroke-width: 1;
  }
  .ink {
    fill: var(--paper-ink);
    opacity: 0.75;
  }
  .ink.faint {
    opacity: 0.3;
  }

  /* LED --------------------------------------------------------------------- */
  .led {
    fill: var(--accent);
    filter: drop-shadow(0 0 3px var(--accent-glow));
    transition: fill var(--dur-base) var(--ease-out);
  }
  [data-fault="true"] .led {
    fill: var(--fault);
    filter: drop-shadow(0 0 3px var(--fault-glow));
    animation: blink 1.6s steps(1, end) infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0.3;
    }
  }

  /* Cover ------------------------------------------------------------------- */
  .cover {
    transform-box: view-box;
    transform-origin: 26px 48px;
    transition: transform var(--dur-slow) var(--ease-out);
  }
  [data-cover="open"] .cover {
    transform: translateY(-6px) rotate(-13deg);
  }
  .bay {
    opacity: 0;
    transition: opacity var(--dur-base) var(--ease-out);
  }
  [data-cover="open"] .bay {
    opacity: 1;
  }
  .roll,
  .roll-edge {
    transition: opacity var(--dur-base) var(--ease-out);
  }
  [data-paper="out"] .roll,
  [data-paper="out"] .roll-edge {
    opacity: 0;
  }

  /* Slip: out while the printer can print, pulled in otherwise. -------------- */
  .slip {
    transition:
      transform var(--dur-slow) var(--ease-out),
      opacity var(--dur-base) var(--ease-out);
  }
  [data-paper="out"] .slip,
  [data-cover="open"] .slip {
    transform: translateY(30px);
    opacity: 0;
  }
  [data-printing="true"] .slip {
    animation: feed 0.9s var(--ease-in-out) infinite alternate;
  }
  @keyframes feed {
    to {
      transform: translateY(-3px);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    [data-fault="true"] .led,
    [data-printing="true"] .slip {
      animation: none;
    }
  }
</style>
