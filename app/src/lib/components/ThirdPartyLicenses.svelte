<!--
  Settings > About > Third-party licenses: every open-source component the
  app ships (Rust crates, npm packages, the font), loaded on demand from the
  generated JSON (see lib/thirdParty.ts).

  - A filter field (name, version, license, kind) with a live count. Esc in
    the field clears it first; the next Esc goes back (handled by the dialog).
  - Each row is a disclosure button showing name, version, kind and the
    license expression; expanding it shows the source address (copyable, not
    a link: the app opens only its own fixed links) and the full text of each
    license that applies.
  - Arrow Up/Down, Home and End move between rows.

  ~340 rows render in a few milliseconds, so the list is neither virtualised
  nor paginated; license texts are only in the DOM while expanded.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import { lookup, i18n } from "../i18n";
  import { duration, easeOut } from "../motion";
  import { errorText } from "../receiptExport";
  import { loadThirdPartyLicenses, type ThirdPartyComponent, type ThirdPartyLicenses } from "../thirdParty";
  import CopyButton from "./CopyButton.svelte";
  import Icon from "./Icon.svelte";

  let { autofocus = false }: { /** Focus the filter field when mounted (the view was just opened). */ autofocus?: boolean } =
    $props();

  const t = $derived(i18n.t.about);

  let filter: HTMLInputElement | undefined = $state();
  let data = $state.raw<ThirdPartyLicenses | null>(null);
  let failure = $state<string | null>(null);
  let query = $state("");
  let expanded = $state.raw<ReadonlySet<string>>(new Set());
  let list: HTMLUListElement | undefined = $state();

  async function load(): Promise<void> {
    failure = null;
    try {
      data = await loadThirdPartyLicenses();
    } catch (error) {
      failure = errorText(error);
    }
  }

  onMount(() => {
    if (autofocus) filter?.focus();
    void load();
  });

  const keyOf = (component: ThirdPartyComponent): string => `${component.kind}:${component.name}@${component.version}`;

  const needle = $derived(query.trim().toLowerCase());
  const shown = $derived.by(() => {
    if (!data) return [];
    if (!needle) return data.components;
    const kinds = t.kinds;
    return data.components.filter((component) =>
      `${component.name} ${component.version} ${component.license} ${component.kind} ${lookup(kinds, component.kind, component.kind)}`
        .toLowerCase()
        .includes(needle),
    );
  });

  function toggle(key: string): void {
    const next = new Set(expanded);
    if (!next.delete(key)) next.add(key);
    expanded = next;
  }

  function onFilterKey(event: KeyboardEvent): void {
    if (event.key === "Escape" && query !== "") {
      // First Esc clears the filter; the next one goes back.
      event.preventDefault();
      event.stopPropagation();
      query = "";
    } else if (event.key === "ArrowDown" && shown.length > 0) {
      event.preventDefault();
      rows()[0]?.focus();
    }
  }

  function rows(): HTMLButtonElement[] {
    return list ? Array.from(list.querySelectorAll<HTMLButtonElement>("button.row")) : [];
  }

  function onListKey(event: KeyboardEvent): void {
    const target = event.target as HTMLElement;
    if (!target.classList.contains("row")) return;
    const all = rows();
    const index = all.indexOf(target as HTMLButtonElement);
    let next = -1;
    if (event.key === "ArrowDown") next = Math.min(index + 1, all.length - 1);
    else if (event.key === "ArrowUp") next = index - 1;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = all.length - 1;
    else return;
    event.preventDefault();
    if (next < 0) filter?.focus();
    else all[next]?.focus();
  }

  function host(url: string): string {
    return url.replace(/^https?:\/\//, "");
  }
</script>

<div class="tp">
  <p class="intro">{t.thirdPartyIntro}</p>

  <div class="toolbar">
    <div class="search">
      <span class="search-icon" aria-hidden="true"><Icon name="search" size={14} /></span>
      <input
        class="input"
        type="text"
        role="searchbox"
        placeholder={t.filter}
        aria-label={t.filterLabel}
        aria-controls="tp-list"
        autocomplete="off"
        spellcheck="false"
        bind:this={filter}
        bind:value={query}
        onkeydown={onFilterKey}
      />
      {#if query}
        <button type="button" class="clear icon-btn" aria-label={t.clearFilter} title={t.clearFilter} onclick={() => (query = "")}>
          <Icon name="x" size={12} />
        </button>
      {/if}
    </div>
    <span class="count" role="status">
      {#if data}{t.count(shown.length, data.components.length)}{/if}
    </span>
  </div>

  {#if failure}
    <div class="state">
      <p><Icon name="alert" size={14} />{t.loadFailed}: {failure}</p>
      <button type="button" class="btn" onclick={load}>{t.retry}</button>
    </div>
  {:else if !data}
    <div class="state" role="status"><span class="spinner" aria-hidden="true"></span>{t.loading}</div>
  {:else if shown.length === 0}
    <div class="state" role="status">{t.noMatches(query.trim())}</div>
  {:else}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <ul class="list" id="tp-list" bind:this={list} onkeydown={onListKey}>
      {#each shown as component (keyOf(component))}
        {@const key = keyOf(component)}
        {@const open = expanded.has(key)}
        {@const panel = `tp-${key.replace(/[^a-z0-9]+/gi, "-")}`}
        <li class:open>
          <button type="button" class="row" aria-expanded={open} aria-controls={panel} onclick={() => toggle(key)}>
            <span class="chevron" aria-hidden="true"><Icon name="chevronRight" size={12} /></span>
            <span class="row-name">
              <span class="name">{component.name}</span>
              <span class="version">{component.version}</span>
              <span class="kind">{lookup(t.kinds, component.kind, component.kind)}</span>
            </span>
            <span class="license">{component.license}</span>
          </button>
          {#if open}
            <div class="panel" id={panel} transition:slide={{ duration: duration("base"), easing: easeOut }}>
              {#if component.url}
                <div class="source">
                  <span class="caps-label">{t.source}</span>
                  <code title={component.url}>{host(component.url)}</code>
                  <CopyButton text={component.url} label={t.copySource(component.name)} bubble="above" />
                </div>
              {/if}
              {#each component.licenseRefs as ref (ref)}
                {@const license = data.licenses[ref]}
                {#if license}
                  <p class="text-title">{license.name} <span class="spdx">{license.id}</span></p>
                  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
                  <pre class="text" tabindex="0" aria-label={t.licenseText(license.name)}>{license.text}</pre>
                {/if}
              {/each}
            </div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .intro {
    margin: var(--space-3) 0 0;
    color: var(--text-secondary);
    font-size: var(--text-sm);
    line-height: var(--leading);
    text-wrap: pretty;
  }

  /* Filter stays in view while the list scrolls under it. */
  .toolbar {
    position: sticky;
    top: calc(-1 * var(--space-1));
    z-index: 1;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin: var(--space-2) calc(-1 * var(--space-4)) 0;
    padding: var(--space-2) var(--space-4);
    background: var(--bg-raised);
    border-bottom: 1px solid var(--border-subtle);
  }
  .search {
    position: relative;
    flex: 1;
    min-width: 0;
  }
  .search-icon {
    position: absolute;
    top: 50%;
    left: var(--space-2-5, 10px);
    translate: 0 -50%;
    color: var(--text-tertiary);
    pointer-events: none;
  }
  .input {
    width: 100%;
    height: 32px;
    padding: 0 var(--space-8) 0 var(--space-8);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-md);
    background: var(--bg-sunken);
    color: var(--text);
    font: inherit;
    font-size: var(--text-md);
    transition:
      border-color var(--dur-fast) var(--ease-out),
      box-shadow var(--dur-fast) var(--ease-out);
  }
  .input::placeholder {
    color: var(--text-tertiary);
  }
  .input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  .clear {
    position: absolute;
    top: 50%;
    right: 3px;
    translate: 0 -50%;
    width: 26px;
    height: 26px;
  }
  .count {
    flex: none;
    color: var(--text-tertiary);
    font-size: var(--text-xs);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-3);
    min-height: 160px;
    color: var(--text-secondary);
    font-size: var(--text-sm);
    text-align: center;
  }
  .state p {
    display: flex;
    align-items: center;
    gap: var(--space-1-5);
    margin: 0;
  }
  .state:has(.spinner) {
    flex-direction: row;
  }

  .list {
    margin: var(--space-2) 0 0;
    padding: 0;
    list-style: none;
  }
  .list li + li {
    border-top: 1px solid var(--border-subtle);
  }
  .row {
    display: grid;
    grid-template-columns: 12px minmax(0, 1fr) minmax(0, max-content);
    align-items: center;
    gap: var(--space-2);
    width: 100%;
    min-height: 36px;
    padding: var(--space-1-5) var(--space-2);
    border: 0;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--text);
    text-align: left;
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-out);
  }
  .row:hover {
    background: var(--bg-hover);
  }
  .row:focus-visible {
    outline-offset: -2px;
  }
  .chevron {
    display: grid;
    place-items: center;
    color: var(--text-tertiary);
    transition: rotate var(--dur-base) var(--ease-out);
  }
  .open .chevron {
    rotate: 90deg;
    color: var(--accent);
  }
  .row-name {
    display: flex;
    align-items: baseline;
    gap: var(--space-1-5);
    min-width: 0;
  }
  .name {
    overflow: hidden;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    font-weight: 700;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .version {
    flex: none;
    color: var(--text-tertiary);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    font-variant-numeric: tabular-nums;
  }
  .kind {
    flex: none;
    align-self: center;
    margin-left: var(--space-1);
    padding: 0 var(--space-1-5);
    border-radius: var(--radius-xs);
    background: var(--bg-active);
    color: var(--text-secondary);
    font-size: var(--text-2xs);
    font-weight: var(--weight-medium);
    white-space: nowrap;
  }
  .license {
    overflow: hidden;
    color: var(--text-secondary);
    font-size: var(--text-xs);
    text-align: right;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .open .license {
    color: var(--text);
  }

  .panel {
    padding: var(--space-1) var(--space-2) var(--space-3) calc(12px + var(--space-4));
  }
  .source {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }
  .source code {
    overflow: hidden;
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    text-overflow: ellipsis;
    white-space: nowrap;
    user-select: text;
  }
  .text-title {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    margin: var(--space-2) 0 var(--space-1-5);
    font-size: var(--text-sm);
    font-weight: var(--weight-semibold);
  }
  .spdx {
    color: var(--text-tertiary);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    font-weight: 400;
  }
  .text {
    max-height: 260px;
    margin: 0;
    padding: var(--space-3);
    overflow: auto;
    overscroll-behavior: contain;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-sunken);
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    line-height: var(--leading);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    user-select: text;
  }
  .text:focus-visible {
    outline: 2px solid var(--focus-ring);
    outline-offset: 1px;
  }

  @media (max-width: 560px) {
    .kind {
      display: none;
    }
  }
</style>
