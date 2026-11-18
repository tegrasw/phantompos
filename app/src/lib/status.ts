// What the UI knows about the printer endpoint (the TCP listener), reduced to
// one discriminated union that the header, the roll area and the settings
// dialog all render. The simulated printer state (paper, cover, offline,
// drawer) is `PrinterStatus` in backend.ts.

import { formatAddress, type ListenerErrorCode, type ListenerStatus } from "./backend";

/** Where clients can reach the printer while it listens (or would). */
export interface Reach {
  port: number;
  /** Bound to every interface (network mode). */
  network: boolean;
  /** `host:port` for every address to show, most relevant first; may be empty. */
  addresses: string[];
  /** Host for hints and the test command: the first address, else loopback. */
  host: string;
}

export type EndpointStatus =
  /** No backend at all (plain browser without the dev demo). */
  | { kind: "disconnected" }
  /** Waiting for the first `get_state` / `listener:status`. */
  | { kind: "starting" }
  /** The backend answered with an error before any listener status. */
  | { kind: "backendError"; message: string }
  | ({ kind: "listening" | "stopped" } & Reach)
  | {
      kind: "error";
      port: number;
      network: boolean;
      code: ListenerErrorCode;
      /** A free port to offer (port in use only). */
      suggestedPort: number | null;
      /** The backend's English description (shown only as a detail). */
      message: string | null;
    };

const LOOPBACK = "127.0.0.1";
const ANY = new Set(["0.0.0.0", "::", "[::]"]);

export function endpointStatus(
  hasBackend: boolean,
  listener: ListenerStatus | null,
  backendError: string | null,
): EndpointStatus {
  if (!hasBackend) return { kind: "disconnected" };
  if (backendError) return { kind: "backendError", message: backendError };
  if (!listener) return { kind: "starting" };
  const network = ANY.has(listener.address);
  if (listener.state === "error") {
    const code: ListenerErrorCode =
      listener.code === "portInUse" ? "portInUse" : listener.code === "permissionDenied" ? "permissionDenied" : "other";
    const suggested = listener.suggestedPort;
    return {
      kind: "error",
      port: listener.port,
      network,
      code,
      suggestedPort:
        code === "portInUse" && typeof suggested === "number" && suggested !== listener.port ? suggested : null,
      message: listener.message ?? null,
    };
  }
  // Older backends send no `addresses`: show the bind address unless it is
  // the wildcard.
  const hosts = listener.addresses ?? (network ? [] : [listener.address]);
  return {
    kind: listener.state,
    port: listener.port,
    network,
    addresses: hosts.map((host) => formatAddress(host, listener.port)),
    host: hosts[0] ?? LOOPBACK,
  };
}

/** `127.0.0.1:port`, the address local programs can always use. */
export function loopbackAddress(port: number): string {
  return formatAddress(LOOPBACK, port);
}

/** What the UI can do about a listener error (wired in App). */
export interface ListenerControls {
  /** Switches to another port (`update_settings`); never called without a click. */
  usePort(port: number): Promise<void>;
  /** Binds the same port again (`retry_listener`). */
  retry(): Promise<void>;
  openSettings(): void;
}

export type ListenerError = Extract<EndpointStatus, { kind: "error" }>;
