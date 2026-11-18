// Interface strings in English and Brazilian Portuguese. `i18n.t` is the
// catalogue of the current locale; reading it inside a component (template or
// `$derived`) is reactive, so the UI follows a locale change.

import { en, type Messages, type Vocabulary } from "./en";
import { locale, type Locale } from "./locale.svelte";
import { pt } from "./pt";

export type { Locale, Messages, Vocabulary };

const CATALOGUES: Record<Locale, Messages> = { en, pt };

export const i18n = {
  get t(): Messages {
    return CATALOGUES[locale.current];
  },
  get locale(): Locale {
    return locale.current;
  },
};

/** A label from an open vocabulary, or `fallback` when it has none. */
export function lookup(vocabulary: Vocabulary, key: string, fallback: string): string {
  return Object.prototype.hasOwnProperty.call(vocabulary, key) ? vocabulary[key] : fallback;
}
