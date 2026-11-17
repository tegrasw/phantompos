// All user-facing UI strings live here so a later stage can add pt/en
// localisation by swapping this module for a locale-aware lookup.

const plural = (n: number, one: string, many: string): string =>
  `${n.toLocaleString("en")} ${n === 1 ? one : many}`;

export const messages = {
  appName: "PhantomPOS",
  appTagline: "Virtual ESC/POS printer",

  header: {
    listening: "Listening",
    stopped: "Stopped",
    error: "Unavailable",
    starting: "Starting",
    disconnected: "No backend",
    disconnectedDetail: "Open this window from the PhantomPOS app",
    stateUnavailable: "Could not read the printer state",
    listenerStopped: "Not accepting connections",
    copyAddress: "Copy address",
    copied: "Copied",
    copyFailed: "Couldn't copy",
    noConnections: "No connections",
    connections: (n: number): string => plural(n, "connection", "connections"),
    connectedPeers: (peers: readonly string[]): string =>
      peers.length > 0 ? `Connected: ${peers.join(", ")}` : "No clients connected",
    showLog: "Show command log",
    hideLog: "Hide command log",
  },

  roll: {
    readyTitle: "Ready for your first print",
    readyAgainTitle: "Ready for the next print",
    readyBody:
      "Point your point-of-sale system at this address. PhantomPOS accepts raw ESC/POS over TCP, exactly like a networked thermal printer.",
    addressLabel: "Printer address",
    testLabel: "Send a test print",
    testCommand: (host: string, port: number): string =>
      `printf '\\033@Hello from PhantomPOS\\n' | nc ${host} ${port}`,
    copyCommand: "Copy command",
    startingTitle: "Starting the printer",
    startingBody: "Opening the port. This only takes a moment.",
    errorTitle: (port: number): string => `Port ${port} is not available`,
    errorBody:
      "Another program is probably using this port. Close it and restart PhantomPOS. Flutter DevTools, for example, also listens on 9100 by default.",
    stoppedTitle: "The printer is not listening",
    stoppedBody: "PhantomPOS stopped accepting connections on this address.",
    disconnectedTitle: "Not connected to PhantomPOS",
    disconnectedBody:
      "This interface needs the PhantomPOS backend. Run it with pnpm tauri dev, or open the installed app.",
    sessionTitle: "This session",
    statCommands: "Commands",
    statBytes: "Received",
    statUnknown: "Unknown",
    statLast: "Last data",
    lastFrom: (peer: string): string => `from ${peer}`,

    // The roll, once receipts exist.
    rollLabel: "Paper roll",
    toolbarLabel: "Roll",
    receiptCount: (n: number): string => plural(n, "receipt", "receipts"),
    clear: "Clear roll",
    clearDisabled: "The roll is empty",
    jumpToLatest: "Jump to latest",
    newReceipts: (n: number): string => plural(n, "new receipt", "new receipts"),
    dropped: (n: number): string =>
      `${plural(n, "older receipt was", "older receipts were")} removed to keep memory in check.`,
    receiptNumber: (id: number): string => `#${id}`,
    printing: "Printing",
    cutFull: "Full cut",
    cutPartial: "Partial cut",
    cutNone: "Connection closed, no cut",
    cutNoneShort: "No cut",
    receiptLabel: (id: number | null, paper: string, cut: string): string =>
      `${id === null ? "Receipt being printed" : `Receipt ${id}`}, ${paper} paper, ${cut}`,
    receiptTitle: (peer: string, connectionId: number): string => `From ${peer} (connection #${connectionId})`,
    slotReady: "Ready",
    slotPrinting: (n: number): string => (n > 1 ? `Printing ${n} receipts` : "Printing"),
    slotOffline: "Not listening",
  },

  paper: {
    label: "Paper",
    groupLabel: "Paper width",
    names: { "58mm": "58 mm", "80mm": "80 mm" } as Record<string, string>,
    hint: "Applies to the next receipt",
    changed: (name: string): string => `Next receipt prints on ${name}`,
    failed: "Couldn't change the paper",
  },

  drawer: {
    closed: "Drawer closed",
    open: "Drawer open",
    closedTitle: "Cash drawer is closed. It opens when the printer receives ESC p.",
    openTitle: "Cash drawer is open. Click to close it.",
    closeLabel: "Cash drawer is open. Close it",
    closedLabel: "Cash drawer is closed",
    kickedBy: (peer: string, pin: number, onMs: number, offMs: number): string =>
      `Last kick from ${peer}: pin ${pin}, on ${onMs} ms, off ${offMs} ms.`,
    announceOpen: "Cash drawer opened",
    announceClosed: "Cash drawer closed",
    closeFailed: "Couldn't close the drawer",
  },

  log: {
    title: "Commands",
    count: (n: number): string => plural(n, "command", "commands"),
    clear: "Clear log",
    collapse: "Collapse",
    expand: "Show command log",
    resize: "Resize command log",
    columnOffset: "Offset",
    columnBytes: "Bytes",
    columnCommand: "Command",
    jumpToLatest: "Jump to latest",
    newRows: (n: number): string => `${plural(n, "new command", "new commands")}`,
    emptyTitle: "No commands yet",
    emptyBody:
      "Everything the printer receives is decoded here, byte for byte. Unrecognised sequences are highlighted.",
    connection: (id: number): string => `#${id}`,
    connectionTitle: (id: number, peer: string): string => `Connection ${id} from ${peer}`,
    groupSummary: (commands: number, bytes: string): string =>
      `${plural(commands, "command", "commands")} · ${bytes}`,
    unknownCount: (n: number): string => `${n.toLocaleString("en")} unknown`,
    unknownLabel: "Unknown sequence",
    textLabel: "Text",
    textTitle: (chars: number): string => plural(chars, "character", "characters"),
    spaceTitle: (n: number): string => plural(n, "space", "spaces"),
    controlTitle: (name: string): string => `Control character ${name}`,
    moreBytes: (n: number): string => `+${plural(n, "byte", "bytes")}`,
    lessBytes: "Show less",
    truncated: (n: number): string =>
      `${plural(n, "older command was", "older commands were")} removed to keep memory in check.`,
  },
} as const;
