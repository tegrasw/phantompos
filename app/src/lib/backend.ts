// Typed access to the Tauri backend. Every event name, command name and
// payload shape the UI depends on is declared here, so the rest of the UI never
// imports `@tauri-apps/api` directly.

import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// ---------------------------------------------------------------------------
// Payloads (camelCase JSON, mirrored by the Rust side)
// ---------------------------------------------------------------------------

export type ListenerState = "listening" | "error" | "stopped";

/** Why the listener could not bind; the UI words each one itself. */
export type ListenerErrorCode = "portInUse" | "permissionDenied" | "other";

/** Payload of `listener:status`. */
export interface ListenerStatus {
  state: ListenerState;
  /** Bind address ("127.0.0.1" in local mode, "0.0.0.0" in network mode). */
  address: string;
  port: number;
  /**
   * Addresses to show: ["127.0.0.1"] in local mode; in network mode every
   * non-loopback IPv4 address of the machine (may be empty).
   */
  addresses?: string[];
  /** English description of an error, for logs (the UI words `code`). */
  message?: string;
  code?: ListenerErrorCode | (string & {});
  /** With `portInUse`: a port that was free when the error happened. */
  suggestedPort?: number;
}

export type NetworkMode = "local" | "network";
export type CodePage = "cp437" | "cp850" | "cp860" | "cp858" | "windows1252";

/** The code pages a user can pick as default, with their usual names. */
export const CODE_PAGES: readonly { id: CodePage; name: string }[] = [
  { id: "cp437", name: "CP437" },
  { id: "cp850", name: "CP850" },
  { id: "cp860", name: "CP860" },
  { id: "cp858", name: "CP858" },
  { id: "windows1252", name: "Windows-1252" },
];

/**
 * Persistent settings (`get_state().settings`, `settings:changed`, result of
 * `update_settings`). `port` and `networkMode` restart only the listener;
 * `paper` applies to the next receipt, `defaultCodePage` to the next
 * connection.
 */
export interface Settings {
  port: number;
  networkMode: NetworkMode;
  paper: Paper;
  defaultCodePage: CodePage;
}

export const DEFAULT_SETTINGS: Settings = {
  port: 9100,
  networkMode: "local",
  paper: "80mm",
  defaultCodePage: "cp437",
};

export const PORT_MIN = 1;
export const PORT_MAX = 65535;

/** Result of `export_receipt`: the saved path, or `saved: false` if cancelled. */
export type ExportResult = { saved: true; path: string } | { saved: false };
export type ExportFormat = "png" | "bin";

/**
 * The external pages the About section opens (`open_link`). The backend maps
 * each id to a fixed address and opens it in the system browser; the webview
 * never passes a URL.
 */
export type ExternalLink = "website" | "repository" | "issues" | "license";

/** Payload of `connection:changed`. */
export interface ConnectionsStatus {
  active: number;
  peers: string[];
}

/** Scalar parameter of a decoded command. */
export type CommandParam = string | number | boolean | null;

/**
 * A decoded ESC/POS command. `type` is the camelCase variant name produced by
 * `phantompos-core`; the remaining fields are its parameters. Only `text`
 * (with `text`) and `unknown` (with an optional `reason`) are guaranteed; the
 * UI renders every other variant generically.
 */
export interface CommandBody {
  type: string;
  [param: string]: CommandParam | undefined;
}

export interface DecodedCommand {
  /** Offset of the first byte within the connection's byte stream. */
  offset: number;
  /** The raw bytes this command was decoded from. */
  bytes: number[];
  command: CommandBody;
}

/** Payload of `commands:received`: one batch of decoded commands. */
export interface CommandsReceived {
  connectionId: number;
  peer: string;
  commands: DecodedCommand[];
}

// ---------------------------------------------------------------------------
// Receipt model (see core/README.md). All sizes are printer dots.
// ---------------------------------------------------------------------------

export type Paper = "58mm" | "80mm";
export const PAPERS: readonly Paper[] = ["58mm", "80mm"];

/** How a receipt was closed: a cut command, or `null` for end of connection. */
export type Cut = "full" | "partial" | null;

export type Alignment = "left" | "center" | "right";
export type UnderlineMode = "off" | "oneDot" | "twoDots";

export interface SpanStyle {
  font: "a" | "b";
  bold: boolean;
  underline: UnderlineMode;
  reverse: boolean;
  widthMultiplier: number;
  heightMultiplier: number;
}

export interface Span {
  text: string;
  /** Left edge from the paper edge, alignment applied. */
  x: number;
  /** Top of the character cells inside the line. */
  y: number;
  width: number;
  columns: number;
  /** Cell size of one character, multipliers applied. */
  charWidth: number;
  charHeight: number;
  style: SpanStyle;
}

export interface LineElement {
  type: "line";
  y: number;
  height: number;
  baseline: number;
  advance: number;
  alignment: Alignment;
  contentWidth: number;
  spans: Span[];
}

/** What produced an image element; unknown kinds are drawn the same way. */
export type ImageKind = "raster" | "qrCode" | "barcode";

/**
 * A 1-bit bitmap on the paper: a raster image (`GS v 0`), a QR code or a
 * barcode's bars (core generates codes as bitmaps; barcode HRI text arrives as
 * ordinary line elements).
 */
export interface ImageElement {
  type: "image";
  kind: ImageKind | (string & {});
  /** Left edge from the paper edge, alignment applied. */
  x: number;
  y: number;
  width: number;
  height: number;
  /** Dots from `y` to the next element. */
  advance: number;
  /**
   * Base64 (standard alphabet, padded) of packed rows, top to bottom: each
   * row is ceil(width / 8) bytes, most significant bit first, 1 = black dot.
   */
  data: string;
}

/**
 * Something printed on the paper. The UI ignores element types it does not
 * know.
 */
export type ReceiptElement = LineElement | ImageElement | { type: string; y?: number };

export interface Receipt {
  paper: Paper;
  /** Paper width in dots (384 or 576). */
  width: number;
  /** Paper length in dots. */
  height: number;
  /** Missing on older backends; treat as `null`. */
  cut?: Cut;
  elements: ReceiptElement[];
}

/** Payload of `receipt:updated`: the in-progress receipt of a connection. */
export interface ReceiptUpdated {
  connectionId: number;
  peer: string;
  receipt: Receipt;
}

/** Payload of `receipt:completed`. `id` is unique and increasing. */
export interface ReceiptCompleted {
  id: number;
  connectionId: number;
  peer: string;
  receipt: Receipt;
}

/** Payload of `drawer:kicked` (`ESC p`). */
export interface DrawerKicked {
  connectionId: number;
  peer: string;
  pin: 2 | 5;
  onMs: number;
  offMs: number;
}

/**
 * The simulated printer state (`get_state().status`, `printer:status`, the
 * result of `set_printer_status`). `paperOut`, `coverOpen` and `offline` are
 * error states: while any of them is on, received data is logged but not
 * printed (see `print:suppressed`). `drawerOpen` never affects printing.
 */
export interface PrinterStatus {
  paperOut: boolean;
  coverOpen: boolean;
  offline: boolean;
  drawerOpen: boolean;
}

/** The error states, in the order the UI names them. */
export type PrinterFault = "coverOpen" | "paperOut" | "offline";
export const PRINTER_FAULTS: readonly PrinterFault[] = ["coverOpen", "paperOut", "offline"];

export const NORMAL_STATUS: PrinterStatus = {
  paperOut: false,
  coverOpen: false,
  offline: false,
  drawerOpen: false,
};

/** How a reply reached the host: `DLE EOT`, `GS r`, or Automatic Status Back. */
export type ReplyKind = "realTime" | "transmit" | "automatic";

/** Payload of `status:sent`: one reply the printer wrote to the socket. */
export interface StatusSent {
  connectionId: number;
  peer: string;
  /** Mnemonic of what was answered, e.g. "DLE EOT 1", "GS r 2" or "ASB". */
  request: string;
  reply: {
    kind: ReplyKind | (string & {});
    bytes: number[];
    /**
     * Stable camelCase codes of the conditions reported ("online",
     * "coverOpen", "paperEnd", "drawerHigh"...). The vocabulary may grow; the
     * UI humanises codes it does not know.
     */
    flags: string[];
  };
}

/** Payload of `print:suppressed`: data logged but not printed. */
export interface PrintSuppressed {
  connectionId: number;
  peer: string;
  reason: PrinterFault | (string & {});
  /** Bytes discarded in this socket read. */
  bytes: number;
}

/** Result of the `get_state` command. */
export interface BackendState {
  listener: ListenerStatus;
  connections: ConnectionsStatus;
  settings: Settings;
  status: PrinterStatus;
}

// ---------------------------------------------------------------------------
// Names
// ---------------------------------------------------------------------------

export const EVENTS = {
  listenerStatus: "listener:status",
  connectionChanged: "connection:changed",
  commandsReceived: "commands:received",
  receiptUpdated: "receipt:updated",
  receiptCompleted: "receipt:completed",
  drawerKicked: "drawer:kicked",
  printerStatus: "printer:status",
  statusSent: "status:sent",
  printSuppressed: "print:suppressed",
  settingsChanged: "settings:changed",
} as const;

export const COMMANDS = {
  getState: "get_state",
  setPrinterStatus: "set_printer_status",
  updateSettings: "update_settings",
  retryListener: "retry_listener",
  exportReceipt: "export_receipt",
  clearReceipts: "clear_receipts",
  openLink: "open_link",
} as const;

// ---------------------------------------------------------------------------
// API
// ---------------------------------------------------------------------------

/** True when the page runs inside a Tauri webview (false in a plain browser). */
export function runningInTauri(): boolean {
  return isTauri();
}

export function getState(): Promise<BackendState> {
  return invoke<BackendState>(COMMANDS.getState);
}

/** Changes some settings; resolves with all of them, rejects on invalid values. */
export function updateSettings(settings: Partial<Settings>): Promise<Settings> {
  return invoke<Settings>(COMMANDS.updateSettings, { settings });
}

/** Binds the listener again with the current settings (`listener:status` follows). */
export function retryListener(): Promise<void> {
  return invoke<void>(COMMANDS.retryListener);
}

/**
 * Saves a completed receipt through the native save dialog. `png` carries the
 * PNG file bytes for `format: "png"`; `.bin` is written from the receipt's
 * own bytes by the backend.
 */
export function exportReceipt(id: number, format: ExportFormat, png?: Uint8Array): Promise<ExportResult> {
  return invoke<ExportResult>(COMMANDS.exportReceipt, {
    id,
    format,
    ...(png ? { png: Array.from(png) } : {}),
  });
}

/** Tells the backend the roll was cleared (it may drop the receipts' bytes). */
export function clearReceipts(): Promise<void> {
  return invoke<void>(COMMANDS.clearReceipts);
}

/** Opens one of the fixed About links in the system browser; rejects with a message. */
export function openLink(link: ExternalLink): Promise<void> {
  return invoke<void>(COMMANDS.openLink, { link });
}

/** Changes some simulated states; resolves with the whole new state. */
export function setPrinterStatus(status: Partial<PrinterStatus>): Promise<PrinterStatus> {
  return invoke<PrinterStatus>(COMMANDS.setPrinterStatus, { status });
}

export interface BackendHandlers {
  onListenerStatus(status: ListenerStatus): void;
  onConnectionsChanged(connections: ConnectionsStatus): void;
  onCommandsReceived(batch: CommandsReceived): void;
  onReceiptUpdated(update: ReceiptUpdated): void;
  onReceiptCompleted(receipt: ReceiptCompleted): void;
  onDrawerKicked(kick: DrawerKicked): void;
  onPrinterStatus(status: PrinterStatus): void;
  onStatusSent(reply: StatusSent): void;
  onPrintSuppressed(suppressed: PrintSuppressed): void;
  onSettingsChanged(settings: Settings): void;
  /** Called with the initial snapshot once all listeners are registered. */
  onInitialState(state: BackendState): void;
  /** Called if the initial snapshot or a listener registration fails. */
  onError(error: unknown): void;
}

/** A connected backend: the commands the UI may call, plus cleanup. */
export interface BackendSession {
  setPrinterStatus(status: Partial<PrinterStatus>): Promise<PrinterStatus>;
  updateSettings(settings: Partial<Settings>): Promise<Settings>;
  retryListener(): Promise<void>;
  exportReceipt(id: number, format: ExportFormat, png?: Uint8Array): Promise<ExportResult>;
  clearReceipts(): Promise<void>;
  openLink(link: ExternalLink): Promise<void>;
  /** Synchronous cleanup, safe to call at any time. */
  disconnect(): void;
}

/** Connects a set of handlers to a backend. */
export type BackendConnector = (handlers: BackendHandlers) => BackendSession;

/**
 * Subscribes to all backend events and then fetches the initial state, so no
 * event emitted in between is missed. The returned `disconnect` is safe to
 * call at any time, even before the subscriptions resolve.
 */
export const connectBackend: BackendConnector = (handlers) => {
  let disposed = false;
  const unlisteners: UnlistenFn[] = [];

  const track = (unlisten: UnlistenFn): void => {
    if (disposed) unlisten();
    else unlisteners.push(unlisten);
  };

  void (async () => {
    try {
      await Promise.all([
        listen<ListenerStatus>(EVENTS.listenerStatus, (e) => {
          if (!disposed) handlers.onListenerStatus(e.payload);
        }).then(track),
        listen<ConnectionsStatus>(EVENTS.connectionChanged, (e) => {
          if (!disposed) handlers.onConnectionsChanged(e.payload);
        }).then(track),
        listen<CommandsReceived>(EVENTS.commandsReceived, (e) => {
          if (!disposed) handlers.onCommandsReceived(e.payload);
        }).then(track),
        listen<ReceiptUpdated>(EVENTS.receiptUpdated, (e) => {
          if (!disposed) handlers.onReceiptUpdated(e.payload);
        }).then(track),
        listen<ReceiptCompleted>(EVENTS.receiptCompleted, (e) => {
          if (!disposed) handlers.onReceiptCompleted(e.payload);
        }).then(track),
        listen<DrawerKicked>(EVENTS.drawerKicked, (e) => {
          if (!disposed) handlers.onDrawerKicked(e.payload);
        }).then(track),
        listen<PrinterStatus>(EVENTS.printerStatus, (e) => {
          if (!disposed) handlers.onPrinterStatus(e.payload);
        }).then(track),
        listen<StatusSent>(EVENTS.statusSent, (e) => {
          if (!disposed) handlers.onStatusSent(e.payload);
        }).then(track),
        listen<PrintSuppressed>(EVENTS.printSuppressed, (e) => {
          if (!disposed) handlers.onPrintSuppressed(e.payload);
        }).then(track),
        listen<Settings>(EVENTS.settingsChanged, (e) => {
          if (!disposed) handlers.onSettingsChanged(e.payload);
        }).then(track),
      ]);
      if (disposed) return;
      const state = await getState();
      if (!disposed) handlers.onInitialState(state);
    } catch (error) {
      if (!disposed) handlers.onError(error);
    }
  })();

  return {
    setPrinterStatus,
    updateSettings,
    retryListener,
    exportReceipt,
    clearReceipts,
    openLink,
    disconnect: () => {
      disposed = true;
      for (const unlisten of unlisteners.splice(0)) unlisten();
    },
  };
};

/** The active error states, in display order (empty when printing normally). */
export function activeFaults(status: PrinterStatus): PrinterFault[] {
  return PRINTER_FAULTS.filter((fault) => status[fault]);
}

/** True for a port the backend accepts. */
export function isValidPort(port: number): boolean {
  return Number.isInteger(port) && port >= PORT_MIN && port <= PORT_MAX;
}

/** Formats `address:port`, bracketing IPv6 literals. */
export function formatAddress(address: string, port: number): string {
  return address.includes(":") ? `[${address}]:${port}` : `${address}:${port}`;
}
