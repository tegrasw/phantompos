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
-->
<script lang="ts">
  import { fade } from "svelte/transition";
  import { CODE_PAGES, isValidPort, PAPERS, type CodePage, type NetworkMode, type Paper, type Settings } from "../backend";
  import { i18n } from "../i18n";
  import { duration } from "../motion";
  import { errorText } from "../receiptExport";
  import type { EndpointStatus, ListenerControls } from "../status";
  import CopyButton from "./CopyButton.svelte";
  import Icon from "./Icon.svelte";
  import ListenerActions from "./ListenerActions.svelte";

  let {
    open,
    settings,
    status,
    controls,
    onChange,
    onClose,
  }: {
    open: boolean;
    settings: Settings;
    status: EndpointStatus;
    controls: ListenerControls;
    /** Optimistic update; rejects (after rolling back) when the backend refuses. */
    onChange: (patch: Partial<Settings>) => Promise<void>;
    onClose: () => void;
  } = $props();

  const t = $derived(i18n.t.settings);
  const VERSION = __APP_VERSION__;

  let dialog: HTMLDialogElement | undefined = $state();
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
    requestClose();
  }

  function onDialogClick(event: MouseEvent): void {
    if (event.target === dialog) requestClose();
  }

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
>
  <div class="card">
    <header class="head">
      <span class="glyph"><Icon name="gear" size={16} /></span>
      <div class="titles">
        <h2 id="settings-title">{t.title}</h2>
        <p class="subtitle" id="settings-subtitle">{t.subtitle}</p>
      </div>
      <button type="button" class="icon-btn" aria-label={t.close} title={t.close} onclick={requestClose}>
        <Icon name="x" />
      </button>
    </header>

    <div class="body">
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
    </div>

    <footer class="foot">
      <span class="about"><strong>{t.about(VERSION)}</strong> · {t.aboutDetail}</span>
    </footer>
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

  .foot {
    flex: none;
    padding: var(--space-2-5, 10px) var(--space-4);
    border-top: 1px solid var(--border-subtle);
    background: var(--bg-surface);
    color: var(--text-tertiary);
    font-size: var(--text-xs);
  }
  .about strong {
    color: var(--text-secondary);
    font-weight: var(--weight-semibold);
  }

  @media (max-width: 560px) {
    .choices {
      grid-template-columns: 1fr;
    }
  }
</style>
