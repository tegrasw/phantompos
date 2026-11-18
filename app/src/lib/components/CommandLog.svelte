<!--
  Decoded command log: offset, hex bytes and decoded command side by side,
  grouped by connection. Follows new output only while the user is at the
  bottom; scrolling up detaches and offers "Jump to latest".
-->
<script lang="ts">
  import { SvelteSet } from "svelte/reactivity";
  import { fade, fly } from "svelte/transition";
  import { hexLines, type CommandLog, type LogRow } from "../commandLog.svelte";
  import { formatBytes, formatCount, formatOffset, formatTime } from "../format";
  import { messages } from "../messages";
  import { duration, easeOut } from "../motion";
  import Icon from "./Icon.svelte";

  let {
    log,
    compact = false,
    onCollapse,
  }: {
    log: CommandLog;
    /** Narrow panel: bytes stack under the decoded command instead of beside it. */
    compact?: boolean;
    onCollapse: () => void;
  } = $props();

  const t = messages.log;
  const STICKY_THRESHOLD_PX = 24;
  /** Height of one hex line plus row padding; keep in sync with the CSS. */
  const LINE_PX = 18;
  const ROW_PAD_PX = 8;

  let scroller: HTMLElement | undefined = $state();
  let follow = $state(true);
  /** `log.ingested` when the user scrolled away from the tail. */
  let detachedAt = $state(0);
  const expanded = new SvelteSet<number>();

  const newSinceDetach = $derived(follow ? 0 : Math.max(0, log.ingested - detachedAt));
  const empty = $derived(log.rowCount === 0);

  $effect(() => {
    // Effects run after the DOM update, so scrollHeight includes new rows.
    void log.groups;
    if (follow && scroller) scroller.scrollTop = scroller.scrollHeight;
  });

  function detach(): void {
    if (!follow) return;
    follow = false;
    detachedAt = log.ingested;
  }

  function onScroll(): void {
    if (!scroller) return;
    const distance = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight;
    if (distance <= STICKY_THRESHOLD_PX) follow = true;
    else detach();
  }

  // Detach immediately on user intent, before the next flush can pull the
  // view back down (the scroll event alone arrives too late during bursts).
  function onWheel(event: WheelEvent): void {
    if (event.deltaY < 0) detach();
  }
  function onKeyDown(event: KeyboardEvent): void {
    if (["ArrowUp", "PageUp", "Home"].includes(event.key)) detach();
  }
  function onPointerDown(event: PointerEvent): void {
    // Pointer down on the scroller itself means its scrollbar.
    if (event.target === scroller) detach();
  }

  function jumpToLatest(): void {
    follow = true;
    scroller?.scrollTo({ top: scroller.scrollHeight, behavior: duration("base") ? "smooth" : "auto" });
  }

  function clear(): void {
    expanded.clear();
    log.clear();
    follow = true;
  }

  function toggleExpanded(key: number): void {
    if (expanded.has(key)) expanded.delete(key);
    else expanded.add(key);
  }

  function rowLines(row: LogRow): string[] {
    return expanded.has(row.key) ? hexLines(row.bytes) : row.hexPreview;
  }

  function intrinsicHeight(row: LogRow): string {
    const lines = Math.max(1, row.hexPreview.length) + (row.hiddenBytes > 0 ? 1 : 0);
    return `auto ${lines * LINE_PX + ROW_PAD_PX}px`;
  }
</script>

<div class="log" class:compact>
  <div class="toolbar">
    <h2 class="title">{t.title}</h2>
    {#if !empty}
      <span class="count" in:fade={{ duration: duration("fast") }}>{formatCount(log.rowCount)}</span>
    {/if}
    {#if log.retainedUnknown > 0}
      <span class="chip-warn" title={t.unknownCount(log.retainedUnknown)} in:fade={{ duration: duration("fast") }}>
        <Icon name="alert" size={12} />
        {t.unknownCount(log.retainedUnknown)}
      </span>
    {/if}
    <span class="spacer"></span>
    <button
      type="button"
      class="icon-btn"
      title={t.clear}
      aria-label={t.clear}
      disabled={empty}
      onclick={clear}
    >
      <Icon name="trash" />
    </button>
    <button type="button" class="icon-btn" title={t.collapse} aria-label={t.collapse} onclick={onCollapse}>
      <Icon name="chevronRight" />
    </button>
  </div>

  {#if !empty}
    <div class="columns" aria-hidden="true">
      <span>{t.columnOffset}</span>
      {#if compact}
        <span>{t.columnCommand} · {t.columnBytes}</span>
      {:else}
        <span>{t.columnBytes}</span>
        <span>{t.columnCommand}</span>
      {/if}
    </div>
  {/if}

  <!-- The listeners only observe scroll intent; the log stays a focusable,
       keyboard-scrollable region. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    class="scroll"
    bind:this={scroller}
    onscroll={onScroll}
    onwheel={onWheel}
    onkeydown={onKeyDown}
    onpointerdown={onPointerDown}
    tabindex="0"
    role="log"
    aria-label={t.title}
  >
    {#if empty}
      <div class="empty" in:fade={{ duration: duration("base") }}>
        <span class="empty-icon"><Icon name="list" size={18} /></span>
        <p class="empty-title">{t.emptyTitle}</p>
        <p class="empty-body">{t.emptyBody}</p>
      </div>
    {:else}
      {#if log.dropped > 0}
        <p class="truncated">{t.truncated(log.dropped)}</p>
      {/if}
      {#each log.groups as group (group.key)}
        <section class="group">
          <header class="group-head" title={t.connectionTitle(group.connectionId, group.peer)}>
            <span class="conn">{t.connection(group.connectionId)}</span>
            <span class="peer">{group.peer}</span>
            {#if group.unknownCount > 0}
              <span class="chip-warn small" title={t.unknownCount(group.unknownCount)}>
                <Icon name="alert" size={10} />{formatCount(group.unknownCount)}
              </span>
            {/if}
            <span class="spacer"></span>
            <span class="summary" title={t.groupSummary(group.rows.length, formatBytes(group.byteCount))}>
              {formatBytes(group.byteCount)}
            </span>
            <time class="time">{formatTime(group.startedAt)}</time>
          </header>

          {#each group.rows as row (row.key)}
            <div
              class="row"
              class:enter={row.enter}
              class:unknown={row.kind === "unknown"}
              style:contain-intrinsic-size={intrinsicHeight(row)}
            >
              <span class="offset">{formatOffset(row.offset)}</span>

              <span class="hex">
                {#each rowLines(row) as line, i (i)}
                  <span class="hex-line">{line}</span>
                {/each}
                {#if expanded.has(row.key) && row.byteLength > row.bytes.length}
                  <span class="hex-rest" title={t.notRetainedTitle}>{t.notRetained(row.byteLength - row.bytes.length)}</span>
                {/if}
                {#if row.hiddenBytes > 0}
                  <button type="button" class="more" onclick={() => toggleExpanded(row.key)}>
                    {expanded.has(row.key) ? t.lessBytes : t.moreBytes(row.hiddenBytes)}
                  </button>
                {/if}
              </span>

              <span class="decoded">
                {#if row.kind === "text"}
                  <span class="mn mn-muted">{t.textLabel}</span>
                  <span class="text" title={t.textTitle(row.charCount)}
                    >{#each row.segments as segment, i (i)}{#if segment.kind === "text"}{segment.value}{:else if segment.kind === "space"}<span
                          class="ws"
                          title={t.spaceTitle(segment.count)}>{"·".repeat(segment.count)}</span
                        >{:else}<span class="ctl" title={t.controlTitle(segment.name)}>{segment.name}</span
                        >{/if}{/each}</span
                  >
                {:else if row.kind === "unknown"}
                  <span class="mn">{row.mnemonic ?? "?"}</span>
                  <span class="body">
                    <span class="name">
                      <Icon name="alert" size={12} />
                      {t.unknownLabel}
                    </span>
                    {#if row.reason}<span class="reason">{row.reason}</span>{/if}
                    {#each row.params as param (param.key)}
                      <span class="param"><span class="k">{param.key}</span> <span class="v">{param.value}</span></span>
                    {/each}
                  </span>
                {:else}
                  <span class="mn">{row.mnemonic ?? ""}</span>
                  <span class="body">
                    <span class="name">{row.label}</span>
                    {#if row.summary}<span class="gist">{row.summary}</span>{/if}
                    {#each row.params as param (param.key)}
                      <span class="param"><span class="k">{param.key}</span> <span class="v">{param.value}</span></span>
                    {/each}
                  </span>
                {/if}
              </span>
            </div>
          {/each}
        </section>
      {/each}
    {/if}
  </div>

  {#if !follow && !empty}
    <button
      type="button"
      class="jump"
      onclick={jumpToLatest}
      transition:fly={{ y: 8, duration: duration("base"), easing: easeOut }}
    >
      <Icon name="arrowDown" size={14} />
      {newSinceDetach > 0 ? t.newRows(newSinceDetach) : t.jumpToLatest}
    </button>
  {/if}
</div>

<style>
  .log {
    --col-offset: 6.5ch;
    --col-hex: 24.5ch;
    --col-gap: var(--space-3);
    --pad-x: var(--space-4);

    position: relative;
    display: flex;
    flex-direction: column;
    height: 100%;
    min-width: 0;
  }

  /* Toolbar -------------------------------------------------------------- */
  .toolbar {
    flex: none;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    height: 44px;
    padding: 0 var(--space-2) 0 var(--pad-x);
    border-bottom: 1px solid var(--border-subtle);
  }
  .title {
    margin: 0;
    font-size: var(--text-md);
    font-weight: var(--weight-semibold);
  }
  .count {
    padding: 0 var(--space-1-5);
    border-radius: var(--radius-pill);
    background: var(--bg-active);
    color: var(--text-secondary);
    font-size: var(--text-xs);
    font-weight: var(--weight-medium);
    line-height: 18px;
    font-variant-numeric: tabular-nums;
  }
  .chip-warn {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    padding: 0 var(--space-1-5);
    border: 1px solid var(--warning-border);
    border-radius: var(--radius-pill);
    background: var(--warning-soft);
    color: var(--warning);
    font-size: var(--text-xs);
    font-weight: var(--weight-medium);
    line-height: 16px;
    white-space: nowrap;
  }
  .chip-warn.small {
    font-size: var(--text-2xs);
    line-height: 14px;
  }
  .spacer {
    flex: 1;
  }

  .columns {
    flex: none;
    display: grid;
    grid-template-columns: var(--col-offset) var(--col-hex) minmax(0, 1fr);
    column-gap: var(--col-gap);
    padding: var(--space-1-5) var(--pad-x);
    border-bottom: 1px solid var(--border);
    color: var(--text-tertiary);
    font-size: var(--text-2xs);
    font-weight: var(--weight-semibold);
    letter-spacing: var(--tracking-caps);
    text-transform: uppercase;
  }
  /* Grid columns are sized in `ch` of the mono font at the row size, so the
     header row uses the same font metrics and restyles only its labels. */
  .columns,
  .row {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
  }
  .columns span {
    font-family: var(--font-ui);
    font-size: var(--text-2xs);
  }

  /* Scroll area ---------------------------------------------------------- */
  .scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    overscroll-behavior: contain;
    padding-bottom: var(--space-8);
  }
  .scroll:focus-visible {
    outline-offset: -2px;
  }

  .truncated {
    margin: 0;
    padding: var(--space-2) var(--pad-x);
    color: var(--text-tertiary);
    font-size: var(--text-xs);
    border-bottom: 1px solid var(--border-subtle);
  }

  .group + .group {
    margin-top: var(--space-2);
  }

  .group-head {
    position: sticky;
    top: 0;
    z-index: 2;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    height: 30px;
    padding: 0 var(--pad-x);
    border-bottom: 1px solid var(--border-subtle);
    background: var(--bg-surface);
    font-size: var(--text-xs);
    color: var(--text-tertiary);
    white-space: nowrap;
  }
  .group + .group .group-head {
    border-top: 1px solid var(--border-subtle);
  }
  .conn {
    padding: 0 var(--space-1-5);
    border-radius: var(--radius-xs);
    background: var(--accent-soft);
    color: var(--accent);
    font-family: var(--font-mono);
    font-weight: 700;
    font-size: var(--text-2xs);
    line-height: 16px;
  }
  .peer {
    flex: 0 1 auto;
    font-family: var(--font-mono);
    color: var(--text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }
  .summary {
    font-variant-numeric: tabular-nums;
  }
  .time {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
  }

  /* Rows ----------------------------------------------------------------- */
  .row {
    display: grid;
    grid-template-columns: var(--col-offset) var(--col-hex) minmax(0, 1fr);
    column-gap: var(--col-gap);
    align-items: start;
    padding: 4px var(--pad-x);
    line-height: 18px;
    font-family: var(--font-mono);
    content-visibility: auto;
    transition: background-color var(--dur-fast) var(--ease-out);
  }
  .row:hover {
    background: var(--bg-hover);
  }
  .row.enter {
    animation: row-in var(--dur-slow) var(--ease-out) both;
  }
  @keyframes row-in {
    from {
      opacity: 0;
      transform: translateY(3px);
      background: var(--accent-soft);
    }
  }

  .offset {
    color: var(--text-tertiary);
    font-variant-numeric: tabular-nums;
    user-select: none;
  }

  .hex {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    color: var(--text-secondary);
    white-space: pre;
  }
  .more {
    margin: 1px 0 0 -4px;
    padding: 0 4px;
    border: 0;
    border-radius: var(--radius-xs);
    background: transparent;
    color: var(--accent);
    font: inherit;
    font-size: var(--text-2xs);
    line-height: 16px;
    cursor: pointer;
  }
  .more:hover {
    background: var(--accent-soft);
  }
  .hex-rest {
    color: var(--text-tertiary);
    font-size: var(--text-2xs);
    line-height: 16px;
  }

  .decoded {
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr);
    column-gap: var(--space-2);
    align-items: baseline;
    min-width: 0;
  }
  .body {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    column-gap: var(--space-2);
    min-width: 0;
  }
  .mn {
    white-space: nowrap;
    color: var(--accent);
    font-weight: 700;
    font-size: var(--text-xs);
  }
  .mn-muted {
    color: var(--text-tertiary);
    font-family: var(--font-ui);
    font-weight: var(--weight-medium);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-caps);
    text-transform: uppercase;
  }
  .name {
    font-family: var(--font-ui);
    font-size: var(--text-md);
    font-weight: var(--weight-medium);
    color: var(--text);
  }
  .name :global(.icon) {
    display: inline-block;
    margin-right: var(--space-0-5);
    vertical-align: -1px;
  }
  .gist {
    padding: 0 var(--space-1-5);
    border: 1px solid var(--border);
    border-radius: var(--radius-xs);
    background: var(--bg-sunken);
    color: var(--text);
    font-size: var(--text-xs);
    font-weight: 700;
    line-height: 16px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
  }
  .param {
    color: var(--text-tertiary);
    font-size: var(--text-xs);
    white-space: nowrap;
  }
  .param .v {
    color: var(--text);
    font-weight: 700;
  }

  .text {
    min-width: 0;
    color: var(--text);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .text::before,
  .text::after {
    content: "\201C";
    color: var(--text-tertiary);
  }
  .text::after {
    content: "\201D";
  }
  .ws {
    color: var(--text-tertiary);
    opacity: 0.7;
  }
  .ctl {
    display: inline-block;
    margin: 0 1px;
    padding: 0 3px;
    border-radius: var(--radius-xs);
    background: var(--bg-active);
    color: var(--text-secondary);
    font-size: var(--text-2xs);
    line-height: 15px;
    vertical-align: 1px;
  }

  .row.unknown {
    background: var(--warning-soft);
    box-shadow: inset 2px 0 0 var(--warning);
  }
  .row.unknown .hex,
  .row.unknown .mn,
  .row.unknown .name {
    color: var(--warning);
  }
  .row.unknown .name {
    font-weight: var(--weight-semibold);
  }
  .reason {
    font-family: var(--font-ui);
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }

  /* Compact (narrow panel) ------------------------------------------------ */
  .compact .columns,
  .compact .row {
    grid-template-columns: var(--col-offset) minmax(0, 1fr);
  }
  .compact .decoded {
    grid-column: 2;
    grid-row: 1;
  }
  .compact .hex {
    grid-column: 2;
    grid-row: 2;
    padding-left: calc(44px + var(--space-2));
    font-size: var(--text-xs);
    line-height: 16px;
    color: var(--text-tertiary);
  }

  /* Empty state ---------------------------------------------------------- */
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    max-width: 30ch;
    margin: 18vh auto 0;
    padding: 0 var(--space-4);
    text-align: center;
  }
  .empty-icon {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    margin-bottom: var(--space-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-sunken);
    color: var(--text-tertiary);
  }
  .empty-title {
    margin: 0 0 var(--space-1);
    font-size: var(--text-md);
    font-weight: var(--weight-semibold);
  }
  .empty-body {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text-secondary);
    line-height: var(--leading);
  }

  /* Jump to latest ------------------------------------------------------- */
  .jump {
    position: absolute;
    left: 50%;
    bottom: var(--space-4);
    translate: -50% 0;
    z-index: 3;
    display: inline-flex;
    align-items: center;
    gap: var(--space-1-5);
    height: 30px;
    padding: 0 var(--space-3) 0 10px;
    border: 1px solid var(--accent-border);
    border-radius: var(--radius-pill);
    background: var(--bg-raised);
    color: var(--accent);
    font-size: var(--text-sm);
    font-weight: var(--weight-medium);
    box-shadow: var(--shadow-pop);
    cursor: pointer;
    white-space: nowrap;
    transition: background-color var(--dur-fast) var(--ease-out);
  }
  .jump:hover {
    background-image: linear-gradient(var(--accent-soft), var(--accent-soft));
  }
</style>
