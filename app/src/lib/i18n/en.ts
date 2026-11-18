// English catalogue. Its shape is the `Messages` type: the Portuguese
// catalogue must provide exactly the same keys (the compiler enforces it).
//
// Maps typed `Vocabulary` are open vocabularies keyed by identifiers from the
// backend (command types, parameter names, reply flags); a missing entry falls
// back to the humanised identifier (see escpos.ts). ESC/POS mnemonics ("GS v 0",
// "DLE EOT 1"), code page names and raw enumeration values are never
// translated.

import { formatCount, formatList, plural } from "../format";

/** Labels for backend identifiers; anything missing is humanised. */
export type Vocabulary = Readonly<Record<string, string>>;

const dots = (width: number, height: number): string => `${formatCount(width)} × ${formatCount(height)} dots`;

export const en = {
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
    copyAddress: "Copy address",
    copyAddressOf: (address: string): string => `Copy ${address}`,
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
    settings: "Settings",
    /** Header chip while an error state is simulated. */
    faultChip: (primary: string, more: number): string => (more > 0 ? `${primary} +${more}` : primary),
    faultTitle: (names: string): string =>
      `Simulated: ${names}. Data is logged but not printed. Click to open the printer state.`,
    network: "Network",
    networkTitle: "Network mode: any device on your local network can send print jobs. Click to change.",
    moreAddresses: (n: number): string => `+${n}`,
    moreAddressesLabel: (n: number): string => `Show ${plural(n, "more address", "more addresses")}`,
    addressesTitle: "Printer addresses",
    addressesHint: "Any of these reaches this printer from another device.",
    noNetworkAddress: "No network address",
    noNetworkAddressTitle:
      "This computer has no IPv4 address besides loopback. Programs on this computer can still print to 127.0.0.1.",
  },

  /** The TCP listener's error states, by `code`. */
  listener: {
    errorShort: {
      portInUse: (port: number): string => `Port ${port} is in use`,
      permissionDenied: (port: number): string => `No permission for port ${port}`,
      other: (port: number): string => `Couldn't open port ${port}`,
    },
    errorTitle: {
      portInUse: (port: number): string => `Port ${port} is already in use`,
      permissionDenied: (port: number): string => `PhantomPOS can't use port ${port}`,
      other: (port: number): string => `Couldn't open port ${port}`,
    },
    errorBody: {
      portInUse:
        "Another program is listening on it (Flutter DevTools, for example, also uses 9100 by default). Switch to a free port, or close that program and try again.",
      permissionDenied:
        "The system refused access to this port. Ports below 1024 usually need administrator rights, so choose another port.",
      other: "The system reported an error while opening the port. Try again, or choose another port.",
    },
    usePort: (port: number): string => `Use port ${port}`,
    usePortShort: (port: number): string => `Use ${port}`,
    usePortTitle: (port: number): string => `Switch PhantomPOS to port ${port}, which is free right now`,
    retry: "Try again",
    retryTitle: "Open the same port again",
    stillUnavailable: "Still unavailable",
    openSettings: "Open settings",
    restarting: "Restarting the listener",
    actionFailed: "Couldn't restart the listener",
    details: "Details",
  },

  roll: {
    readyTitle: "Ready for your first print",
    readyAgainTitle: "Ready for the next print",
    readyBody:
      "Point your point-of-sale system at this address. PhantomPOS accepts raw ESC/POS over TCP, exactly like a networked thermal printer.",
    readyBodyNetwork:
      "Point your point-of-sale system at any of these addresses. PhantomPOS accepts raw ESC/POS over TCP, exactly like a networked thermal printer.",
    addressLabel: "Printer address",
    addressesLabel: "Printer addresses",
    networkBadge: "Local network",
    noAddressValue: "No network address found",
    noAddressHint: (local: string): string =>
      `Connect this computer to a network to print from other devices. Programs on this computer can use ${local}.`,
    testLabel: "Send a test print",
    testCommand: (host: string, port: number): string =>
      `printf '\\033@Hello from PhantomPOS\\n' | nc ${host} ${port}`,
    copyCommand: "Copy command",
    startingTitle: "Starting the printer",
    startingBody: "Opening the port. This only takes a moment.",
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
    slotCommands: (n: number): string => `${formatCount(n)} cmd`,
    idleFault: (names: string): string => `${names}: anything sent now is logged but not printed.`,
    suppressedTitle: (bytes: string, reason: string): string => `${bytes} not printed · ${reason}`,
    suppressedDetail: (peer: string, connectionId: number): string => `${peer} · #${connectionId}`,
    suppressedDetailTitle: (peer: string, connectionId: number): string =>
      `From ${peer} (connection #${connectionId})`,
    suppressedReads: (n: number): string => plural(n, "read", "reads"),
    bytes: (n: number): string => plural(n, "byte", "bytes"),
    suppressedLabel: (bytes: string, reason: string, peer: string): string =>
      `${bytes} from ${peer} were not printed because of the simulated state: ${reason}`,
  },

  /** Saving a completed receipt (menu on each receipt, then a notice). */
  receiptExport: {
    menu: (id: number): string => `Save receipt #${id}`,
    menuTitle: "Save this receipt",
    png: "Save as PNG",
    pngDetail: (width: number, height: number): string => `${dots(width, height)}, black and white`,
    bin: "Export raw bytes",
    binDetail: ".bin, exactly as received; works as a test fixture",
    saving: "Saving…",
    saved: (name: string): string => `Saved ${name}`,
    savedPng: "PNG image",
    savedBin: "Raw bytes",
    unavailable: "This receipt is no longer available",
    unavailableDetail: "PhantomPOS keeps only recent receipts. Print it again to save it.",
    failed: "Couldn't save the receipt",
    tooTall: (max: string): string => `This receipt is too tall for a PNG here (the limit is ${max} dots).`,
    renderFailed: "Couldn't draw the image",
  },

  paper: {
    label: "Paper",
    groupLabel: "Paper width",
    names: { "58mm": "58 mm", "80mm": "80 mm" } as Vocabulary,
    hint: "Applies to the next receipt",
    changed: (name: string): string => `Next receipt prints on ${name}`,
    failed: "Couldn't change the paper",
  },

  settings: {
    title: "Settings",
    subtitle: "Changes are saved and kept between launches",
    close: "Close",
    connection: "Connection",
    port: "Port",
    portHint: "TCP port the printer listens on. Applying restarts the listener and drops open connections.",
    portPlaceholder: "9100",
    apply: "Apply",
    applying: "Applying",
    portInvalid: "Enter a whole number from 1 to 65535",
    portUnchanged: "Already using this port",
    portListening: (port: number): string => `Listening on port ${port}`,
    portStopped: (port: number): string => `Not listening on port ${port}`,
    portStarting: "Starting",
    access: "Accept print jobs from",
    accessLocal: "This computer",
    accessLocalHint: "Only programs on this computer (127.0.0.1)",
    accessNetwork: "Local network",
    accessNetworkHint: "Any device that can reach this computer",
    confirmTitle: "Allow print jobs from your network?",
    confirmBody:
      "Any device on your local network will be able to send print jobs to PhantomPOS. Like a real network printer, connections have no authentication and no encryption.",
    confirmAllow: "Allow network access",
    confirmCancel: "Keep local only",
    reachableAt: "Reachable at",
    noAddresses: "No network address found. Connect this computer to a network to print from other devices.",
    printer: "Printer",
    paper: "Paper width",
    paperHint: "Applies to the next receipt.",
    codePage: "Default code page",
    codePageHint: "Used until a client selects one with ESC t. Applies to new connections.",
    codePages: {
      cp437: "US, standard Europe",
      cp850: "Multilingual (Latin-1)",
      cp860: "Portuguese",
      cp858: "Multilingual with euro",
      windows1252: "Western European (Windows)",
    },
    saveFailed: "Couldn't save this setting",
    about: (version: string): string => `PhantomPOS ${version}`,
    aboutDetail: "Virtual ESC/POS thermal printer",
  },

  drawer: {
    title: "Cash drawer",
    closed: "Closed",
    open: "Open",
    closedHint: "Opens on ESC p",
    close: "Close",
    closeLabel: "Close the cash drawer",
    kicked: (pin: number, onMs: number, offMs: number): string => `Pin ${pin} · ${onMs}/${offMs} ms`,
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
    } as Vocabulary,
    faultHints: {
      paperOut: "Roll empty: reports paper end",
      coverOpen: "Lid lifted: reports cover open",
      offline: "Reports offline",
    } as Vocabulary,
    /** Short lower-case form for running text ("248 bytes not printed · paper out"). */
    faultsLower: {
      paperOut: "paper out",
      coverOpen: "cover open",
      offline: "offline",
    } as Vocabulary,
    list: (names: readonly string[]): string => formatList(names),
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
    kinds: { qrCode: "QR code", barcode: "barcode", raster: "image" } as Vocabulary,
    titles: { qrCode: "QR code", barcode: "Barcode", raster: "Image" } as Vocabulary,
    fallbackKind: "image",
    fallbackTitle: "Image",
    size: dots,
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
    newRows: (n: number): string => plural(n, "new command", "new commands"),
    emptyTitle: "No commands yet",
    emptyBody:
      "Everything the printer receives is decoded here, byte for byte. Unrecognised sequences are highlighted.",
    connection: (id: number): string => `#${id}`,
    connectionTitle: (id: number, peer: string): string => `Connection ${id} from ${peer}`,
    groupSummary: (commands: number, bytes: string): string =>
      `${plural(commands, "command", "commands")} · ${bytes}`,
    unknownCount: (n: number): string => `${formatCount(n)} unknown`,
    unknownLabel: "Unknown sequence",
    textLabel: "Text",
    textTitle: (chars: number): string => plural(chars, "character", "characters"),
    spaceTitle: (n: number): string => plural(n, "space", "spaces"),
    controlTitle: (name: string): string => `Control character ${name}`,
    moreBytes: (n: number): string => `+${plural(n, "byte", "bytes")}`,
    lessBytes: "Show less",
    /** Shown after the expanded hex of a command too long to keep in full. */
    notRetained: (n: number): string => `… ${plural(n, "more byte", "more bytes")}`,
    notRetainedTitle: "Only the start of long commands is kept in the log",
    summary: {
      dots,
      barcode: (symbology: string, data: string | null, invalid: boolean): string =>
        `${symbology}${data === null ? "" : ` “${data}”`}${invalid ? " (invalid)" : ""}`,
      storedData: (n: number): string => `${plural(n, "byte", "bytes")} of data`,
    },
    /** Boolean parameter values. */
    on: "on",
    off: "off",
    /**
     * Command names by `type`. English uses the humanised identifier
     * ("setCharacterSize" → "Set character size"), so only exceptions go here.
     */
    commandTypes: {} as Vocabulary,
    /** Parameter labels by key; English humanises the key. */
    params: {} as Vocabulary,
    /** `unknown.reason` values. */
    reasons: {} as Vocabulary,
    /** What a status request asks for (DLE EOT / GS r parameters). */
    statusRequests: {} as Vocabulary,
    replyLabel: "Reply",
    replyTitle: "Sent by the printer to the host",
    replyOffset: "←",
    /** Short tag in the mnemonic column (it is 44 px wide). */
    replyKinds: { realTime: "Reply", transmit: "Reply", automatic: "ASB" } as Vocabulary,
    replyKindTitles: {
      realTime: "Real-time status (DLE EOT)",
      transmit: "Transmitted status (GS r)",
      automatic: "Automatic Status Back (enabled with GS a)",
    } as Vocabulary,
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
    } as Vocabulary,
    noFlags: "No conditions",
    replyAnnounce: (request: string, flags: string): string => `Printer replied to ${request}: ${flags}`,
    asbMask: (names: readonly string[]): string => (names.length ? names.join(", ") : "disabled"),
    asbBits: ["drawer", "online/offline", "errors", "paper"] as readonly string[],
    truncated: (n: number): string =>
      `${plural(n, "older command was", "older commands were")} removed to keep memory in check.`,
  },

  toast: {
    dismiss: "Dismiss",
    region: "Notifications",
  },
};

export type Messages = typeof en;
