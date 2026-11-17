// What the UI knows about the printer endpoint, reduced to one discriminated
// union that the header and the roll area both render.

import { formatAddress, type ListenerStatus } from "./backend";

export type PrinterStatus =
  /** No backend at all (plain browser without the dev demo). */
  | { kind: "disconnected" }
  /** Waiting for the first `get_state` / `listener:status`. */
  | { kind: "starting" }
  /** The backend answered with an error before any listener status. */
  | { kind: "backendError"; message: string }
  | { kind: "listening"; host: string; port: number; address: string }
  | { kind: "stopped"; host: string; port: number; address: string }
  | { kind: "error"; port: number; address: string; message: string };

export function printerStatus(
  hasBackend: boolean,
  listener: ListenerStatus | null,
  backendError: string | null,
): PrinterStatus {
  if (!hasBackend) return { kind: "disconnected" };
  if (backendError) return { kind: "backendError", message: backendError };
  if (!listener) return { kind: "starting" };
  const address = formatAddress(listener.address, listener.port);
  if (listener.state === "error")
    return {
      kind: "error",
      port: listener.port,
      address,
      message: listener.message ?? address,
    };
  // Suggest loopback in hints even when bound to every interface.
  const host =
    listener.address === "0.0.0.0" || listener.address === "::"
      ? "127.0.0.1"
      : listener.address;
  return { kind: listener.state, host, port: listener.port, address };
}
