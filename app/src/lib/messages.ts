// All user-facing UI strings live here so a later stage can add pt/en
// localisation by swapping this module for a locale-aware lookup.

export const messages = {
  appName: "PhantomPOS",

  header: {
    listening: "Listening",
    stopped: "Stopped",
    error: "Error",
    starting: "Starting…",
    notTauri: "Not running inside Tauri",
    notTauriDetail: "Open this UI through the PhantomPOS app (pnpm tauri dev).",
    stateUnavailable: "Could not read the backend state",
    copy: "Copy",
    copied: "Copied",
    copyFailed: "Copy failed",
    copyTitle: "Copy the listening address to the clipboard",
    connections: (n: number): string =>
      n === 1 ? "1 connection" : `${n} connections`,
  },

  log: {
    title: "Raw bytes",
    clear: "Clear",
    jumpToLatest: "Jump to latest",
    emptyTitle: "Waiting for data",
    emptyHint: "Send some bytes to the printer port to see them here, e.g.:",
    emptyCommand: (host: string, port: number): string =>
      `printf 'Hello\\n' | nc ${host} ${port}`,
    connectionLabel: (id: number): string => `conn #${id}`,
    byteCount: (n: number): string => (n === 1 ? "1 byte" : `${n} bytes`),
    truncated: (dropped: number): string =>
      `${dropped} older ${dropped === 1 ? "chunk was" : "chunks were"} discarded to limit memory use.`,
  },
} as const;
