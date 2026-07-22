/**
 * The shipped UI locales (`prd.md` FR-018) and how a first run picks one.
 *
 * The autonyms are deliberately NOT in the catalogs: a language's own name is the same in
 * every language, so translating "Deutsch" 18 times would be 18 chances to get it wrong for
 * no gain. A picker that lists each language in its own script is also the only kind a user
 * who cannot read the current UI language can navigate out of.
 */

/**
 * English first (FR-018), then the rest alphabetically by autonym.
 *
 * THE ORDER IS BAKED IN, NOT SORTED AT RUNTIME, AND THAT IS THE POINT. Collation is
 * locale-sensitive: `Intl.Collator("ar")` lifts العربية to the top, `("ru")` lifts Cyrillic,
 * `("zh-CN")` lifts the CJK names. Sorting this list with the active locale therefore
 * reshuffles it every time the user switches language — the picker they just used moves under
 * their cursor. Across the 18 shipped locales that produces five different orders.
 *
 * So the sort happened once, with a fixed `Intl.Collator("en")` over the autonyms, and the
 * result is written out here. Every user sees the same list in the same order, whatever
 * language they are in. `localeOrderIsStable` in `locales.test.ts` holds this literal to that.
 * When a locale is added, insert it where that same collator puts it.
 */
export const LOCALES = [
  { code: "en", autonym: "English" },
  { code: "id", autonym: "Bahasa Indonesia" },
  { code: "de", autonym: "Deutsch" },
  { code: "es", autonym: "Español" },
  { code: "fr", autonym: "Français" },
  { code: "it", autonym: "Italiano" },
  { code: "nl", autonym: "Nederlands" },
  { code: "pl", autonym: "Polski" },
  { code: "pt-BR", autonym: "Português (Brasil)" },
  { code: "vi", autonym: "Tiếng Việt" },
  { code: "tr", autonym: "Türkçe" },
  { code: "ru", autonym: "Русский" },
  { code: "uk", autonym: "Українська" },
  { code: "ar", autonym: "العربية" },
  { code: "hi", autonym: "हिन्दी" },
  { code: "ko", autonym: "한국어" },
  { code: "ja", autonym: "日本語" },
  { code: "zh-CN", autonym: "简体中文" },
] as const;

export type LocaleCode = (typeof LOCALES)[number]["code"];

/** The source catalog. Every other locale falls back to it key by key. */
export const DEFAULT_LOCALE: LocaleCode = "en";

/** Scripts written right-to-left. Arabic is the only one of the 18. */
const RTL: readonly string[] = ["ar"];

export function isRtl(locale: LocaleCode): boolean {
  return RTL.includes(locale);
}

const CODES: readonly string[] = LOCALES.map((l) => l.code);

export function isLocaleCode(value: string): value is LocaleCode {
  return CODES.includes(value);
}

/**
 * The best shipped locale for a list of user-preferred tags, or `null` if none fits.
 *
 * Matches exactly first, then on the primary subtag — so `pt-PT` lands on `pt-BR` and `zh-TW`
 * on `zh-CN` rather than dropping all the way to English. Simplified letterforms are the wrong
 * ones for a Traditional reader, but a language they read beats one they may not.
 */
export function matchLocale(preferred: readonly string[]): LocaleCode | null {
  for (const tag of preferred) {
    const exact = LOCALES.find((l) => l.code.toLowerCase() === tag.toLowerCase());
    if (exact) return exact.code;

    const primary = tag.split("-")[0]?.toLowerCase();
    if (!primary) continue;
    const related = LOCALES.find((l) => l.code.split("-")[0].toLowerCase() === primary);
    if (related) return related.code;
  }
  return null;
}

/**
 * The locale to start in: the user's stored choice, or — on a first run, where there is no
 * choice yet — whatever the OS told the webview, falling back to English.
 */
export function resolveLocale(stored: string | null | undefined): LocaleCode {
  if (stored && isLocaleCode(stored)) return stored;
  const preferred =
    typeof navigator === "undefined"
      ? []
      : (navigator.languages ?? (navigator.language ? [navigator.language] : []));
  return matchLocale(preferred) ?? DEFAULT_LOCALE;
}
