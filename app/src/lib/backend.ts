// Typed access to the Tauri backend. Every event name, command name and
// payload shape the UI depends on is declared here, so the rest of the UI never
// imports `@tauri-apps/api` directly.

import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// ---------------------------------------------------------------------------
// Payloads (camelCase JSON, mirrored by the Rust side)
// ---------------------------------------------------------------------------

export type ListenerState = "listening" | "error" | "stopped";

/** Payload of `listener:status`. */
export interface ListenerStatus {
  state: ListenerState;
  address: string;
  port: number;
  /** Present when `state` is `"error"`, e.g. "port already in use". */
  message?: string;
}

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

/** Result of the `get_state` command. */
export interface BackendState {
  listener: ListenerStatus;
  connections: ConnectionsStatus;
  paper: Paper;
  drawerOpen: boolean;
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
} as const;

export const COMMANDS = {
  getState: "get_state",
  setPaper: "set_paper",
  closeDrawer: "close_drawer",
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

export function setPaper(paper: Paper): Promise<Paper> {
  return invoke<Paper>(COMMANDS.setPaper, { paper });
}

export function closeDrawer(): Promise<void> {
  return invoke<void>(COMMANDS.closeDrawer);
}

export interface BackendHandlers {
  onListenerStatus(status: ListenerStatus): void;
  onConnectionsChanged(connections: ConnectionsStatus): void;
  onCommandsReceived(batch: CommandsReceived): void;
  onReceiptUpdated(update: ReceiptUpdated): void;
  onReceiptCompleted(receipt: ReceiptCompleted): void;
  onDrawerKicked(kick: DrawerKicked): void;
  /** Called with the initial snapshot once all listeners are registered. */
  onInitialState(state: BackendState): void;
  /** Called if the initial snapshot or a listener registration fails. */
  onError(error: unknown): void;
}

/** A connected backend: the commands the UI may call, plus cleanup. */
export interface BackendSession {
  setPaper(paper: Paper): Promise<Paper>;
  closeDrawer(): Promise<void>;
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
      ]);
      if (disposed) return;
      const state = await getState();
      if (!disposed) handlers.onInitialState(state);
    } catch (error) {
      if (!disposed) handlers.onError(error);
    }
  })();

  return {
    setPaper,
    closeDrawer,
    disconnect: () => {
      disposed = true;
      for (const unlisten of unlisteners.splice(0)) unlisten();
    },
  };
};

/** Formats `address:port`, bracketing IPv6 literals. */
export function formatAddress(address: string, port: number): string {
  return address.includes(":") ? `[${address}]:${port}` : `${address}:${port}`;
}
