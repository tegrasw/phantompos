<!--
  Settings (SPEC "Configurações"), opened from the header's gear: a native
  modal <dialog> like the code close-up (focus stays inside, Esc and a click
  outside close it, fade and lift in, none under reduced motion). The backend
  saves every change and keeps it across launches.

  Connection
  - Port: typed, validated (1-65535) and applied explicitly with "Apply" or
    Enter, because applying restarts the listener and drops connections. The
    line below shows the outcome: restarting, listening, or the listener
    error with its actions.
  - Accept print jobs from: "This computer" / "Local network". Choosing the
    network does not apply at once: a confirmation panel inside the dialog
    explains that any device on the network can then print, with no
    authentication or encryption (SPEC "Rede e status"), and only "Allow
    network access" applies it. Back to local applies at once. In network
    mode the resulting addresses are listed with copy buttons.
  Printer
  - Paper width (next receipt) and default code page (next connection) apply
    at once.

  Every change is optimistic (App updates `settings` first and rolls back if
  `update_settings` rejects); a rejection shows its message under the field.
  `settings:changed` keeps the dialog in sync with changes made elsewhere
  (the roll toolbar's paper control, the port-in-use actions).

  About (the footer row) opens nested views inside the same dialog:
  Settings > About > MIT license | Third-party licenses. The header then shows
  a back button; Esc goes back one level, and closes the dialog only from
  Settings. Going forward focuses the new view (its filter field, or the back
  button); going back focuses the control that opened the view.
-->
<script lang="ts">
  import { tick } from "svelte";
  import { fade, fly } from "svelte/transition";
  import {
    CODE_PAGES,
    isValidPort,
    PAPERS,
    type CodePage,
    type ExternalLink,
    type NetworkMode,
    type Paper,
    type Settings,
  } from "../backend";
  import { i18n } from "../i18n";
  import { duration, easeOut } from "../motion";
  import { errorText } from "../receiptExport";
  import type { EndpointStatus, ListenerControls } from "../status";
  import AboutView from "./AboutView.svelte";
  import AppMark from "./AppMark.svelte";
  import CopyButton from "./CopyButton.svelte";
  import Icon from "./Icon.svelte";
  import ListenerActions from "./ListenerActions.svelte";

  let {
    open,
    settings,
    status,
    controls,
    onChange,
    onOpenLink,
    onClose,
  }: {
    open: boolean;
    settings: Settings;
    status: EndpointStatus;
    controls: ListenerControls;
    /** Optimistic update; rejects (after rolling back) when the backend refuses. */
    onChange: (patch: Partial<Settings>) => Promise<void>;
    /** Opens a fixed external page in the system browser; rejects with a message. */
    onOpenLink: (link: ExternalLink) => Promise<void>;
    onClose: () => void;
  } = $props();

  const t = $derived(i18n.t.settings);
  const ta = $derived(i18n.t.about);
  const VERSION = __APP_VERSION__;
  // The LICENSE file is hard-wrapped at 80 columns; reflow its paragraphs so
  // they wrap to the dialog's width instead.
  const LICENSE_PARAGRAPHS = __APP_LICENSE__.split(/\n\s*\n/).map((paragraph) => paragraph.replace(/\s*\n\s*/g, " ").trim());

  let dialog: HTMLDialogElement | undefined = $state();
  let body: HTMLDivElement | undefined = $state();
  let backButton: HTMLButtonElement | undefined = $state();
  let shown = $state(false);
  let closing: ReturnType<typeof setTimeout> | null = null;

  // Open / close -------------------------------------------------------------
  $effect(() => {
    if (!open || !dialog) return;
    if (closing !== null) {
      clearTimeout(closing);
      closing = null;
    }
    if (!dialog.open) {
      resetDrafts();
      view = "settings";
      linkError = null;
      dialog.showModal();
    }
    const frame = requestAnimationFrame(() => (shown = true));
    return () => cancelAnimationFrame(frame);
  });

  $effect(() => {
    if (!open && dialog?.open) animateClose();
  });

  function animateClose(): void {
    if (!dialog?.open || closing !== null) return;
    shown = false;
    closing = setTimeout(() => {
      closing = null;
      dialog?.close();
    }, duration("base"));
  }

  function requestClose(): void {
    animateClose();
    onClose();
  }

  function onCancel(event: Event): void {
    event.preventDefault();
    // Esc normally arrives as keydown first (onDialogKey); this covers other
    // close requests, and one that follows an Esc already handled there.
    if (performance.now() - lastEscBack < 100) return;
    if (view !== "settings") void back();
    else requestClose();
  }

  function onDialogClick(event: MouseEvent): void {
    if (event.target === dialog) requestClose();
  }

  // Nested views (About) ---------------------------------------------------------
  type View = "settings" | "about" | "license" | "thirdParty";
  const PARENT: Record<View, View | null> = { settings: null, about: "settings", license: "about", thirdParty: "about" };

  let view = $state<View>("settings");
  /** +1 when going deeper, -1 when going back: the direction views slide in from. */
  let direction = $state(1);
  let lastEscBack = 0;
  let linkError = $state<string | null>(null);

  async function navigate(next: View): Promise<void> {
    direction = 1;
    view = next;
    linkError = null;
    if (body) body.scrollTop = 0;
    await tick();
    const initial = body?.querySelector<HTMLElement>("[data-initial-focus]");
    (initial ?? backButton)?.focus();
  }

  async function back(): Promise<void> {
    const from = view;
    const parent = PARENT[from];
    if (!parent) return;
    direction = -1;
    view = parent;
    if (from === "about") linkError = null;
    await tick();
    dialog?.querySelector<HTMLElement>(`[data-nav="${from}"]`)?.focus();
  }

  function onDialogKey(event: KeyboardEvent): void {
    if (event.key !== "Escape" || event.defaultPrevented || view === "settings") return;
    event.preventDefault();
    lastEscBack = performance.now();
    void back();
  }

  async function openLink(link: ExternalLink): Promise<void> {
    linkError = null;
    try {
      await onOpenLink(link);
    } catch (error) {
      linkError = errorText(error);
    }
  }

  const heading = $derived.by((): { title: string; subtitle: string } => {
    switch (view) {
      case "about":
        return { title: ta.title, subtitle: ta.subtitle };
      case "license":
        return { title: ta.licenseTitle, subtitle: ta.licenseSubtitle };
      case "thirdParty":
        return { title: ta.thirdParty, subtitle: ta.thirdPartySubtitle };
      default:
        return { title: t.title, subtitle: t.subtitle };
    }
  });

  // Port -----------------------------------------------------------------------
  let portDraft = $state("");
  let portFocused = $state(false);
  let portError = $state<string | null>(null);
  /** The port just applied, until the listener reports on it. */
  let pendingPort = $state<number | null>(null);
  let portBusy = $state(false);

  const parsedPort = $derived(/^\s*\d{1,5}\s*$/.test(portDraft) ? Number(portDraft) : NaN);
  const portValid = $derived(isValidPort(parsedPort));
  const portDirty = $derived(portDraft.trim() !== String(settings.port));
  const portMessage = $derived(portError ?? (portDraft.trim() !== "" && !portValid ? t.portInvalid : null));

  // Follow outside changes (e.g. "Use port 9101") while the field is idle.
  $effect(() => {
    const current = String(settings.port);
    if (!portFocused && !portBusy && portError === null) portDraft = current;
  });

  // The pending marker ends when the listener answers on the new port.
  $effect(() => {
    if (pendingPort === null) return;
    if ((status.kind === "listening" || status.kind === "error") && status.port === pendingPort) pendingPort = null;
  });

  async function applyPort(): Promise<void> {
    if (!portValid || portBusy) return;
    if (!portDirty) {
      portDraft = String(settings.port);
      return;
    }
    portBusy = true;
    portError = null;
    pendingPort = parsedPort;
    try {
      await onChange({ port: parsedPort });
    } catch (error) {
      portError = `${t.saveFailed}: ${errorText(error)}`;
      pendingPort = null;
    } finally {
      portBusy = false;
    }
  }

  function onPortKey(event: KeyboardEvent): void {
    if (event.key === "Enter") {
      event.preventDefault();
      void applyPort();
    } else if (event.key === "Escape" && portDirty) {
      // First Esc reverts the draft; the next one closes the dialog.
      event.preventDefault();
      event.stopPropagation();
      portDraft = String(settings.port);
      portError = null;
    }
  }

  // Network access ---------------------------------------------------------------
  let confirming = $state(false);
  let accessBusy = $state(false);
  let accessError = $state<string | null>(null);

  async function chooseAccess(mode: NetworkMode): Promise<void> {
    accessError = null;
    if (mode === settings.networkMode) {
      confirming = false;
      return;
    }
    if (mode === "network") {
      confirming = true;
      return;
    }
    confirming = false;
    await applyAccess("local");
  }

  async function applyAccess(mode: NetworkMode): Promise<void> {
    accessBusy = true;
    try {
      await onChange({ networkMode: mode });
      confirming = false;
    } catch (error) {
      accessError = `${t.saveFailed}: ${errorText(error)}`;
    } finally {
      accessBusy = false;
    }
  }

  function onAccessKey(event: KeyboardEvent): void {
    const order: NetworkMode[] = ["local", "network"];
    const current = confirming ? "network" : settings.networkMode;
    const step = event.key === "ArrowRight" || event.key === "ArrowDown" ? 1 : event.key === "ArrowLeft" || event.key === "ArrowUp" ? -1 : 0;
    if (!step) return;
    event.preventDefault();
    const next = order[(order.indexOf(current) + step + order.length) % order.length];
    void chooseAccess(next);
    const group = event.currentTarget as HTMLElement;
    queueMicrotask(() => group.querySelector<HTMLElement>(`[data-mode="${next}"]`)?.focus());
  }

  // Printer --------------------------------------------------------------------
  let paperError = $state<string | null>(null);
  let codePageError = $state<string | null>(null);

  async function choosePaper(paper: Paper): Promise<void> {
    if (paper === settings.paper) return;
    paperError = null;
    try {
      await onChange({ paper });
    } catch (error) {
      paperError = `${t.saveFailed}: ${errorText(error)}`;
    }
  }

  function onPaperKey(event: KeyboardEvent): void {
    const step = event.key === "ArrowRight" || event.key === "ArrowDown" ? 1 : event.key === "ArrowLeft" || event.key === "ArrowUp" ? -1 : 0;
    if (!step) return;
    event.preventDefault();
    const next = PAPERS[(PAPERS.indexOf(settings.paper) + step + PAPERS.length) % PAPERS.length];
    void choosePaper(next);
    const group = event.currentTarget as HTMLElement;
    queueMicrotask(() => group.querySelector<HTMLElement>(`[data-paper="${next}"]`)?.focus());
  }

  async function chooseCodePage(event: Event): Promise<void> {
    const value = (event.currentTarget as HTMLSelectElement).value as CodePage;
    codePageError = null;
    try {
      await onChange({ defaultCodePage: value });
    } catch (error) {
      codePageError = `${t.saveFailed}: ${errorText(error)}`;
    }
  }

  function resetDrafts(): void {
    portDraft = String(settings.port);
    portError = null;
    confirming = false;
    accessError = null;
    paperError = null;
    codePageError = null;
  }

  // Listener outcome shown under the port ----------------------------------------
  const outcome = $derived.by((): { tone: "ok" | "error" | "idle" | "busy"; text: string } => {
    if (pendingPort !== null || (status.kind === "stopped" && (portBusy || accessBusy)))
      return { tone: "busy", text: i18n.t.listener.restarting };
    switch (status.kind) {
      case "listening":
        return { tone: "ok", text: t.portListening(status.port) };
      case "error":
        return { tone: "error", text: i18n.t.listener.errorShort[status.code](status.port) };
      case "stopped":
        return { tone: "idle", text: t.portStopped(status.port) };
      default:
        return { tone: "idle", text: t.portStarting };
    }
  });

  const showMode: NetworkMode = $derived(confirming ? "network" : settings.networkMode);
  const accessChoices = $derived([
    { mode: "local" as const, icon: "laptop" as const, title: t.accessLocal, hint: t.accessLocalHint },
    { mode: "network" as const, icon: "globe" as const, title: t.accessNetwork, hint: t.accessNetworkHint },
  ]);
  const reach = $derived(status.kind === "listening" || status.kind === "stopped" ? status : null);
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
<dialog
  class="settings"
  bind:this={dialog}
  data-open={shown ? "" : undefined}
  aria-labelledby="settings-title"
  aria-describedby="settings-subtitle"
  oncancel={onCancel}
  onclick={onDialogClick}
  onkeydown={onDialogKey}
>
  <div class="card" data-view={view}>
    <header class="head">
      {#if view === "settings"}
        <span class="glyph"><Icon name="gear" size={16} /></span>
      {:else}
        <button
          type="button"
          class="icon-btn back"
          bind:this={backButton}
          aria-label={ta.back}
          title={ta.back}
          onclick={() => back()}
        >
          <Icon name="chevronLeft" size={16} />
        </button>
      {/if}
      <div class="titles">
        <h2 id="settings-title">{heading.title}</h2>
        <p class="subtitle" id="settings-subtitle">{heading.subtitle}</p>
      </div>
      <button type="button" class="icon-btn" aria-label={t.close} title={t.close} onclick={requestClose}>
        <Icon name="x" />
      </button>
    </header>

    <div class="body" bind:this={body}>
      {#key view}
        <div class="view" in:fly={{ x: 14 * direction, duration: duration("base"), easing: easeOut }}>
          {#if view === "about"}
            <AboutView
              version={VERSION}
              {linkError}
              onOpenLink={openLink}
              onShowLicense={() => navigate("license")}
              onShowThirdParty={() => navigate("thirdParty")}
            />
          {:else if view === "license"}
            <div class="license">
              <div class="license-text">
                {#each LICENSE_PARAGRAPHS as paragraph, index (index)}
                  <p class:license-heading={index === 0}>{paragraph}</p>
                {/each}
              </div>
              <div class="license-actions">
                <button type="button" class="btn" onclick={() => openLink("license")}>
                  <Icon name="external" size={12} />{ta.licenseOnline}
                </button>
                {#if linkError}
                  <p class="field-error" role="alert"><Icon name="alert" size={12} />{ta.linkFailed}: {linkError}</p>
                {/if}
              </div>
            </div>
          {:else if view === "thirdParty"}
            <!-- Loaded on demand with its data: neither is in the main bundle. -->
            {#await import("./ThirdPartyLicenses.svelte")}
              <div class="view-loading" role="status"><span class="spinner" aria-hidden="true"></span>{ta.loading}</div>
            {:then module}
              <module.default autofocus />
            {:catch error}
              <p class="field-error view-loading" role="alert"><Icon name="alert" size={12} />{ta.loadFailed}: {errorText(error)}</p>
            {/await}
          {:else}
            <!-- Connection ------------------------------------------------------ -->
            <section class="section" aria-labelledby="settings-connection">
              <h3 class="section-title caps-label" id="settings-connection">{t.connection}</h3>

              <div class="field">
                <label class="field-label" for="settings-port">{t.port}</label>
                <div class="port-row">
                  <input
                    id="settings-port"
                    class="input mono"
                    class:invalid={portMessage !== null}
                    type="text"
                    inputmode="numeric"
                    autocomplete="off"
                    spellcheck="false"
                    maxlength="5"
                    placeholder={t.portPlaceholder}
                    aria-invalid={portMessage !== null}
                    aria-describedby="settings-port-hint settings-port-status"
                    bind:value={portDraft}
                    onfocus={() => (portFocused = true)}
                    onblur={() => (portFocused = false)}
                    onkeydown={onPortKey}
                    oninput={() => (portError = null)}
                  />
                  <button
                    type="button"
                    class="btn primary apply"
                    disabled={!portValid || !portDirty || portBusy}
                    aria-busy={portBusy}
                    onclick={applyPort}
                  >
                    {#if portBusy}<span class="spinner" aria-hidden="true"></span>{t.applying}{:else}{t.apply}{/if}
                  </button>
                </div>
                {#if portMessage}
                  <p class="field-error" role="alert" transition:fade={{ duration: duration("fast") }}>
                    <Icon name="alert" size={12} />{portMessage}
                  </p>
                {/if}
                <p class="hint" id="settings-port-hint">{t.portHint}</p>
                <div class="outcome" id="settings-port-status" data-tone={outcome.tone} role="status">
                  {#if outcome.tone === "busy"}
                    <span class="spinner" aria-hidden="true"></span>
                  {:else}
                    <span class="outcome-dot" aria-hidden="true"></span>
                  {/if}
                  <span>{outcome.text}</span>
                </div>
                {#if status.kind === "error" && pendingPort === null}
                  <div class="outcome-actions" transition:fade={{ duration: duration("fast") }}>
                    <ListenerActions {status} {controls} />
                  </div>
                {/if}
              </div>

              <div class="field">
                <span class="field-label" id="settings-access-label">{t.access}</span>
                <div class="choices" role="radiogroup" aria-labelledby="settings-access-label" tabindex="-1" onkeydown={onAccessKey}>
                  {#each accessChoices as choice (choice.mode)}
                    <button
                      type="button"
                      role="radio"
                      class="choice"
                      data-mode={choice.mode}
                      aria-checked={showMode === choice.mode}
                      class:pending={confirming && choice.mode === "network"}
                      tabindex={showMode === choice.mode ? 0 : -1}
                      disabled={accessBusy}
                      onclick={() => chooseAccess(choice.mode)}
                    >
                      <span class="choice-icon"><Icon name={choice.icon} size={16} /></span>
                      <span class="choice-text">
                        <span class="choice-title">{choice.title}</span>
                        <span class="choice-hint">{choice.hint}</span>
                      </span>
                      <span class="radio" aria-hidden="true"></span>
                    </button>
                  {/each}
                </div>

                {#if confirming}
                  <div class="confirm" role="alertdialog" aria-labelledby="settings-confirm-title" aria-describedby="settings-confirm-body" transition:fade={{ duration: duration("base") }}>
                    <span class="confirm-icon" aria-hidden="true"><Icon name="shield" size={18} /></span>
                    <div class="confirm-text">
                      <p class="confirm-title" id="settings-confirm-title">{t.confirmTitle}</p>
                      <p class="confirm-body" id="settings-confirm-body">{t.confirmBody}</p>
                      <div class="confirm-actions">
                        <button type="button" class="btn primary" disabled={accessBusy} aria-busy={accessBusy} onclick={() => applyAccess("network")}>
                          {#if accessBusy}<span class="spinner" aria-hidden="true"></span>{/if}{t.confirmAllow}
                        </button>
                        <button type="button" class="btn" disabled={accessBusy} onclick={() => (confirming = false)}>{t.confirmCancel}</button>
                      </div>
                    </div>
                  </div>
                {:else if settings.networkMode === "network"}
                  <div class="reach" transition:fade={{ duration: duration("base") }}>
                    <span class="reach-label caps-label">{t.reachableAt}</span>
                    {#if status.kind === "stopped" || pendingPort !== null || accessBusy}
                      <p class="reach-note"><span class="spinner" aria-hidden="true"></span>{i18n.t.listener.restarting}</p>
                    {:else if reach && reach.network && reach.addresses.length > 0}
                      <ul class="reach-list">
                        {#each reach.addresses as address (address)}
                          <li>
                            <code>{address}</code>
                            <CopyButton text={address} label={i18n.t.header.copyAddressOf(address)} bubble="above" />
                          </li>
                        {/each}
                      </ul>
                    {:else if reach}
                      <p class="reach-note">{t.noAddresses}</p>
                    {:else}
                      <p class="reach-note">{outcome.text}</p>
                    {/if}
                  </div>
                {/if}
                {#if accessError}
                  <p class="field-error" role="alert"><Icon name="alert" size={12} />{accessError}</p>
                {/if}
              </div>
            </section>

            <!-- Printer --------------------------------------------------------- -->
            <section class="section" aria-labelledby="settings-printer">
              <h3 class="section-title caps-label" id="settings-printer">{t.printer}</h3>

              <div class="field inline">
                <div class="field-text">
                  <span class="field-label" id="settings-paper-label">{t.paper}</span>
                  <p class="hint">{t.paperHint}</p>
                </div>
                <div class="segmented" role="radiogroup" aria-labelledby="settings-paper-label" tabindex="-1" onkeydown={onPaperKey}>
                  <span class="thumb" style:translate={settings.paper === "80mm" ? "100% 0" : "0 0"} aria-hidden="true"></span>
                  {#each PAPERS as option (option)}
                    <button
                      type="button"
                      role="radio"
                      data-paper={option}
                      aria-checked={settings.paper === option}
                      tabindex={settings.paper === option ? 0 : -1}
                      onclick={() => choosePaper(option)}
                    >
                      {i18n.t.paper.names[option]}
                    </button>
                  {/each}
                </div>
              </div>
              {#if paperError}
                <p class="field-error" role="alert"><Icon name="alert" size={12} />{paperError}</p>
              {/if}

              <div class="field">
                <label class="field-label" for="settings-codepage">{t.codePage}</label>
                <div class="select-wrap">
                  <select id="settings-codepage" class="input" value={settings.defaultCodePage} onchange={chooseCodePage} aria-describedby="settings-codepage-hint">
                    {#each CODE_PAGES as page (page.id)}
                      <option value={page.id}>{page.name} — {t.codePages[page.id]}</option>
                    {/each}
                  </select>
                  <span class="select-chevron" aria-hidden="true"><Icon name="chevronRight" size={12} /></span>
                </div>
                {#if codePageError}
                  <p class="field-error" role="alert"><Icon name="alert" size={12} />{codePageError}</p>
                {/if}
                <p class="hint" id="settings-codepage-hint">{t.codePageHint}</p>
              </div>
            </section>
          {/if}
        </div>
      {/key}
    </div>

    {#if view === "settings"}
      <footer class="foot">
        <button type="button" class="about-row" data-nav="about" onclick={() => navigate("about")}>
          <AppMark size={20} />
          <span class="about-text">
            <span class="about-title">{ta.open}</span>
            <span class="about-hint">{ta.openHint(VERSION)}</span>
          </span>
          <span class="about-chevron" aria-hidden="true"><Icon name="chevronRight" size={14} /></span>
        </button>
      </footer>
    {/if}
  </div>
</dialog>

<style>
  .settings {
    width: min(540px, calc(100vw - 32px));
    max-width: none;
    max-height: calc(100vh - 32px);
    padding: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    box-shadow: var(--shadow-pop);
    color: var(--text);
    overflow: hidden;
    opacity: 0;
    transform: translateY(8px) scale(0.97);
    transition:
      opacity var(--dur-base) var(--ease-out),
      transform var(--dur-base) var(--ease-out);
  }
  .settings[data-open] {
    opacity: 1;
    transform: none;
  }
  .settings::backdrop {
    background: var(--scrim);
    backdrop-filter: blur(3px);
    -webkit-backdrop-filter: blur(3px);
    opacity: 0;
    transition: opacity var(--dur-base) var(--ease-out);
  }
  .settings[data-open]::backdrop {
    opacity: 1;
  }
  .settings:focus-visible {
    outline: none;
  }

  .card {
    display: flex;
    flex-direction: column;
    max-height: calc(100vh - 34px);
  }

  .head {
    flex: none;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-3) var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--border-subtle);
  }
  .glyph {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border: 1px solid var(--accent-border);
    border-radius: var(--radius-md);
    background: var(--accent-soft);
    color: var(--accent);
    flex: none;
  }
  .titles {
    flex: 1;
    min-width: 0;
  }
  h2 {
    margin: 0;
    font-size: var(--text-lg);
    font-weight: var(--weight-semibold);
    line-height: var(--leading-tight);
  }
  .subtitle {
    margin: 2px 0 0;
    color: var(--text-tertiary);
    font-size: var(--text-xs);
  }

  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    padding: var(--space-1) var(--space-4) var(--space-4);
  }
  .section + .section {
    margin-top: var(--space-2);
    border-top: 1px solid var(--border-subtle);
  }
  .section-title {
    margin: var(--space-4) 0 var(--space-3);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-1-5);
  }
  .field ~ .field {
    margin-top: var(--space-5);
  }
  .section > .field-error {
    margin-top: var(--space-1-5);
  }
  .field.inline {
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
  }
  .field-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .field-label {
    color: var(--text);
    font-size: var(--text-md);
    font-weight: var(--weight-medium);
  }
  .hint {
    margin: 0;
    color: var(--text-tertiary);
    font-size: var(--text-xs);
    line-height: var(--leading);
    text-wrap: pretty;
  }
  .field-error {
    display: flex;
    align-items: center;
    gap: var(--space-1-5);
    margin: 0;
    /* Neutral on purpose: red is reserved for listener errors. */
    color: var(--text);
    font-size: var(--text-xs);
    font-weight: var(--weight-medium);
  }
  .field-error :global(.icon) {
    color: var(--text-secondary);
  }

  /* Inputs ------------------------------------------------------------------ */
  .input {
    height: 32px;
    padding: 0 var(--space-3);
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
  .input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  .input.mono {
    font-family: var(--font-mono);
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
  .input.invalid {
    border-color: var(--text-secondary);
  }
  .port-row {
    display: flex;
    gap: var(--space-2);
  }
  .port-row .input {
    width: 112px;
  }
  .apply {
    height: 32px;
    padding: 0 var(--space-4);
  }

  .outcome {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-top: var(--space-0-5);
    color: var(--text-secondary);
    font-size: var(--text-sm);
    font-weight: var(--weight-medium);
  }
  .outcome-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--text-tertiary);
    flex: none;
  }
  .outcome[data-tone="ok"] {
    color: var(--accent);
  }
  .outcome[data-tone="ok"] .outcome-dot {
    background: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  .outcome[data-tone="error"] {
    color: var(--danger);
  }
  .outcome[data-tone="error"] .outcome-dot {
    background: var(--danger);
    box-shadow: 0 0 0 3px var(--danger-soft);
  }
  .outcome-actions {
    margin-top: var(--space-1);
  }

  /* Access choice ------------------------------------------------------------- */
  .choices {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-2);
  }
  .choices:focus {
    outline: none;
  }
  .choice {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2-5, 10px);
    padding: var(--space-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-surface);
    text-align: left;
    cursor: pointer;
    transition:
      border-color var(--dur-fast) var(--ease-out),
      background-color var(--dur-fast) var(--ease-out),
      box-shadow var(--dur-fast) var(--ease-out);
  }
  .choice:hover:not(:disabled) {
    border-color: var(--border-strong);
  }
  .choice[aria-checked="true"] {
    border-color: var(--accent-border);
    background: color-mix(in srgb, var(--bg-surface), var(--accent) 5%);
    box-shadow: 0 0 0 1px var(--accent-border);
  }
  .choice.pending {
    border-style: dashed;
  }
  .choice:disabled {
    cursor: default;
  }
  .choice-icon {
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
  .choice[aria-checked="true"] .choice-icon {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .choice-text {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .choice-title {
    color: var(--text);
    font-size: var(--text-md);
    font-weight: var(--weight-semibold);
    line-height: var(--leading-tight);
  }
  .choice-hint {
    color: var(--text-tertiary);
    font-size: var(--text-xs);
    line-height: var(--leading-tight);
  }
  .radio {
    flex: none;
    width: 14px;
    height: 14px;
    margin-top: 2px;
    border: 1.5px solid var(--border-strong);
    border-radius: 50%;
    transition:
      border-color var(--dur-fast) var(--ease-out),
      border-width var(--dur-fast) var(--ease-out);
  }
  .choice[aria-checked="true"] .radio {
    border: 4px solid var(--accent);
  }

  .confirm {
    display: flex;
    gap: var(--space-3);
    margin-top: var(--space-1);
    padding: var(--space-3) var(--space-4) var(--space-4) var(--space-3);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-lg);
    background: var(--bg-sunken);
  }
  .confirm-icon {
    display: grid;
    place-items: center;
    flex: none;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    background: var(--bg-active);
    color: var(--text);
  }
  .confirm-text {
    min-width: 0;
  }
  .confirm-title {
    margin: 5px 0 var(--space-1);
    font-size: var(--text-md);
    font-weight: var(--weight-semibold);
  }
  .confirm-body {
    margin: 0 0 var(--space-3);
    color: var(--text-secondary);
    font-size: var(--text-sm);
    line-height: var(--leading);
    text-wrap: pretty;
  }
  .confirm-actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }

  .reach {
    margin-top: var(--space-1);
    padding: var(--space-2) var(--space-2) var(--space-2) var(--space-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-sunken);
  }
  .reach-list {
    margin: var(--space-1) 0 0;
    padding: 0;
    list-style: none;
  }
  .reach-list li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 28px;
  }
  .reach-list code {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    font-weight: 700;
  }
  .reach-note {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: var(--space-1) 0 var(--space-0-5);
    color: var(--text-secondary);
    font-size: var(--text-sm);
    line-height: var(--leading);
  }

  /* Paper -------------------------------------------------------------------- */
  .segmented {
    position: relative;
    flex: none;
    display: grid;
    grid-template-columns: 1fr 1fr;
    padding: 2px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-sunken);
  }
  .segmented:focus {
    outline: none;
  }
  .thumb {
    position: absolute;
    top: 2px;
    bottom: 2px;
    left: 2px;
    width: calc(50% - 2px);
    border-radius: var(--radius-sm);
    background: var(--bg-raised);
    box-shadow: var(--shadow-sm), var(--inset-highlight), 0 0 0 1px var(--border);
    transition: translate var(--dur-base) var(--ease-out);
  }
  .segmented button {
    position: relative;
    height: 26px;
    padding: 0 var(--space-3);
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-tertiary);
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    font-weight: 700;
    white-space: nowrap;
    cursor: pointer;
    transition: color var(--dur-fast) var(--ease-out);
  }
  .segmented button:hover,
  .segmented button[aria-checked="true"] {
    color: var(--text);
  }
  .segmented button:focus-visible {
    outline-offset: 0;
  }

  /* Code page ---------------------------------------------------------------- */
  .select-wrap {
    position: relative;
  }
  .select-wrap select {
    width: 100%;
    padding-right: var(--space-8);
    appearance: none;
    -webkit-appearance: none;
    cursor: pointer;
  }
  .select-chevron {
    position: absolute;
    top: 50%;
    right: var(--space-3);
    translate: 0 -50%;
    rotate: 90deg;
    color: var(--text-tertiary);
    pointer-events: none;
  }

  /* Footer: the way into About. */
  .foot {
    flex: none;
    padding: var(--space-1-5) var(--space-2);
    border-top: 1px solid var(--border-subtle);
    background: var(--bg-surface);
  }
  .about-row {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    width: 100%;
    padding: var(--space-1-5) var(--space-2);
    border: 0;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--text);
    text-align: left;
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-out);
  }
  .about-row:hover {
    background: var(--bg-hover);
  }
  .about-row:active {
    background: var(--bg-active);
  }
  .about-text {
    flex: 1;
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0 var(--space-2);
    min-width: 0;
  }
  .about-title {
    font-size: var(--text-sm);
    font-weight: var(--weight-semibold);
  }
  .about-hint {
    color: var(--text-tertiary);
    font-size: var(--text-xs);
  }
  .about-chevron {
    color: var(--text-tertiary);
    transition:
      color var(--dur-fast) var(--ease-out),
      translate var(--dur-fast) var(--ease-out);
  }
  .about-row:hover .about-chevron {
    color: var(--accent);
    translate: 2px 0;
  }

  /* Nested views ------------------------------------------------------------ */
  .back {
    flex: none;
    width: 30px;
    height: 30px;
    border-color: var(--border);
  }
  /* The licence list is long: give the dialog its full height at once, so it
     does not grow while the list loads. */
  .card[data-view="thirdParty"] {
    height: calc(100vh - 34px);
    max-height: 760px;
  }
  .license {
    padding-top: var(--space-4);
  }
  .view-loading {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    min-height: 160px;
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }
  .license-text {
    padding: var(--space-4) var(--space-5);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-sunken);
    color: var(--text-secondary);
    font-size: var(--text-sm);
    line-height: 1.65;
    user-select: text;
  }
  .license-text p {
    margin: 0;
    text-wrap: pretty;
  }
  .license-text p + p {
    margin-top: var(--space-3);
  }
  .license-heading {
    color: var(--text);
    font-size: var(--text-md);
    font-weight: var(--weight-semibold);
  }
  .license-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
    margin-top: var(--space-3);
  }

  @media (max-width: 560px) {
    .choices {
      grid-template-columns: 1fr;
    }
  }
</style>
