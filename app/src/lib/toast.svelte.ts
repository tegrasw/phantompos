// Transient notices ("Saved receipt-12.png", "Couldn't save the receipt"),
// shown by <Toaster> at the bottom of the window. Any module can call
// `toasts.show()`; at most MAX_VISIBLE stay on screen (the oldest go first).
// Each one closes by itself after its timeout, which pauses while the pointer
// or keyboard focus is on it.

export type ToastTone = "success" | "neutral";

export interface ToastOptions {
  tone?: ToastTone;
  title: string;
  /** Second line: a file name, an explanation. */
  detail?: string;
  /** Shown on hover over the detail (e.g. the full path). */
  detailTitle?: string;
  /** Milliseconds before it closes; 0 keeps it until dismissed. */
  timeout?: number;
}

export interface Toast extends Required<Pick<ToastOptions, "tone" | "title" | "timeout">> {
  id: number;
  detail: string | null;
  detailTitle: string | null;
}

const MAX_VISIBLE = 3;
const DEFAULT_TIMEOUT = { success: 4500, neutral: 8000 } as const;

class Toasts {
  items: Toast[] = $state([]);
  #nextId = 1;

  show(options: ToastOptions): number {
    const tone = options.tone ?? "success";
    const toast: Toast = {
      id: this.#nextId++,
      tone,
      title: options.title,
      detail: options.detail ?? null,
      detailTitle: options.detailTitle ?? null,
      timeout: options.timeout ?? DEFAULT_TIMEOUT[tone],
    };
    this.items = [...this.items, toast].slice(-MAX_VISIBLE);
    return toast.id;
  }

  dismiss(id: number): void {
    this.items = this.items.filter((toast) => toast.id !== id);
  }
}

export const toasts = new Toasts();
