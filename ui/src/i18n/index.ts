/**
 * The Fluent translation runtime (`prd.md` FR-018).
 *
 * Every catalog is bundled eagerly. All 18 together are a few tens of kilobytes of text, and
 * loading them up front is what lets the Language pane switch instantly, offline, with no
 * restart and no request — the charter forbids the app reaching the network on its own, so a
 * lazily-fetched catalog is not an option anyway.
 *
 * A key missing from the active catalog falls back to English, then to the key itself, so a
 * gap shows up as a visible identifier rather than as blank chrome. `npm run i18n:lint` is
 * what stops such a gap ever reaching a build.
 */
import { FluentBundle, FluentResource } from "@fluent/bundle";
import { createContext, useContext, useMemo } from "react";

import { DEFAULT_LOCALE, isRtl, type LocaleCode } from "./locales";

/** Every `<code>.ftl` beside this file, inlined at build time. */
const SOURCES = import.meta.glob("./*.ftl", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

/** Values a string can interpolate. Keep them primitive — a catalog formats, it does not compute. */
export type TranslateArgs = Record<string, string | number>;

export type Translate = (id: string, args?: TranslateArgs) => string;

const bundles = new Map<string, FluentBundle>();

function bundleFor(locale: LocaleCode): FluentBundle | null {
  const cached = bundles.get(locale);
  if (cached) return cached;

  const source = SOURCES[`./${locale}.ftl`];
  if (source === undefined) return null;

  // `useIsolating` wraps every interpolated value in U+2068/U+2069 bidi isolates. That is the
  // right default for text that mixes directions inside one paragraph, but here it would put
  // invisible control characters into button labels and accessible names — where they leak
  // into `aria-label` comparisons and into anything matching on exact text. The strings that
  // interpolate at all are short and single-direction, so the isolates buy nothing.
  const bundle = new FluentBundle(locale, { useIsolating: false });
  bundle.addResource(new FluentResource(source));
  bundles.set(locale, bundle);
  return bundle;
}

function format(bundle: FluentBundle | null, id: string, args?: TranslateArgs): string | null {
  if (!bundle) return null;
  const message = bundle.getMessage(id);
  if (!message?.value) return null;
  return bundle.formatPattern(message.value, args);
}

/** A `t()` bound to one locale, falling back to English and then to the key itself. */
export function translator(locale: LocaleCode): Translate {
  const active = bundleFor(locale);
  const fallback = locale === DEFAULT_LOCALE ? null : bundleFor(DEFAULT_LOCALE);
  return (id, args) => format(active, id, args) ?? format(fallback, id, args) ?? id;
}

/**
 * Stamp the locale on the document root.
 *
 * `lang` is load-bearing beyond accessibility: `styles/fonts.css` keys its per-script font
 * stacks off `:lang()`, so this one attribute is what makes Japanese render in JP letterforms
 * rather than Simplified Chinese ones. `dir` mirrors the shell for Arabic.
 */
export function applyLocale(locale: LocaleCode): void {
  const root = document.documentElement;
  root.lang = locale;
  root.dir = isRtl(locale) ? "rtl" : "ltr";
}

/**
 * The active `t()`. Provided by `App`, which owns the locale because it owns the settings the
 * locale is stored in.
 */
export const I18nContext = createContext<Translate>(translator(DEFAULT_LOCALE));

export function useT(): Translate {
  return useContext(I18nContext);
}

/** `t()` for a locale, rebuilt only when the locale actually changes. */
export function useTranslator(locale: LocaleCode): Translate {
  return useMemo(() => translator(locale), [locale]);
}
