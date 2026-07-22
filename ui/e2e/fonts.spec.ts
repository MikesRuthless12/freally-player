import fs from "node:fs";

import { expect, test, type Page } from "@playwright/test";

const DIR = "e2e/screenshots";
fs.mkdirSync(DIR, { recursive: true });

/**
 * Screenshots are tagged with the OS so a three-OS CI matrix does not have each runner
 * overwrite the last one's images. Font fallback is per-platform — that is precisely what
 * running this on Windows, macOS and Linux is meant to expose.
 */
const OS = process.platform;

/**
 * Font smoke: every shipped language renders in a real bundled Noto face.
 *
 * This asserts what was ACTUALLY used to rasterise the glyphs, via the DevTools Protocol's
 * `CSS.getPlatformFontsForNode` — not what CSS asked for. That distinction is the whole point:
 * `font-family` claiming "Noto Sans KR Variable" proves nothing if the file failed to load, the
 * `unicode-range` slice was wrong, or the glyphs quietly fell back to a system font. Chromium
 * reports the resolved face per glyph run, so a regression shows up as the wrong family name
 * rather than as a screenshot a human has to squint at.
 *
 * It also catches tofu directly: an unrenderable codepoint produces zero glyphs from any real
 * family, so `glyphCount` collapses.
 *
 * Han unification is the subtle case this guards. SC, TC, JP and KR all cover the same
 * codepoints with different letterforms, so Japanese text rendering in Simplified Chinese
 * shapes is legible, wrong, and invisible to every other kind of test — it is only caught by
 * checking which family won.
 */

/** A language the UI ships, the script it needs, and the face that must render it. */
type LangCase = {
  /** BCP-47 tag, as the language switcher will set it on `<html lang>`. */
  lang: string;
  label: string;
  /** Text using glyphs that only the expected family (or a system fallback) can draw. */
  sample: string;
  /**
   * Family-name substrings that are an acceptable result. Usually one. Devanagari takes two
   * because the main Noto Sans ships a `devanagari` subset of its own, so Hindi renders in a
   * genuine Noto face either way and forcing one over the other buys nothing visually.
   */
  accept: string[];
};

/**
 * The 18 shipped UI locales (`ar de en es fr hi id it ja ko nl pl pt-BR ru tr uk vi zh-CN`),
 * plus zh-TW: Traditional Chinese is bundled because media *content* — filenames, subtitle
 * tracks, titles — is in whatever script the file uses, whatever the UI language is.
 */
const CASES: LangCase[] = [
  // Latin. One per locale rather than a single "Latin" case, because the accented and extended
  // characters live in different `unicode-range` slices and a missing slice is per-language.
  { lang: "en", label: "English", sample: "English", accept: ["Noto Sans"] },
  { lang: "de", label: "German", sample: "Grüße Straße", accept: ["Noto Sans"] },
  { lang: "es", label: "Spanish", sample: "Español ñ", accept: ["Noto Sans"] },
  { lang: "fr", label: "French", sample: "Français àèù", accept: ["Noto Sans"] },
  { lang: "id", label: "Indonesian", sample: "Bahasa Indonesia", accept: ["Noto Sans"] },
  { lang: "it", label: "Italian", sample: "Italiano perché", accept: ["Noto Sans"] },
  { lang: "nl", label: "Dutch", sample: "Nederlands", accept: ["Noto Sans"] },
  // Latin Extended-A: ł ą ę ż are a different slice from the base Latin one.
  { lang: "pl", label: "Polish", sample: "Język polski ćłńóśźż", accept: ["Noto Sans"] },
  { lang: "pt-BR", label: "Portuguese (BR)", sample: "Português ção", accept: ["Noto Sans"] },
  { lang: "tr", label: "Turkish", sample: "Türkçe ışğİ", accept: ["Noto Sans"] },
  // Vietnamese stacks two diacritics per glyph and has its own slice.
  { lang: "vi", label: "Vietnamese", sample: "Tiếng Việt ượ", accept: ["Noto Sans"] },
  // Cyrillic.
  { lang: "ru", label: "Russian", sample: "Русский язык", accept: ["Noto Sans"] },
  { lang: "uk", label: "Ukrainian", sample: "Українська їєґ", accept: ["Noto Sans"] },
  // Scripts with their own bundled family.
  { lang: "ar", label: "Arabic", sample: "العربية", accept: ["Noto Sans Arabic"] },
  {
    lang: "hi",
    label: "Hindi",
    sample: "हिन्दी भाषा",
    accept: ["Noto Sans Devanagari", "Noto Sans"],
  },
  // CJK — the Han-unification cases.
  { lang: "ja", label: "Japanese", sample: "日本語のテキスト", accept: ["Noto Sans JP"] },
  { lang: "ko", label: "Korean", sample: "한국어 텍스트", accept: ["Noto Sans KR"] },
  {
    lang: "zh-CN",
    label: "Chinese (Simplified)",
    sample: "简体中文文本",
    accept: ["Noto Sans SC"],
  },
  {
    lang: "zh-TW",
    label: "Chinese (Traditional)",
    sample: "繁體中文文字",
    accept: ["Noto Sans TC"],
  },
];

/**
 * `isCustomFont` is what makes this test honest on every OS: it distinguishes a face loaded
 * from our bundled `@font-face` from one the machine already had. Ubuntu CI commonly has
 * system Noto installed, so a family-name match alone would go green even if the bundle were
 * missing entirely — the exact regression this test exists to catch.
 */
type PlatformFont = { familyName: string; isCustomFont: boolean; glyphCount: number };
type Cdp = Awaited<ReturnType<Awaited<ReturnType<Page["context"]>>["newCDPSession"]>>;

/** Put `sample` on screen under `lang`, and force the layout that triggers the font fetch. */
async function paintProbe(page: Page, lang: string, sample: string): Promise<void> {
  await page.evaluate(
    ({ lang, sample }) => {
      document.documentElement.lang = lang;
      let probe = document.querySelector("#font-probe") as HTMLElement | null;
      if (!probe) {
        probe = document.createElement("div");
        probe.id = "font-probe";
        // Fully visible and measurable: a display:none, zero-size or transparent node produces
        // no glyph runs, so Chromium would report no fonts and the assertions would pass
        // vacuously. Being visible also makes the screenshot below worth looking at.
        probe.style.cssText =
          "position:fixed;left:24px;top:24px;padding:16px 24px;font-size:40px;z-index:99999;" +
          "background:#141417;color:#e8e8ea;border:1px solid #2a2a30;border-radius:8px;";
        document.body.appendChild(probe);
      }
      probe.textContent = sample;
      // Reading a layout property forces reflow, which is what actually schedules the fetch of
      // the `unicode-range` slice this text needs.
      void probe.offsetWidth;
    },
    { lang, sample },
  );
}

/** What Chromium actually used to rasterise the probe, right now. */
async function renderedFonts(cdp: Cdp): Promise<PlatformFont[]> {
  const doc = await cdp.send("DOM.getDocument");
  const { nodeId } = await cdp.send("DOM.querySelector", {
    nodeId: doc.root.nodeId,
    selector: "#font-probe",
  });
  if (!nodeId) return [];
  const { fonts } = await cdp.send("CSS.getPlatformFontsForNode", { nodeId });
  return fonts as PlatformFont[];
}

/**
 * Poll until the expected face has actually rasterised the sample, or time out.
 *
 * Polling the resolved fonts is deliberate. `document.fonts.check()` looks like the natural
 * gate and is the wrong one: every CJK family also carries Latin slices, so for English text
 * `check()` sees matching-but-unloaded faces and answers false forever. The rasterised result
 * is the only thing that answers the question being asked.
 */
async function waitForRenderedFont(
  page: Page,
  cdp: Cdp,
  { lang, sample, accept }: LangCase,
  // Generous on purpose. Each test gets a fresh browser context, so nothing is cached between
  // them and every one re-fetches its `unicode-range` slices; a CJK case pulls several. At 15s
  // this went red under a full `ci-local.mjs` run — not because anything was broken, but
  // because the machine was busy after nine prior steps — and a font test that fails on load
  // is indistinguishable from one that fails on a real bug. CI runners are slower still.
  timeoutMs = 45_000,
): Promise<PlatformFont[]> {
  const deadline = Date.now() + timeoutMs;
  let fonts: PlatformFont[] = [];
  await paintProbe(page, lang, sample);
  for (;;) {
    await page.evaluate(() => document.fonts.ready);
    fonts = await renderedFonts(cdp);
    if (fonts.some((f) => isBundled(f, accept))) return fonts;
    if (Date.now() > deadline) return fonts;
    await page.waitForTimeout(200);
  }
}

/** A face that is both one of the accepted Noto families AND loaded from our bundle. */
function isBundled(font: PlatformFont, accept: string[]): boolean {
  return font.isCustomFont && accept.some((family) => font.familyName.includes(family));
}

test.describe("bundled Noto renders every shipped language", () => {
  for (const testCase of CASES) {
    test(`${testCase.lang} — ${testCase.label}`, async ({ page }) => {
      await page.addInitScript({ path: "e2e/tauri-mock.js" });
      await page.goto("/");
      await page.getByRole("button", { name: "Open media…" }).waitFor({ timeout: 15_000 });

      const cdp = await page.context().newCDPSession(page);
      await cdp.send("DOM.enable");
      await cdp.send("CSS.enable");
      const fonts = await waitForRenderedFont(page, cdp, testCase);
      const used =
        fonts
          .map(
            (f) =>
              `${f.familyName}${f.isCustomFont ? " [bundled]" : " [system]"} (${f.glyphCount} glyphs)`,
          )
          .join(", ") || "nothing";

      // Something must have drawn actual glyphs. Zero glyph runs means tofu.
      expect(fonts.length, `${testCase.lang}: nothing rendered any glyphs`).toBeGreaterThan(0);
      const drew = fonts.reduce((n, f) => n + f.glyphCount, 0);
      expect(drew, `${testCase.lang}: no glyphs drawn — used: ${used}`).toBeGreaterThan(0);

      // ...and it must be OUR bundled Noto, not a system font standing in for it.
      const matched = fonts.some((f) => isBundled(f, testCase.accept));
      expect(
        matched,
        `${testCase.lang} (${testCase.label}) expected a bundled ${testCase.accept.join(" or ")}, but rendered with: ${used}`,
      ).toBe(true);

      // Visual evidence, one image per language per OS. The assertions above are the gate; this
      // is what a human looks at to judge whether the letterforms are actually RIGHT — Han
      // mis-unification is correct-by-every-machine-check and still wrong to a reader.
      await page
        .locator("#font-probe")
        .screenshot({ path: `${DIR}/font-${testCase.lang}-${OS}.png` });
    });
  }
});
