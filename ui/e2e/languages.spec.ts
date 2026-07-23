import fs from "node:fs";

import { expect, test, type Page } from "@playwright/test";

import { LOCALES } from "../src/i18n/locales";

/**
 * Language smoke — Definition of Done step 6c.
 *
 * Drives the REAL Settings modal: opens the Language pane, clicks each of the 18 shipped
 * languages in turn, and proves the UI actually changed into it. Not "a catalog file exists" —
 * the chrome in front of the user is in that language, switched live, with no restart.
 *
 * The expected strings are read from the catalogs themselves rather than written out here. A
 * copy of the translations in the test would only prove the test agrees with itself, and would
 * go green against a catalog nothing loads.
 *
 * Screenshots are tagged per OS, like `fonts.spec.ts`: this runs on Windows, macOS and Linux,
 * and the point of the images is that a human can see each language rendered on each platform.
 * `fonts.spec.ts` proves the right FONT drew the glyphs; this proves the right STRINGS are on
 * screen and the shell mirrors for Arabic. Neither one implies the other.
 */
const DIR = "e2e/screenshots";
fs.mkdirSync(DIR, { recursive: true });

const OS = process.platform;

/** One string out of a catalog, read the same way the app reads it. */
function catalogString(locale: string, key: string): string {
  const source = fs.readFileSync(`src/i18n/${locale}.ftl`, "utf8");
  const match = new RegExp(`^${key}\\s*=\\s*(.+)$`, "m").exec(source);
  if (!match) throw new Error(`${locale}.ftl has no "${key}" — i18n:lint should have caught this`);
  return match[1].trim();
}

/**
 * In the mirrored Arabic shell, the seek buttons' "−" must still sit to the LEFT of "10".
 *
 * It is a bidi-neutral character, so in an RTL paragraph it resolves to the paragraph
 * direction and jumps to the far side of the digits: "−10 ث" renders as "ث 10−". That is
 * correct in reading order and still reads as "10 minus" to anyone reading the numerals, which
 * is why those buttons are pinned `dir="ltr"`. Measured per character, because the string is
 * byte-identical either way — only the glyph positions differ, and no text assertion can see
 * the difference.
 */
async function expectSignBeforeDigits(page: Page) {
  const order = await page.evaluate(() => {
    const button = [...document.querySelectorAll("button")].find((b) =>
      b.textContent?.includes("−10"),
    );
    const node = button?.firstChild;
    if (!node?.textContent) return null;
    const text = node.textContent;
    const positions = [...text].map((ch, i) => {
      const range = document.createRange();
      range.setStart(node, i);
      range.setEnd(node, i + 1);
      return { ch, x: range.getBoundingClientRect().left };
    });
    return positions
      .sort((a, b) => a.x - b.x)
      .map((p) => p.ch)
      .join("");
  });
  expect(order, "the seek button's sign is on the wrong side of its digits").toBe("−10 ث");
}

async function boot(page: Page) {
  await page.addInitScript({ path: "e2e/tauri-mock.js" });
  // Open a file, paused: the transport controls behind the modal (Play/pause, the seek
  // buttons) are what prove the shell re-rendered into each language, and a paused file keeps
  // them up rather than auto-hiding, with the toggle reading Play.
  await page.goto("/?media=1&paused=1");
  await page.getByRole("button", { name: "Play", exact: true }).waitFor({ timeout: 15_000 });
}

test.describe("every shipped language switches live from the Settings modal", () => {
  for (const { code, autonym } of LOCALES) {
    test(`${code} — ${autonym}`, async ({ page }) => {
      await boot(page);

      // Start in English every time, so each case exercises a real switch INTO its language.
      await expect(page.getByRole("button", { name: "Play", exact: true })).toBeVisible();

      await page.getByRole("button", { name: "Settings", exact: true }).click();
      await page.getByRole("button", { name: "Language", exact: true }).click();

      // English first, then alphabetical by the language's own name — and the same order in
      // every language, because the list is a baked-in literal rather than a runtime sort.
      const picker = page.getByRole("dialog").locator("button[lang]");
      await expect(picker).toHaveText(LOCALES.map((l) => l.autonym));

      await page.getByRole("button", { name: autonym, exact: true }).click();

      // The transport is behind the modal and is not part of the pane that was clicked, so its
      // label changing is proof the whole shell re-rendered, not just the picker.
      const play = catalogString(code, "transport-play");
      await expect(page.getByRole("button", { name: play, exact: true })).toBeVisible();
      if (code !== "en") {
        await expect(page.getByRole("button", { name: "Play", exact: true })).toHaveCount(0);
      }

      // The pane's own heading, from the same catalog.
      await expect(
        page.getByRole("heading", { name: catalogString(code, "settings-language"), exact: true }),
      ).toBeVisible();

      // `lang` drives the per-script font stacks in styles/fonts.css; `dir` mirrors the shell.
      await expect(page.locator("html")).toHaveAttribute("lang", code);
      await expect(page.locator("html")).toHaveAttribute("dir", code === "ar" ? "rtl" : "ltr");

      if (code === "ar") await expectSignBeforeDigits(page);

      await page.screenshot({ path: `${DIR}/lang-${code}-${OS}.png` });
    });
  }
});
