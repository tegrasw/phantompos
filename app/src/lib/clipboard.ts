// Clipboard writes without a Tauri plugin.
//
// `navigator.clipboard.writeText` is used first; it needs a secure context and
// a user gesture, which the Tauri webviews provide for button clicks. If it is
// unavailable or rejected (some WebKitGTK builds), fall back to the legacy
// `execCommand("copy")` on a temporary textarea. If both fail, the caller shows
// a "Copy failed" hint; adding @tauri-apps/plugin-clipboard-manager would be
// the next step.

export async function copyText(text: string): Promise<boolean> {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch {
    // Fall through to the legacy path.
  }
  return legacyCopy(text);
}

function legacyCopy(text: string): boolean {
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  document.body.appendChild(textarea);
  textarea.select();
  try {
    return document.execCommand("copy");
  } catch {
    return false;
  } finally {
    textarea.remove();
  }
}
