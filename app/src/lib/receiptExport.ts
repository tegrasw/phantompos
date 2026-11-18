// Saving a completed receipt (SPEC "Ações"): the PNG is drawn here and its
// bytes handed to `export_receipt`; the `.bin` comes from the backend, which
// keeps each receipt's own bytes (the command log keeps only 256 bytes per
// command). The backend opens the native save dialog. The outcome is
// reported with a toast: the file name on success, nothing when the user
// cancels the dialog, a clear message on failure.

import type { BackendSession, ExportFormat, Receipt } from "./backend";
import { baseName, formatCount } from "./format";
import { i18n } from "./i18n";
import { ReceiptTooTallError, receiptPng } from "./receipt/png";
import { toasts } from "./toast.svelte";

/** What the backend says when the id is gone (it has no error codes yet). */
const UNAVAILABLE = /no longer|not available|unavailable|not found|unknown receipt|no receipt/i;

export async function saveReceipt(
  session: BackendSession | null,
  item: { id: number; receipt: Receipt },
  format: ExportFormat,
): Promise<void> {
  const t = i18n.t.receiptExport;
  let png: Uint8Array | undefined;
  if (format === "png") {
    try {
      png = await receiptPng(item.receipt);
    } catch (error) {
      toasts.show({
        tone: "neutral",
        title: error instanceof ReceiptTooTallError ? t.tooTall(formatCount(error.maxRows)) : t.renderFailed,
        detail: error instanceof ReceiptTooTallError ? undefined : errorText(error),
      });
      return;
    }
  }

  try {
    if (!session) throw new Error("No backend");
    const result = await session.exportReceipt(item.id, format, png);
    if (!result.saved) return; // Cancelled in the save dialog.
    const name = baseName(result.path);
    const folder = result.path.slice(0, Math.max(0, result.path.length - name.length - 1));
    toasts.show({
      tone: "success",
      title: t.saved(name),
      detail: [format === "png" ? t.savedPng : t.savedBin, folder].filter(Boolean).join(" · "),
      detailTitle: result.path,
    });
  } catch (error) {
    const message = errorText(error);
    const gone = UNAVAILABLE.test(message);
    toasts.show({
      tone: "neutral",
      title: gone ? t.unavailable : t.failed,
      detail: gone ? t.unavailableDetail : message,
    });
  }
}

/** Tauri rejects with the command's error string; anything else is stringified. */
export function errorText(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

