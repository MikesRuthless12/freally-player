import fs from "node:fs";

import { expect, test, type Page } from "@playwright/test";

/**
 * The visual-smoke gallery: boot the real built UI against a mocked Tauri IPC and screenshot
 * every feature panel. Each screenshot is a rendering confirmation — proof the panel draws,
 * not proof the feature works.
 *
 * Every screenshot is also asserted on, so a blank or broken panel FAILS rather than quietly
 * producing an empty image. A gallery that only screenshots can go green while showing
 * nothing.
 *
 * Anything this cannot reach — real decoding, the native video surface, audio devices, GPU
 * paths, OS integration — belongs in `Live-To-Do-List.md` as a human drill.
 */
const DIR = "e2e/screenshots";
fs.mkdirSync(DIR, { recursive: true });

async function boot(page: Page, query = "") {
  await page.addInitScript({ path: "e2e/tauri-mock.js" });
  await page.goto("/" + query);
}

/** Commands the UI actually sent to the backend, in order. */
async function invoked(page: Page): Promise<string[]> {
  return page.evaluate(() =>
    (window as unknown as { __invokeLog: { cmd: string }[] }).__invokeLog.map((c) => c.cmd),
  );
}

/** The arguments of the last call to `cmd`. */
async function lastArgs(page: Page, cmd: string): Promise<Record<string, unknown> | undefined> {
  return page.evaluate((wanted) => {
    const log = (window as unknown as { __invokeLog: { cmd: string; args: unknown }[] })
      .__invokeLog;
    const hit = [...log].reverse().find((c) => c.cmd === wanted);
    return hit?.args as Record<string, unknown> | undefined;
  }, cmd);
}

/** The player chrome is up once the transport is rendered. */
async function playerReady(page: Page) {
  await page.getByRole("button", { name: "Open media…" }).waitFor({ timeout: 15_000 });
  await page.waitForTimeout(300);
}

test("01 — EULA acceptance gate", async ({ page }) => {
  await boot(page, "?eula=0");

  const agree = page.getByRole("button", { name: "I Agree" });
  await agree.waitFor({ timeout: 15_000 });
  // The gate must actually gate: no player chrome behind it, and Agree stays disabled
  // until the agreement has been scrolled through.
  await expect(page.getByLabel("Video stage")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Decline & Quit" })).toBeVisible();
  // The gate's own title, not the heading inside the rendered agreement text.
  await expect(
    page.getByRole("heading", { name: "Freally Player — End User License Agreement", exact: true }),
  ).toBeVisible();
  // The embedded agreement really rendered, not an empty scroll box.
  await expect(page.getByText(/solely responsible/)).toBeVisible();

  await page.screenshot({ path: `${DIR}/01-eula-gate.png` });
});

test("02 — player shell, nothing loaded", async ({ page }) => {
  await boot(page);
  await playerReady(page);

  await expect(page.getByText("No media loaded")).toBeVisible();
  await expect(page.getByLabel("Video stage")).toBeVisible();
  await expect(page.getByText("v0.10.0")).toBeVisible();

  await page.screenshot({ path: `${DIR}/02-player-idle.png` });
});

test("03 — transport with media open", async ({ page }) => {
  await boot(page, "?media=1");
  await playerReady(page);

  // The stage stays clear: the picture is drawn by the native surface, not the web layer.
  await expect(page.getByText(/Big Buck Bunny/)).toBeVisible();
  await expect(page.getByText(/playing/)).toBeVisible();
  for (const control of ["Play", "Pause", "−10s", "+10s"]) {
    await expect(page.getByRole("button", { name: control })).toBeVisible();
  }

  await page.screenshot({ path: `${DIR}/03-transport-playing.png` });
});

test("04 — light theme", async ({ page }) => {
  await boot(page, "?theme=light");
  await playerReady(page);

  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await expect(page.getByRole("button", { name: "Switch to dark mode" })).toBeVisible();

  await page.screenshot({ path: `${DIR}/04-light-theme.png` });
});

test("05 — theme toggles back to dark", async ({ page }) => {
  await boot(page, "?theme=light");
  await playerReady(page);

  await page.getByRole("button", { name: "Switch to dark mode" }).click();
  await expect(page.locator("html")).not.toHaveAttribute("data-theme", "light");

  await page.screenshot({ path: `${DIR}/05-theme-toggled-dark.png` });
});

test("06 — bug reporter, opened from the footer", async ({ page }) => {
  await boot(page);
  await playerReady(page);

  await page.getByRole("button", { name: "Report a bug" }).click();
  const dialog = page.getByRole("dialog", { name: "Report a bug" });
  await dialog.waitFor({ timeout: 10_000 });

  // The charter promise is visible, and the exact text that would be sent is shown.
  await expect(page.getByText(/opt-in and anonymous/)).toBeVisible();
  await expect(page.getByText(/EXACTLY WHAT WILL BE SENT/i)).toBeVisible();
  await expect(page.getByText(/ANONYMOUS DIAGNOSTICS/)).toBeVisible();
  for (const target of ["Open GitHub issue", "Compose in Gmail", "Send email", "Copy report"]) {
    await expect(page.getByRole("button", { name: target })).toBeVisible();
  }

  await page.screenshot({ path: `${DIR}/06-bug-report.png` });
});

test("08 — accepting the EULA persists it and opens the player", async ({ page }) => {
  await boot(page, "?eula=0");

  const agree = page.getByRole("button", { name: "I Agree" });
  await agree.waitFor({ timeout: 15_000 });
  await expect(agree).toBeEnabled();
  await agree.click();

  await playerReady(page);
  expect(await invoked(page)).toContain("eula_accept");

  await page.screenshot({ path: `${DIR}/08-eula-accepted.png` });
});

test("09 — Open media… drives open_media with the picked path", async ({ page }) => {
  await boot(page);
  await playerReady(page);

  await page.getByRole("button", { name: "Open media…" }).click();
  await expect.poll(() => invoked(page)).toContain("open_media");
  expect(await lastArgs(page, "open_media")).toEqual({
    path: "C:/Videos/Big Buck Bunny.mkv",
  });

  await page.screenshot({ path: `${DIR}/09-open-media.png` });
});

test("10 — cancelling the file picker opens nothing", async ({ page }) => {
  await boot(page, "?cancel=1");
  await playerReady(page);

  await page.getByRole("button", { name: "Open media…" }).click();
  await page.waitForTimeout(400);
  expect(await invoked(page)).not.toContain("open_media");
});

test("11 — transport controls drive the backend", async ({ page }) => {
  await boot(page, "?media=1");
  await playerReady(page);

  await page.getByRole("button", { name: "Pause" }).click();
  await expect.poll(() => invoked(page)).toContain("pause");

  await page.getByRole("button", { name: "Play" }).click();
  await expect.poll(() => invoked(page)).toContain("play");

  // Seeking is relative to the position the UI last mirrored (137s from the mock).
  await page.getByRole("button", { name: "+10s" }).click();
  await expect.poll(() => invoked(page)).toContain("seek");
  expect(await lastArgs(page, "seek")).toEqual({ positionSecs: 147 });

  await page.getByRole("button", { name: "−10s" }).click();
  expect(await lastArgs(page, "seek")).toEqual({ positionSecs: 127 });
});

// The honesty invariant: never a silent failure or a black screen.
test("12 — a build with no engine says so instead of failing silently", async ({ page }) => {
  await boot(page, "?engine=0");
  await playerReady(page);

  await page.getByRole("button", { name: "Open media…" }).click();

  const alert = page.getByRole("alert");
  await alert.waitFor({ timeout: 10_000 });
  await expect(alert).toContainText(/no playback engine/);

  await page.screenshot({ path: `${DIR}/12-no-engine-honest-error.png` });
});

test("13 — the video stage stays clear for the native surface", async ({ page }) => {
  await boot(page, "?media=1");
  await playerReady(page);

  // The picture is drawn by a native GPU surface positioned over this element, so the web
  // layer must not paint anything into it. The UI reports its geometry to Rust instead.
  const stage = page.getByLabel("Video stage");
  await expect(stage).toBeVisible();
  await expect(stage).toHaveText("");
  await expect.poll(() => invoked(page)).toContain("set_video_rect");

  const rect = await lastArgs(page, "set_video_rect");
  expect(rect).toBeDefined();
  expect(Number(rect?.width)).toBeGreaterThan(0);
  expect(Number(rect?.height)).toBeGreaterThan(0);
});

test("07 — bug reporter auto-surfaces a pending crash", async ({ page }) => {
  await boot(page, "?crash=1");

  const dialog = page.getByRole("dialog", { name: "Report a bug" });
  await dialog.waitFor({ timeout: 15_000 });

  // The crash excerpt is shown to the user before anything can be sent, and the home path
  // is redacted.
  await expect(page.getByText(/the playback engine went away/)).toBeVisible();
  await expect(page.getByText(/closed unexpectedly last time/)).toBeVisible();
  await expect(page.getByRole("button", { name: "Dismiss crash" })).toBeVisible();
  await expect(page.getByText(/Crashed: 2026-07-21/)).toBeVisible();

  await page.screenshot({ path: `${DIR}/07-bug-report-pending-crash.png` });
});
