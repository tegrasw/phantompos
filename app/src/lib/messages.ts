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
    showState: "Show printer state",
    hideState: "Hide printer state",
    /** Header chip while an error state is simulated. */
    faultChip: (primary: string, more: number): string => (more > 0 ? `${primary} +${more}` : primary),
    faultTitle: (names: string): string => `Simulated: ${names}. Data is logged but not printed. Click to open the printer state.`,
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
    statReplies: "Replies",
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
    /** Slot label while an error state is simulated; `more` other states are on too. */
    slotFault: (primary: string, more: number): string => (more > 0 ? `${primary} +${more}` : primary),
    slotHalted: "Not printing",
    idleFault: (names: string): string => `${names}: anything sent now is logged but not printed.`,
    suppressedTitle: (bytes: string, reason: string): string => `${bytes} not printed \u00B7 ${reason}`,
    suppressedDetail: (peer: string, connectionId: number): string => `${peer} \u00B7 #${connectionId}`,
    suppressedDetailTitle: (peer: string, connectionId: number): string => `From ${peer} (connection #${connectionId})`,
    suppressedReads: (n: number): string => plural(n, "read", "reads"),
    bytes: (n: number): string => plural(n, "byte", "bytes"),
    suppressedLabel: (bytes: string, reason: string, peer: string): string =>
      `${bytes} from ${peer} were not printed because of the simulated state: ${reason}`,
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
    title: "Cash drawer",
    closed: "Closed",
    open: "Open",
    closedHint: "Opens on ESC p",
    close: "Close",
    closeLabel: "Close the cash drawer",
    kicked: (pin: number, onMs: number, offMs: number): string => `Pin ${pin} \u00B7 ${onMs}/${offMs} ms`,
    kickedBy: (peer: string, pin: number, onMs: number, offMs: number): string =>
      `Kicked by ${peer}: pin ${pin}, on ${onMs} ms, off ${offMs} ms`,
    announceOpen: "Cash drawer opened",
    closeFailed: "Couldn't close the drawer",
  },

  printer: {
    title: "Printer",
    expand: "Show printer state",
    collapse: "Collapse",
    railLabel: "Printer",
    statesLabel: "Simulated states",
    /** Switch names and what each one does. */
    faults: {
      paperOut: "Paper out",
      coverOpen: "Cover open",
      offline: "Offline",
    } as Record<string, string>,
    faultHints: {
      paperOut: "Roll empty: reports paper end",
      coverOpen: "Lid lifted: reports cover open",
      offline: "Reports offline",
    } as Record<string, string>,
    /** Short lower-case form for running text ("248 bytes not printed · paper out"). */
    faultsLower: {
      paperOut: "paper out",
      coverOpen: "cover open",
      offline: "offline",
    } as Record<string, string>,
    list: (names: readonly string[]): string =>
      names.length <= 1 ? (names[0] ?? "") : `${names.slice(0, -1).join(", ")} and ${names.at(-1)}`,
    changeFailed: "Couldn't change the printer state",
    stateReady: "Ready",
    stateReadyDetail: "Printing everything it receives",
    stateHalted: "Not printing",
    stateHaltedDetail: "Data is logged, nothing is printed",
    reportsLabel: "Reports to the host",
    reports: {
      online: "Online",
      offline: "Offline",
      paper: "Paper",
      paperPresent: "Present",
      paperEnd: "Paper end",
      cover: "Cover",
      coverClosed: "Closed",
      coverOpen: "Open",
      drawer: "Drawer",
      drawerOpen: "Open",
      drawerClosed: "Closed",
      connection: "Status",
      printing: "Printing",
      printingOn: "Accepted",
      printingOff: "Discarded",
    },
    answered:
      "Status requests (DLE EOT, GS r) are answered in every state, and clients that enabled GS a get each change automatically.",
    lastReply: "Last reply",
    noReplies: "No status requests yet",
    replyCount: (n: number): string => plural(n, "reply", "replies"),
    illustration: (state: string): string => `Printer illustration: ${state}`,
  },

  code: {
    inspect: (kind: string): string => `Inspect ${kind}`,
    kinds: { qrCode: "QR code", barcode: "barcode", raster: "image" } as Record<string, string>,
    titles: { qrCode: "QR code", barcode: "Barcode", raster: "Image" } as Record<string, string>,
    fallbackKind: "image",
    fallbackTitle: "Image",
    size: (width: number, height: number): string =>
      `${width.toLocaleString("en")} \u00D7 ${height.toLocaleString("en")} dots`,
    scale: (pixels: number): string => `${pixels} px per dot`,
    source: (id: number | null): string => (id === null ? "Printing" : `Receipt #${id}`),
    hint: "Exactly as printed, enlarged without smoothing. Point a phone camera at it to scan.",
    close: "Close",
  },

  log: {
    title: "Commands",
    count: (n: number): string => plural(n, "command", "commands"),
    replies: (n: number): string => plural(n, "reply", "replies"),
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
    /** Shown after the expanded hex of a command too long to keep in full. */
    notRetained: (n: number): string => `\u2026 ${plural(n, "more byte", "more bytes")}`,
    notRetainedTitle: "Only the start of long commands is kept in the log",
    summary: {
      dots: (width: number, height: number): string =>
        `${width.toLocaleString("en")} \u00D7 ${height.toLocaleString("en")} dots`,
      barcode: (symbology: string, data: string | null, invalid: boolean): string =>
        `${symbology}${data === null ? "" : ` \u201C${data}\u201D`}${invalid ? " (invalid)" : ""}`,
      storedData: (n: number): string => `${plural(n, "byte", "bytes")} of data`,
    },
    replyLabel: "Reply",
    replyTitle: "Sent by the printer to the host",
    replyOffset: "\u2190",
    /** Short tag in the mnemonic column (it is 44 px wide). */
    replyKinds: { realTime: "Reply", transmit: "Reply", automatic: "ASB" } as Record<string, string>,
    replyKindTitles: {
      realTime: "Real-time status (DLE EOT)",
      transmit: "Transmitted status (GS r)",
      automatic: "Automatic Status Back (enabled with GS a)",
    } as Record<string, string>,
    /** Labels for the reply flag codes; unknown codes are humanised. */
    flags: {
      online: "Online",
      offline: "Offline",
      coverOpen: "Cover open",
      coverClosed: "Cover closed",
      paperEnd: "Paper end",
      paperNearEnd: "Paper near end",
      paperPresent: "Paper present",
      paperAdequate: "Paper adequate",
      paperFeedButton: "Feed button",
      paperEndStop: "Stopped: paper end",
      drawerHigh: "Drawer high",
      drawerLow: "Drawer low",
      errorOccurred: "Error",
      recoverableError: "Recoverable error",
      unrecoverableError: "Unrecoverable error",
      autoCutterError: "Cutter error",
      autoRecoverableError: "Auto-recoverable error",
      waitingForOnline: "Waiting for online",
    } as Record<string, string>,
    noFlags: "No conditions",
    replyAnnounce: (request: string, flags: string): string => `Printer replied to ${request}: ${flags}`,
    asbMask: (names: readonly string[]): string => (names.length ? names.join(", ") : "disabled"),
    asbBits: ["drawer", "online/offline", "errors", "paper"] as readonly string[],
    truncated: (n: number): string =>
      `${plural(n, "older command was", "older commands were")} removed to keep memory in check.`,
  },
} as const;
