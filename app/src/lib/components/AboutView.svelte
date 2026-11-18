<!--
  Settings > About: the app's identity (mark, name, version, tagline), the
  credits, links that open in the system browser (through `open_link`, by id),
  the legal notes and the entries to the two nested views: the app's own MIT
  license and the third-party licenses. Navigation and focus are handled by
  SettingsDialog; this view only reports what was chosen.
-->
<script lang="ts">
  import type { ExternalLink } from "../backend";
  import { i18n } from "../i18n";
  import AppMark from "./AppMark.svelte";
  import Icon, { type IconName } from "./Icon.svelte";

  let {
    version,
    linkError,
    onOpenLink,
    onShowLicense,
    onShowThirdParty,
  }: {
    version: string;
    /** Message of the last failed `open_link`, if any. */
    linkError: string | null;
    onOpenLink: (link: ExternalLink) => void;
    onShowLicense: () => void;
    onShowThirdParty: () => void;
  } = $props();

  const t = $derived(i18n.t.about);

  const links = $derived<{ link: ExternalLink; icon: IconName; title: string; hint: string }[]>([
    { link: "website", icon: "globe", title: t.website, hint: t.websiteHint },
    { link: "repository", icon: "code", title: t.sourceCode, hint: t.sourceCodeHint },
    { link: "issues", icon: "flag", title: t.reportProblem, hint: t.reportProblemHint },
  ]);
</script>

<div class="about">
  <div class="hero">
    <span class="mark-tile"><AppMark size={34} /></span>
    <div class="identity">
      <p class="name">
        {i18n.t.appName}
        <span class="version" title={t.version(version)}>v{version}</span>
      </p>
      <p class="tagline">{t.tagline}</p>
    </div>
  </div>

  <section class="section" aria-labelledby="about-credits">
    <h3 class="section-title caps-label" id="about-credits">{t.credits}</h3>
    <dl class="facts">
      <div>
        <dt>{t.developedBy}</dt>
        <dd>{t.developer}</dd>
      </div>
      <div>
        <dt>{t.author}</dt>
        <dd>{t.authorName}</dd>
      </div>
    </dl>
  </section>

  <section class="section" aria-labelledby="about-links">
    <h3 class="section-title caps-label" id="about-links">{t.links}</h3>
    <div class="links">
      {#each links as item (item.link)}
        <button
          type="button"
          class="link-tile"
          data-nav={`link-${item.link}`}
          title={t.opensInBrowser}
          onclick={() => onOpenLink(item.link)}
        >
          <span class="tile-icon"><Icon name={item.icon} size={16} /></span>
          <span class="tile-text">
            <span class="tile-title">{item.title}</span>
            <span class="tile-hint">{item.hint}</span>
          </span>
          <span class="tile-out" aria-hidden="true"><Icon name="external" size={12} /></span>
        </button>
      {/each}
    </div>
    {#if linkError}
      <p class="field-error" role="alert"><Icon name="alert" size={12} />{t.linkFailed}: {linkError}</p>
    {/if}
  </section>

  <section class="section" aria-labelledby="about-legal">
    <h3 class="section-title caps-label" id="about-legal">{t.legal}</h3>
    <p class="copyright"><strong>{t.copyright}</strong><br />{t.licensedUnder}</p>
    <div class="nav-list">
      <button type="button" class="nav-row" data-nav="license" onclick={onShowLicense}>
        <span class="tile-icon"><Icon name="scale" size={16} /></span>
        <span class="tile-text">
          <span class="tile-title">{t.viewLicense}</span>
          <span class="tile-hint">{t.viewLicenseHint}</span>
        </span>
        <span class="chevron" aria-hidden="true"><Icon name="chevronRight" size={14} /></span>
      </button>
      <button type="button" class="nav-row" data-nav="thirdParty" onclick={onShowThirdParty}>
        <span class="tile-icon"><Icon name="package" size={16} /></span>
        <span class="tile-text">
          <span class="tile-title">{t.thirdParty}</span>
          <span class="tile-hint">{t.thirdPartyHint}</span>
        </span>
        <span class="chevron" aria-hidden="true"><Icon name="chevronRight" size={14} /></span>
      </button>
    </div>
    <p class="trademark"><Icon name="info" size={12} />{t.trademark}</p>
  </section>
</div>

<style>
  .hero {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    margin: var(--space-4) 0 var(--space-1);
    padding: var(--space-4);
    border: 1px solid var(--accent-border);
    border-radius: var(--radius-lg);
    background:
      radial-gradient(120% 140% at 0% 0%, var(--accent-soft), transparent 60%),
      var(--bg-surface);
  }
  .mark-tile {
    display: grid;
    place-items: center;
    flex: none;
    width: 56px;
    height: 56px;
    border: 1px solid var(--accent-border);
    border-radius: 14px;
    background: var(--bg-raised);
    box-shadow: var(--shadow-sm), 0 0 0 4px var(--accent-soft);
  }
  .identity {
    min-width: 0;
  }
  .name {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--space-1) var(--space-2);
    margin: 0;
    font-size: var(--text-xl);
    font-weight: var(--weight-semibold);
    line-height: var(--leading-tight);
    letter-spacing: -0.01em;
  }
  .version {
    padding: 1px var(--space-1-5);
    border: 1px solid var(--accent-border);
    border-radius: var(--radius-pill);
    color: var(--accent);
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 700;
    letter-spacing: 0;
    font-variant-numeric: tabular-nums;
  }
  .tagline {
    margin: var(--space-1) 0 0;
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }

  .section + .section {
    margin-top: var(--space-2);
    border-top: 1px solid var(--border-subtle);
  }
  .section-title {
    margin: var(--space-4) 0 var(--space-3);
  }

  .facts {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-2);
    margin: 0;
  }
  .facts div {
    padding: var(--space-2) var(--space-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-surface);
  }
  .facts dt {
    color: var(--text-tertiary);
    font-size: var(--text-xs);
  }
  .facts dd {
    margin: 2px 0 0;
    color: var(--text);
    font-size: var(--text-md);
    font-weight: var(--weight-semibold);
  }

  .links {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--space-2);
  }
  .link-tile,
  .nav-row {
    display: flex;
    align-items: center;
    gap: var(--space-2-5, 10px);
    min-width: 0;
    padding: var(--space-2-5, 10px);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-surface);
    color: var(--text);
    text-align: left;
    cursor: pointer;
    transition:
      border-color var(--dur-fast) var(--ease-out),
      background-color var(--dur-fast) var(--ease-out),
      box-shadow var(--dur-fast) var(--ease-out);
  }
  .link-tile:hover,
  .nav-row:hover {
    border-color: var(--accent-border);
    background: color-mix(in srgb, var(--bg-surface), var(--accent) 4%);
  }
  .link-tile:active,
  .nav-row:active {
    background: color-mix(in srgb, var(--bg-surface), var(--accent) 8%);
  }
  .tile-icon {
    display: grid;
    place-items: center;
    flex: none;
    width: 30px;
    height: 30px;
    border-radius: var(--radius-md);
    background: var(--bg-active);
    color: var(--text-secondary);
    transition:
      background-color var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }
  .link-tile:hover .tile-icon,
  .nav-row:hover .tile-icon {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .tile-text {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }
  .tile-title {
    overflow: hidden;
    font-size: var(--text-md);
    font-weight: var(--weight-semibold);
    line-height: var(--leading-tight);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tile-hint {
    overflow: hidden;
    color: var(--text-tertiary);
    font-size: var(--text-xs);
    line-height: var(--leading-tight);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tile-out,
  .chevron {
    flex: none;
    color: var(--text-tertiary);
    transition:
      color var(--dur-fast) var(--ease-out),
      translate var(--dur-fast) var(--ease-out);
  }
  /* Link tiles stack: icon and the "opens outside" arrow, then the text. */
  .link-tile {
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: start;
    row-gap: var(--space-2);
    padding: var(--space-3);
  }
  .link-tile .tile-text {
    grid-column: 1 / -1;
  }
  .link-tile .tile-title {
    white-space: normal;
  }
  .link-tile .tile-out {
    grid-column: 2;
    grid-row: 1;
  }
  .link-tile:hover .tile-out {
    color: var(--accent);
    translate: 1px -1px;
  }
  .nav-row:hover .chevron {
    color: var(--accent);
    translate: 2px 0;
  }

  .field-error {
    display: flex;
    align-items: center;
    gap: var(--space-1-5);
    margin: var(--space-2) 0 0;
    color: var(--text);
    font-size: var(--text-xs);
    font-weight: var(--weight-medium);
  }

  .copyright {
    margin: 0 0 var(--space-3);
    color: var(--text-secondary);
    font-size: var(--text-sm);
    line-height: var(--leading);
  }
  .copyright strong {
    color: var(--text);
    font-weight: var(--weight-semibold);
  }
  .nav-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .trademark {
    display: flex;
    align-items: flex-start;
    gap: var(--space-1-5);
    margin: var(--space-4) 0 0;
    color: var(--text-tertiary);
    font-size: var(--text-xs);
    line-height: var(--leading);
    text-wrap: pretty;
  }
  .trademark :global(.icon) {
    margin-top: 2px;
  }

  @media (max-width: 560px) {
    .links {
      grid-template-columns: 1fr;
    }
    .link-tile {
      grid-template-columns: auto minmax(0, 1fr) auto;
      align-items: center;
      column-gap: var(--space-2-5, 10px);
      padding: var(--space-2-5, 10px);
    }
    .link-tile .tile-text {
      grid-column: 2;
      grid-row: 1;
    }
    .link-tile .tile-out {
      grid-column: 3;
      align-self: center;
    }
    .facts {
      grid-template-columns: 1fr;
    }
  }
</style>
