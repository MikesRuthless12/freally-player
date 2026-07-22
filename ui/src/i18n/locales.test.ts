import { describe, expect, it } from "vitest";

import { DEFAULT_LOCALE, isRtl, LOCALES, matchLocale, resolveLocale } from "./locales";
import { translator } from "./index";

describe("the shipped locales", () => {
  it("ships the 18 languages FR-018 names", () => {
    expect(LOCALES.map((l) => l.code).sort()).toEqual(
      [
        "ar",
        "de",
        "en",
        "es",
        "fr",
        "hi",
        "id",
        "it",
        "ja",
        "ko",
        "nl",
        "pl",
        "pt-BR",
        "ru",
        "tr",
        "uk",
        "vi",
        "zh-CN",
      ].sort(),
    );
  });

  it("lists English first, then the rest alphabetically by their own name", () => {
    expect(LOCALES[0].code).toBe("en");

    const rest = LOCALES.slice(1);
    const collator = new Intl.Collator("en");
    const expected = [...rest].sort((a, b) => collator.compare(a.autonym, b.autonym));
    expect(rest.map((l) => l.autonym)).toEqual(expected.map((l) => l.autonym));
  });

  /**
   * The reason the list is a baked-in literal rather than a runtime sort.
   *
   * Collation is locale-sensitive: sorting these names with the ACTIVE locale's collator
   * reorders them per language — Arabic lifts العربية to the top, Russian lifts Cyrillic,
   * Chinese lifts the CJK names. The picker would rearrange itself under the user every time
   * they switched. This proves that hazard is real, so nobody "tidies" the literal into a
   * `.sort()` later; the order the UI renders is `LOCALES` itself, in every language.
   */
  it("cannot be replaced by a locale-sensitive sort", () => {
    const orders = new Set(
      LOCALES.map(({ code }) => {
        const collator = new Intl.Collator(code);
        return [...LOCALES]
          .sort((a, b) => collator.compare(a.autonym, b.autonym))
          .map((l) => l.code)
          .join(",");
      }),
    );
    expect(orders.size).toBeGreaterThan(1);
  });

  it("marks Arabic, and only Arabic, as right-to-left", () => {
    expect(LOCALES.filter((l) => isRtl(l.code)).map((l) => l.code)).toEqual(["ar"]);
  });
});

describe("choosing a locale", () => {
  it("prefers an exact tag", () => {
    expect(matchLocale(["pt-BR"])).toBe("pt-BR");
    expect(matchLocale(["JA"])).toBe("ja");
  });

  // A language the user reads beats English, even when the exact regional catalog is absent.
  it("falls back to a shipped locale with the same primary language", () => {
    expect(matchLocale(["pt-PT"])).toBe("pt-BR");
    expect(matchLocale(["zh-TW"])).toBe("zh-CN");
    expect(matchLocale(["de-AT"])).toBe("de");
  });

  it("takes the first preference it actually ships", () => {
    expect(matchLocale(["cy", "ga", "fr"])).toBe("fr");
  });

  it("reports no match rather than guessing", () => {
    expect(matchLocale(["cy", "ga"])).toBeNull();
  });

  it("uses a stored choice over anything the OS says", () => {
    expect(resolveLocale("ko")).toBe("ko");
  });

  // A hand-edited settings file is the realistic source of this: Rust stores the tag without
  // validating it, because the catalogs — not the settings store — decide what exists.
  it("ignores a stored language it does not ship", () => {
    expect(resolveLocale("xx-YY")).toBe(DEFAULT_LOCALE);
  });
});

describe("translation", () => {
  it("translates a key in every shipped locale", () => {
    for (const { code } of LOCALES) {
      expect(translator(code)("titlebar-settings")).not.toBe("");
    }
  });

  it("actually differs from English in every other locale", () => {
    const english = translator("en")("transport-play");
    for (const { code } of LOCALES) {
      if (code === "en") continue;
      expect(translator(code)("transport-play"), `${code} did not translate it`).not.toBe(english);
    }
  });

  it("interpolates without wrapping the value in bidi isolates", () => {
    // `useIsolating: false`: an isolate here would put U+2068/U+2069 into accessible names.
    expect(translator("en")("eula-version", { version: "2026-07-21" })).toBe("Version 2026-07-21");
  });

  // A gap must be visible, not blank. `i18n:lint` is what stops one ever shipping.
  it("falls back to the key itself for a string no catalog has", () => {
    expect(translator("fr")("no-such-key")).toBe("no-such-key");
  });
});
