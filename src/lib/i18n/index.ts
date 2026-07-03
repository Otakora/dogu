import { app } from "../stores/app.svelte.js";
import { translations, type TranslationKey } from "./translations.js";

export type { TranslationKey } from "./translations.js";
export type { Locale } from "../types/index.js";

export function t(key: TranslationKey, vars?: Record<string, string | number>): string {
  const locale = app.settings.locale;
  let text = translations[locale][key] ?? translations.en[key] ?? key;
  if (vars) {
    for (const [k, v] of Object.entries(vars)) {
      text = text.replaceAll(`{${k}}`, String(v));
    }
  }
  return text;
}

export function tn(
  countKey: TranslationKey,
  pluralKey: TranslationKey,
  count: number,
  vars?: Record<string, string | number>
): string {
  return t(count === 1 ? countKey : pluralKey, { count, ...vars });
}
