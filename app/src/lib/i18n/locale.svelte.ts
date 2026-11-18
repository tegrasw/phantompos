// The interface language, chosen once at startup from the system language
// (SPEC "Aparência": Portuguese or English, following the system).
//
// `navigator.languages` lists the user's preferred languages, most preferred
// first (in the Tauri webviews it mirrors the OS setting). The first entry
// that is Portuguese or English wins; anything else falls back to English. In
// development, `?lang=pt` or `?lang=en` overrides the choice.
//
// This module holds only the reactive state, so number and date formatting
// (format.ts) can read the locale without importing the catalogues.

export type Locale = "en" | "pt";

class LocaleState {
  /** Which catalogue the UI shows. */
  current: Locale = $state("en");
  /** BCP 47 tag for `Intl` (keeps the system's region, e.g. "pt-PT", "en-GB"). */
  tag: string = $state("en");
}

export const locale = new LocaleState();

/** Picks the catalogue and `Intl` tag for a list of preferred languages. */
export function detectLocale(languages: readonly string[]): { locale: Locale; tag: string } {
  for (const language of languages) {
    const lower = language.toLowerCase();
    if (lower === "pt" || lower.startsWith("pt-")) {
      return { locale: "pt", tag: lower === "pt" ? "pt-BR" : language };
    }
    if (lower === "en" || lower.startsWith("en-")) return { locale: "en", tag: language };
  }
  return { locale: "en", tag: "en" };
}

/** Chooses the locale before the app mounts and sets `<html lang>`. */
export function initLocale(): void {
  let preferred: readonly string[] = navigator.languages?.length ? navigator.languages : [navigator.language];
  if (import.meta.env.DEV) {
    const override = new URLSearchParams(location.search).get("lang");
    if (override === "pt" || override === "en") preferred = [override === "pt" ? "pt-BR" : "en-US"];
  }
  setLocale(detectLocale(preferred));
}

export function setLocale(next: { locale: Locale; tag: string }): void {
  locale.current = next.locale;
  locale.tag = safeTag(next.tag);
  document.documentElement.lang = next.locale === "pt" ? "pt-BR" : "en";
}

/** Falls back to the bare language when `Intl` rejects the system's tag. */
function safeTag(tag: string): string {
  try {
    return Intl.getCanonicalLocales(tag)[0] ?? "en";
  } catch {
    return locale.current === "pt" ? "pt-BR" : "en";
  }
}
