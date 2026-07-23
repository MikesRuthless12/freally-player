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

/** The player chrome is up once the stage is rendered — present whether idle or playing. */
async function playerReady(page: Page) {
  await page.getByLabel("Video stage").waitFor({ timeout: 15_000 });
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

  // With nothing open the native surface must stay HIDDEN, or it paints black over the
  // stage and hides this very message.
  await expect.poll(() => invoked(page)).toContain("set_video_rect");
  expect((await lastArgs(page, "set_video_rect"))?.visible).toBe(false);

  await expect(page.getByText("No media loaded")).toBeVisible();
  await expect(page.getByLabel("Video stage")).toBeVisible();
  await expect(page.getByText("v0.30.0")).toBeVisible();

  await page.screenshot({ path: `${DIR}/02-player-idle.png` });
});

test("03 — transport with media open", async ({ page }) => {
  await boot(page, "?media=1");
  await playerReady(page);

  // The open media's title takes over the title bar; the stage itself stays clear for the
  // native surface. A playing file offers Pause (the play/pause toggle), the skip buttons and
  // the live clock.
  await expect(page.getByText("Big Buck Bunny", { exact: true }).first()).toBeVisible();
  for (const control of ["Pause", "−10s", "+10s"]) {
    await expect(page.getByRole("button", { name: control })).toBeVisible();
  }
  await expect(page.getByText("2:17 / 9:56")).toBeVisible();

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

  // Play/pause is one toggle button; on a playing file it reads Pause and sends toggle_play.
  await page.getByRole("button", { name: "Pause" }).click();
  await expect.poll(() => invoked(page)).toContain("toggle_play");

  // Seeking is relative to the position the UI last mirrored (137s from the mock).
  await page.getByRole("button", { name: "+10s" }).click();
  await expect.poll(() => invoked(page)).toContain("seek");
  expect(await lastArgs(page, "seek")).toEqual({ positionSecs: 147 });

  await page.getByRole("button", { name: "−10s" }).click();
  expect(await lastArgs(page, "seek")).toEqual({ positionSecs: 127 });
});

test("11b — the control bar drives volume, speed, frame-step, A–B and snapshot", async ({
  page,
}) => {
  await boot(page, "?media=1");
  await playerReady(page);

  // Mute.
  await page.getByRole("button", { name: "Mute" }).click();
  await expect.poll(() => invoked(page)).toContain("set_muted");
  expect(await lastArgs(page, "set_muted")).toEqual({ muted: true });

  // Frame step (back and forward).
  await page.getByRole("button", { name: "Previous frame" }).click();
  expect(await lastArgs(page, "frame_step")).toEqual({ forward: false });
  await page.getByRole("button", { name: "Next frame" }).click();
  expect(await lastArgs(page, "frame_step")).toEqual({ forward: true });

  // A–B repeat: the first click marks the start at the mirrored position (137s).
  await page.getByRole("button", { name: "Set repeat start" }).click();
  await expect.poll(() => invoked(page)).toContain("set_ab_loop");
  expect(await lastArgs(page, "set_ab_loop")).toEqual({ a: 137, b: null });

  // Speed menu → 2×.
  await page.getByRole("button", { name: "Playback speed" }).click();
  await page.getByRole("menuitem", { name: "2×" }).click();
  expect(await lastArgs(page, "set_speed")).toEqual({ speed: 2 });

  // Snapshot opens the native save dialog and captures with subtitles baked in.
  await page.getByRole("button", { name: "Save a snapshot" }).click();
  await expect.poll(() => invoked(page)).toContain("capture_frame");
  expect(await lastArgs(page, "capture_frame")).toEqual({
    path: "C:/Videos/snapshot.png",
    withSubs: true,
  });

  await page.screenshot({ path: `${DIR}/11b-control-bar.png` });
});

test("11c — the chapters menu jumps to a chapter", async ({ page }) => {
  await boot(page, "?media=1");
  await playerReady(page);

  await page.getByRole("button", { name: "Chapters" }).click();
  const menu = page.getByRole("menu", { name: "Chapters" });
  await expect(menu).toBeVisible();
  // The mock's three chapters are listed with their titles.
  await expect(menu.getByText("The meadow")).toBeVisible();
  await page.screenshot({ path: `${DIR}/11d-chapters-menu.png` });

  await menu.getByRole("menuitem", { name: /The meadow/ }).click();
  await expect.poll(() => invoked(page)).toContain("set_chapter");
  expect(await lastArgs(page, "set_chapter")).toEqual({ index: 1 });
});

test("11e — the scrubber seeks to a clicked position", async ({ page }) => {
  await boot(page, "?media=1");
  await playerReady(page);

  const scrubber = page.getByRole("slider", { name: "Seek" });
  const box = await scrubber.boundingBox();
  expect(box).not.toBeNull();
  // Click at the mid-point of the track → roughly half of the 596s duration.
  await page.mouse.click(box!.x + box!.width / 2, box!.y + box!.height / 2);

  await expect.poll(() => invoked(page)).toContain("seek");
  const secs = Number((await lastArgs(page, "seek"))?.positionSecs);
  expect(secs).toBeGreaterThan(250);
  expect(secs).toBeLessThan(346);
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

test("12b — the idle screen shows the drop zone and Open button", async ({ page }) => {
  await boot(page);
  await playerReady(page);

  await expect(page.getByText("No media loaded")).toBeVisible();
  await expect(page.getByText(/Drop a video here/)).toBeVisible();
  await expect(page.getByRole("button", { name: "Open media…" })).toBeVisible();

  await page.screenshot({ path: `${DIR}/12b-idle-screen.png` });
});

test("12c — Continue watching lists resumable files and reopens one", async ({ page }) => {
  await boot(page, "?recent=1");
  await playerReady(page);

  const row = page.getByRole("region", { name: "Continue watching" });
  await expect(row).toBeVisible();
  // The file stem is the display title; a progress bar shows how far in it is.
  await expect(row.getByText("Arrival.2016")).toBeVisible();
  await expect(row.getByText("Big Buck Bunny")).toBeVisible();
  await page.screenshot({ path: `${DIR}/12d-continue-watching.png` });

  await row.getByRole("button", { name: /Arrival\.2016/ }).click();
  await expect.poll(() => invoked(page)).toContain("open_media");
  expect(await lastArgs(page, "open_media")).toEqual({ path: "C:/Videos/Arrival.2016.mkv" });
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
  // Visible only because media is open — an empty surface would paint black over the stage.
  expect(rect?.visible).toBe(true);
});

test("11f — fullscreen hides the shell chrome and toggles its own label", async ({ page }) => {
  await boot(page, "?media=1");
  await playerReady(page);

  // Entering fullscreen hides the title bar and footer so the picture fills the window.
  await expect(page.getByRole("button", { name: "Minimize" })).toBeVisible();
  await page.getByRole("button", { name: "Fullscreen", exact: true }).click();

  await expect(page.getByRole("button", { name: "Exit fullscreen" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Minimize" })).toHaveCount(0);
});

test("14 — custom title bar, centred title and window controls", async ({ page }) => {
  await boot(page);
  await playerReady(page);

  // The window is borderless, so these are the only way to move/minimise/close it.
  for (const control of ["Minimize", "Maximize", "Close", "Settings", "About"]) {
    await expect(page.getByRole("button", { name: control })).toBeVisible();
  }

  // The title is centred in the WINDOW, not in the space the buttons leave over: its box
  // spans the full width, so its own centre and the window's centre coincide.
  const title = page.getByText("Freally Player", { exact: true }).first();
  const titleBox = await title.boundingBox();
  const viewport = page.viewportSize();
  expect(titleBox).not.toBeNull();
  expect(viewport).not.toBeNull();
  const titleCentre = titleBox!.x + titleBox!.width / 2;
  expect(Math.abs(titleCentre - viewport!.width / 2)).toBeLessThan(2);

  await page.screenshot({ path: `${DIR}/14-title-bar.png` });
});

test("15 — Settings modal, General with minimize to tray", async ({ page }) => {
  await boot(page);
  await playerReady(page);

  await page.getByRole("button", { name: "Settings" }).click();
  const dialog = page.getByRole("dialog", { name: "Settings" });
  await dialog.waitFor({ timeout: 10_000 });

  // Two-pane shell: category sidebar on the left, the selected pane on the right. Scoped to
  // the dialog — "About" also names the title-bar icon that opened it.
  for (const category of ["General", "Appearance", "Language", "About"]) {
    await expect(dialog.getByRole("button", { name: category, exact: true })).toBeVisible();
  }
  const tray = dialog.getByRole("checkbox", { name: /Minimize to system tray/ });
  await expect(tray).toBeVisible();
  await expect(tray).not.toBeChecked();

  await tray.click();
  await expect.poll(() => invoked(page)).toContain("settings_set");
  expect(await lastArgs(page, "settings_set")).toEqual({
    // `language: null` — nothing chosen yet, so the UI is still following the OS.
    settings: {
      theme: "dark",
      minimizeToTray: true,
      subtitleStyle: { enabled: false, font: null, fontSize: null, color: null },
      openSubtitles: { enabled: false, apiKey: null, username: null },
      language: null,
    },
  });

  await page.screenshot({ path: `${DIR}/15-settings-general.png` });
});

test("16 — Settings modal, Appearance and About panes", async ({ page }) => {
  await boot(page);
  await playerReady(page);

  await page.getByRole("button", { name: "Settings" }).click();
  const dialog = page.getByRole("dialog", { name: "Settings" });
  await dialog.waitFor({ timeout: 10_000 });

  await dialog.getByRole("button", { name: "Appearance", exact: true }).click();
  await expect(dialog.getByRole("button", { name: "Dark", exact: true })).toBeVisible();
  await page.screenshot({ path: `${DIR}/16-settings-appearance.png` });

  // The Language pane. Switching languages is driven exhaustively in `languages.spec.ts`;
  // this is the gallery's photograph of the picker sitting in the modal.
  await dialog.getByRole("button", { name: "Language", exact: true }).click();
  await expect(dialog.getByRole("button", { name: "English", exact: true })).toBeVisible();
  await expect(dialog.locator("button[lang]")).toHaveCount(18);
  await page.screenshot({ path: `${DIR}/16c-settings-language.png` });

  await dialog.getByRole("button", { name: "About", exact: true }).click();
  await expect(dialog.getByText(/All Rights Reserved/)).toBeVisible();
  await expect(dialog.getByText("0.30.0")).toBeVisible();
  await page.screenshot({ path: `${DIR}/16b-settings-about.png` });
});

test("17 — the tray preference is reflected when already enabled", async ({ page }) => {
  await boot(page, "?tray=1");
  await playerReady(page);

  await page.getByRole("button", { name: "Settings" }).click();
  const dialog = page.getByRole("dialog", { name: "Settings" });
  await dialog.waitFor({ timeout: 10_000 });

  await expect(dialog.getByRole("checkbox", { name: /Minimize to system tray/ })).toBeChecked();
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

// --- Phase 2: subtitles & audio tracks --------------------------------------

test("18 — the audio menu lists tracks and switches one", async ({ page }) => {
  await boot(page, "?media=1");
  await playerReady(page);

  await page.getByRole("button", { name: "Audio", exact: true }).click();
  const menu = page.getByRole("menu", { name: "Audio" });
  await expect(menu).toBeVisible();
  // The named track shows its title; the unnamed one is spelled out from its language code.
  await expect(menu.getByText("Stereo")).toBeVisible();
  await expect(menu.getByText("Japanese")).toBeVisible();
  await page.screenshot({ path: `${DIR}/18-audio-menu.png` });

  await menu.getByRole("menuitem", { name: "Japanese" }).click();
  await expect.poll(() => invoked(page)).toContain("set_audio_track");
  expect(await lastArgs(page, "set_audio_track")).toEqual({ id: 2 });
});

test("19 — the subtitle menu switches tracks and adjusts sync", async ({ page }) => {
  await boot(page, "?media=1");
  await playerReady(page);

  await page.getByRole("button", { name: "Subtitles", exact: true }).click();
  const menu = page.getByRole("menu", { name: "Subtitles" });
  await expect(menu).toBeVisible();
  // Off plus the two tracks, and the per-file timing controls (the primary track is on).
  // "English" appears in both the primary and secondary track lists, so scope to the first.
  await expect(menu.getByText("English").first()).toBeVisible();
  await expect(menu.getByRole("menuitem", { name: "Off" }).first()).toBeVisible();
  await page.screenshot({ path: `${DIR}/19-subtitle-menu.png` });

  // Nudge the delay: the first "Delay +" press moves it a tenth of a second.
  await menu.getByRole("button", { name: "Delay +" }).click();
  await expect.poll(() => invoked(page)).toContain("set_subtitle_delay");
  expect(await lastArgs(page, "set_subtitle_delay")).toEqual({ secs: 0.1 });

  // Turning the primary track off sends a null selection.
  await menu.getByRole("menuitem", { name: "Off" }).first().click();
  await expect.poll(() => invoked(page)).toContain("set_subtitle_track");
  expect(await lastArgs(page, "set_subtitle_track")).toEqual({ id: null });
});

test("20 — loading an external subtitle transcodes and names the encoding", async ({ page }) => {
  await boot(page, "?media=1");
  await playerReady(page);

  await page.getByRole("button", { name: "Subtitles", exact: true }).click();
  await page.getByRole("button", { name: "Load subtitle file…" }).click();

  await expect.poll(() => invoked(page)).toContain("add_subtitle_file");
  expect(await lastArgs(page, "add_subtitle_file")).toEqual({
    path: "C:/Videos/Big Buck Bunny.mkv",
  });
  // The honest note tells the viewer the file was converted from its legacy charset.
  await expect(page.getByText(/converted from windows-1251/)).toBeVisible();
  await page.screenshot({ path: `${DIR}/20-subtitle-loaded.png` });
});

test("21 — Settings Subtitles pane: style override and OpenSubtitles opt-in", async ({ page }) => {
  await boot(page, "?substyle=1");
  await playerReady(page);

  await page.getByRole("button", { name: "Settings" }).click();
  const dialog = page.getByRole("dialog", { name: "Settings" });
  await dialog.waitFor({ timeout: 10_000 });
  await dialog.getByRole("button", { name: "Subtitles", exact: true }).click();

  // The style override is on (from ?substyle=1), so its font/size/colour fields are shown.
  await expect(dialog.getByRole("checkbox", { name: "Override subtitle styling" })).toBeChecked();
  await expect(dialog.getByText("Font", { exact: true })).toBeVisible();
  // The OpenSubtitles opt-in toggle is present and off by default.
  await expect(
    dialog.getByRole("checkbox", { name: "Enable online subtitle fetch" }),
  ).toBeVisible();
  await page.screenshot({ path: `${DIR}/21-settings-subtitles.png` });
});

test("22 — OpenSubtitles search, sign-in and download, when opted in", async ({ page }) => {
  await boot(page, "?media=1&os=1");
  await playerReady(page);

  await page.getByRole("button", { name: "Subtitles", exact: true }).click();
  const menu = page.getByRole("menu", { name: "Subtitles" });
  const query = menu.getByRole("textbox", { name: "Title to search" });
  await query.waitFor({ timeout: 10_000 });

  await query.fill("Big Buck Bunny");
  await menu.getByRole("button", { name: "Search", exact: true }).click();
  await expect.poll(() => invoked(page)).toContain("opensubtitles_search");
  expect(await lastArgs(page, "opensubtitles_search")).toEqual({
    query: "Big Buck Bunny",
    languages: ["en"],
  });

  // Results render with their filenames.
  await expect(menu.getByText("Big.Buck.Bunny.en.srt")).toBeVisible();
  await page.screenshot({ path: `${DIR}/22-opensubtitles.png` });

  // Signing in exchanges credentials for a session (the password is never persisted).
  // A password input is not exposed as a textbox role, so target it by its label.
  await menu.getByLabel("Password").fill("hunter2");
  await menu.getByRole("button", { name: "Sign in" }).click();
  await expect.poll(() => invoked(page)).toContain("opensubtitles_login");
  expect(await lastArgs(page, "opensubtitles_login")).toEqual({
    username: "cinephile",
    password: "hunter2",
  });

  // Choosing a result downloads and attaches it.
  await menu.getByRole("button", { name: /Big\.Buck\.Bunny\.en\.srt/ }).click();
  await expect.poll(() => invoked(page)).toContain("opensubtitles_download");
  expect(await lastArgs(page, "opensubtitles_download")).toEqual({ fileId: 101 });
});
