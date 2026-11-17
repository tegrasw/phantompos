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

/** Result of the `get_state` command. */
export interface BackendState {
  listener: ListenerStatus;
  connections: ConnectionsStatus;
}

// ---------------------------------------------------------------------------
// Names
// ---------------------------------------------------------------------------

export const EVENTS = {
  listenerStatus: "listener:status",
  connectionChanged: "connection:changed",
  commandsReceived: "commands:received",
} as const;

export const COMMANDS = {
  getState: "get_state",
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

export interface BackendHandlers {
  onListenerStatus(status: ListenerStatus): void;
  onConnectionsChanged(connections: ConnectionsStatus): void;
  onCommandsReceived(batch: CommandsReceived): void;
  /** Called with the initial snapshot once all listeners are registered. */
  onInitialState(state: BackendState): void;
  /** Called if the initial snapshot or a listener registration fails. */
  onError(error: unknown): void;
}

/** Connects a set of handlers to a backend; returns a synchronous cleanup. */
export type BackendConnector = (handlers: BackendHandlers) => () => void;

/**
 * Subscribes to all backend events and then fetches the initial state, so no
 * event emitted in between is missed. Returns a synchronous cleanup function
 * that is safe to call at any time, even before the subscriptions resolve.
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
      ]);
      if (disposed) return;
      const state = await getState();
      if (!disposed) handlers.onInitialState(state);
    } catch (error) {
      if (!disposed) handlers.onError(error);
    }
  })();

  return () => {
    disposed = true;
    for (const unlisten of unlisteners.splice(0)) unlisten();
  };
};

/** Formats `address:port`, bracketing IPv6 literals. */
export function formatAddress(address: string, port: number): string {
  return address.includes(":") ? `[${address}]:${port}` : `${address}:${port}`;
}
