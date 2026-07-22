import { defineConfig, devices } from "@playwright/test";

// Visual-smoke "gallery": loads the REAL built UI in Chromium with a mocked Tauri IPC and
// screenshots every feature panel. One screenshot per panel = a rendering confirmation.
//
// UI-render coverage only — the engine, the native video surface and everything needing real
// hardware are covered by the per-OS `cargo test` suite and by `Live-To-Do-List.md`.
export default defineConfig({
  testDir: "./e2e",
  outputDir: "./e2e/.output",
  fullyParallel: false,
  workers: 1,
  forbidOnly: !!process.env.CI,
  reporter: [["list"]],
  use: {
    baseURL: "http://localhost:4173",
    viewport: { width: 1440, height: 900 },
    deviceScaleFactor: 1,
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    // `preview` serves the built `dist` — run `npm run build` first.
    command: "npm run preview -- --port 4173 --strictPort",
    url: "http://localhost:4173",
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
