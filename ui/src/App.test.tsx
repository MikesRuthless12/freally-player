import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import App from "./App";
import { LOCALES } from "./i18n/locales";
import type { PlaybackState } from "./ipc/types";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/plugin-process", () => ({ exit: vi.fn() }));

// The window is borderless, so the title bar drives the real window API.
const windowApi = vi.hoisted(() => ({
  minimize: vi.fn(() => Promise.resolve()),
  maximize: vi.fn(() => Promise.resolve()),
  unmaximize: vi.fn(() => Promise.resolve()),
  close: vi.fn(() => Promise.resolve()),
  isMaximized: vi.fn(() => Promise.resolve(false)),
  onResized: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => windowApi }));

const pickFile = vi.hoisted(() => vi.fn());
const saveFile = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: pickFile, save: saveFile }));

// The player subscribes to webview drag-drop; give it an inert handle in the unit environment.
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({ onDragDropEvent: () => Promise.resolve(() => {}) }),
}));

/** Captures `listen(event, handler)` so tests can push events the way Rust would. */
const listeners = vi.hoisted(() => new Map<string, (event: { payload: unknown }) => void>());
const listen = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/event", () => ({ listen }));

const IDLE: PlaybackState = {
  status: "idle",
  positionSecs: 0,
  media: null,
  volume: 100,
  muted: false,
  speed: 1,
  bufferedSecs: 0,
  abLoop: { a: null, b: null },
  audioId: null,
  subtitle: { id: null, secondaryId: null, visible: true, delaySecs: 0, pos: 100, scale: 1 },
};

/** A playing snapshot for `title`, resuming the shared idle defaults. */
function playing(title: string, positionSecs: number, durationSecs: number | null): PlaybackState {
  return {
    ...IDLE,
    status: "playing",
    positionSecs,
    media: { path: `C:/v/${title}.mkv`, title, durationSecs, chapters: [], tracks: [] },
  };
}

/** Canned backend: every command answers, EULA accepted and no pending crash by default. */
function backend(overrides: Record<string, unknown> = {}) {
  const responses: Record<string, unknown> = {
    app_info: { name: "Freally Player", version: "0.1.0" },
    settings_get: {
      theme: "dark",
      minimizeToTray: false,
      subtitleStyle: { enabled: false, font: null, fontSize: null, color: null },
      openSubtitles: { enabled: false, apiKey: null, username: null },
      language: null,
    },
    settings_set: undefined,
    eula_status: { version: "2026-07-01", text: "# EULA\n\nTerms.", accepted: true },
    eula_accept: undefined,
    get_state: IDLE,
    play: undefined,
    pause: undefined,
    seek: undefined,
    toggle_play: undefined,
    set_video_rect: undefined,
    recent_watch: [],
    bug_report_context: {
      appVersion: "0.1.0",
      os: "windows",
      arch: "x86_64",
      diagnostics: "App: Freally Player 0.1.0\nOS: windows / x86_64\n",
      pendingCrash: null,
    },
    ...overrides,
  };
  invoke.mockImplementation((command: string) => {
    if (!(command in responses)) return Promise.reject(new Error(`unexpected command: ${command}`));
    const value = responses[command];
    return value instanceof Error ? Promise.reject(value) : Promise.resolve(value);
  });
}

/**
 * Push a `player://…` event exactly as the Rust side would emit it.
 *
 * Waits for the UI to have subscribed first. Without that, `emit` is a silent no-op whenever
 * the subscription effect has not run yet, and the assertion that follows fails on timing
 * rather than on behaviour. (In production the same ordering holds: Rust only emits once the
 * app is up and listening.)
 */
async function emit(event: string, payload: unknown) {
  await waitFor(() => expect(listeners.has(event)).toBe(true));
  listeners.get(event)?.({ payload });
}

/**
 * Match the transport status line, which renders title/status/time as one span built from
 * several interpolations — so a plain string match would report "broken up by multiple
 * elements".
 */
function transportLine(fragment: string) {
  return (_content: string, element: Element | null) =>
    element?.tagName === "SPAN" && (element.textContent ?? "").includes(fragment);
}

describe("App", () => {
  beforeEach(() => {
    invoke.mockReset();
    pickFile.mockReset();
    listeners.clear();
    listen.mockReset();
    listen.mockImplementation((event: string, handler: (e: { payload: unknown }) => void) => {
      listeners.set(event, handler);
      // Remove only THIS handler, the way Tauri's per-listener id does. Deleting by event
      // name lets a late async cleanup unregister a newer subscription.
      return Promise.resolve(() => {
        if (listeners.get(event) === handler) listeners.delete(event);
      });
    });
    document.documentElement.removeAttribute("data-theme");
    document.documentElement.removeAttribute("lang");
    document.documentElement.removeAttribute("dir");
  });

  it("shows the version reported by the app_info command", async () => {
    backend();
    render(<App />);
    expect(await screen.findByText("v0.1.0")).toBeInTheDocument();
  });

  it("stays usable and says so plainly when the version cannot be read", async () => {
    backend({ app_info: new Error("ipc unavailable") });
    render(<App />);

    expect(await screen.findByText("version unavailable")).toBeInTheDocument();
    expect(screen.getByLabelText("Video stage")).toBeInTheDocument();
  });

  describe("the EULA gate", () => {
    it("blocks the player until the agreement is accepted", async () => {
      backend({
        eula_status: { version: "2026-07-01", text: "# EULA\n\nTerms.", accepted: false },
      });

      render(<App />);

      expect(await screen.findByRole("button", { name: "I Agree" })).toBeInTheDocument();
      expect(screen.queryByLabelText("Video stage")).not.toBeInTheDocument();
    });

    it("opens the player once the user agrees", async () => {
      backend({
        eula_status: { version: "2026-07-01", text: "# EULA\n\nTerms.", accepted: false },
      });
      render(<App />);

      // "I Agree" stays disabled until the agreement has been scrolled through, so wait for
      // it to become enabled rather than clicking a no-op.
      const agree = await screen.findByRole("button", { name: "I Agree" });
      await waitFor(() => expect(agree).toBeEnabled());
      await userEvent.click(agree);

      expect(await screen.findByLabelText("Video stage")).toBeInTheDocument();
      expect(invoke).toHaveBeenCalledWith("eula_accept");
    });

    // Fail closed: a backend we cannot reach must not be treated as "accepted".
    it("keeps the gate up when the EULA status cannot be read", async () => {
      backend({ eula_status: new Error("no backend") });

      render(<App />);

      expect(await screen.findByRole("button", { name: "I Agree" })).toBeInTheDocument();
      expect(screen.queryByLabelText("Video stage")).not.toBeInTheDocument();
    });
  });

  describe("theming", () => {
    it("applies the stored light theme to the document root", async () => {
      backend({ settings_get: { theme: "light", minimizeToTray: false } });
      render(<App />);

      await waitFor(() =>
        expect(document.documentElement.getAttribute("data-theme")).toBe("light"),
      );
    });

    it("toggles between dark and light and persists the choice", async () => {
      backend();
      render(<App />);

      await userEvent.click(await screen.findByRole("button", { name: "Switch to light mode" }));

      expect(document.documentElement.getAttribute("data-theme")).toBe("light");
      expect(invoke).toHaveBeenCalledWith("settings_set", {
        settings: {
          theme: "light",
          minimizeToTray: false,
          subtitleStyle: { enabled: false, font: null, fontSize: null, color: null },
          openSubtitles: { enabled: false, apiKey: null, username: null },
          language: null,
        },
      });

      await userEvent.click(await screen.findByRole("button", { name: "Switch to dark mode" }));
      expect(document.documentElement.hasAttribute("data-theme")).toBe(false);
    });
  });

  describe("the playback bus", () => {
    it("mirrors transport state pushed over player://state", async () => {
      backend();
      render(<App />);
      await screen.findByText("No media loaded");

      await emit("player://state", playing("Arrival", 75, 7200));

      // The open media's title takes over the title bar, and the control bar reflects the
      // playing status by offering Pause and the live clock.
      expect(await screen.findByText("Arrival")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Pause" })).toBeInTheDocument();
      expect(screen.getByText(transportLine("1:15 / 2:00:00"))).toBeInTheDocument();
    });

    it("sends the picked path through open_media rather than reading the file itself", async () => {
      backend({ open_media: { path: "C:/v/clip.mkv", title: "clip", durationSecs: null } });
      pickFile.mockResolvedValue("C:/v/clip.mkv");
      render(<App />);

      await userEvent.click(await screen.findByRole("button", { name: "Open media…" }));

      await waitFor(() =>
        expect(invoke).toHaveBeenCalledWith("open_media", { path: "C:/v/clip.mkv" }),
      );
    });

    it("does nothing when the file dialog is cancelled", async () => {
      backend();
      pickFile.mockResolvedValue(null);
      render(<App />);

      await userEvent.click(await screen.findByRole("button", { name: "Open media…" }));

      expect(invoke).not.toHaveBeenCalledWith("open_media", expect.anything());
    });

    // Honesty invariant: a build with no decode backend says so, it does not go black.
    it("shows the engine's refusal verbatim instead of failing silently", async () => {
      backend({
        open_media: new Error(
          "this build has no playback engine — it was built without the libmpv backend",
        ),
      });
      pickFile.mockResolvedValue("C:/v/clip.mkv");
      render(<App />);

      await userEvent.click(await screen.findByRole("button", { name: "Open media…" }));

      expect(await screen.findByRole("alert")).toHaveTextContent("no playback engine");
    });

    it("seeks relative to the position it last mirrored", async () => {
      backend();
      render(<App />);
      await screen.findByText("No media loaded");

      await emit("player://state", playing("clip", 40, null));
      await screen.findByRole("button", { name: "Pause" });

      await userEvent.click(screen.getByRole("button", { name: "+10s" }));
      expect(invoke).toHaveBeenCalledWith("seek", { positionSecs: 50 });

      // Never below zero, whatever the engine last reported.
      await userEvent.click(screen.getByRole("button", { name: "−10s" }));
      expect(invoke).toHaveBeenCalledWith("seek", { positionSecs: 30 });
    });
  });

  describe("the custom title bar", () => {
    // The window is borderless, so these buttons are the ONLY way to minimise or close it.
    it("drives the real window with its own controls", async () => {
      backend();
      render(<App />);
      await screen.findByLabelText("Video stage");

      await userEvent.click(screen.getByRole("button", { name: "Minimize" }));
      expect(windowApi.minimize).toHaveBeenCalled();

      await userEvent.click(screen.getByRole("button", { name: "Maximize" }));
      expect(windowApi.maximize).toHaveBeenCalled();

      await userEvent.click(screen.getByRole("button", { name: "Close" }));
      expect(windowApi.close).toHaveBeenCalled();
    });

    it("keeps its controls on the EULA gate, where they are the only way out", async () => {
      backend({
        eula_status: { version: "2026-07-21", text: "# EULA\n\nTerms.", accepted: false },
      });
      render(<App />);
      await screen.findByRole("button", { name: "I Agree" });

      expect(screen.getByRole("button", { name: "Close" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Minimize" })).toBeInTheDocument();
      // Nothing behind the gate is usable yet, so these stay hidden.
      expect(screen.queryByRole("button", { name: "Settings" })).not.toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "About" })).not.toBeInTheDocument();
    });
  });

  describe("the Settings modal", () => {
    it("opens from the gear and persists minimize-to-tray", async () => {
      backend();
      render(<App />);
      await screen.findByLabelText("Video stage");

      await userEvent.click(screen.getByRole("button", { name: "Settings" }));
      const dialog = await screen.findByRole("dialog", { name: "Settings" });
      expect(dialog).toBeInTheDocument();

      await userEvent.click(screen.getByRole("checkbox", { name: /Minimize to system tray/ }));

      expect(invoke).toHaveBeenCalledWith("settings_set", {
        settings: {
          theme: "dark",
          minimizeToTray: true,
          subtitleStyle: { enabled: false, font: null, fontSize: null, color: null },
          openSubtitles: { enabled: false, apiKey: null, username: null },
          language: null,
        },
      });
    });

    it("opens on the About pane from the info icon", async () => {
      backend();
      render(<App />);
      await screen.findByLabelText("Video stage");

      await userEvent.click(screen.getByRole("button", { name: "About" }));

      await screen.findByRole("dialog", { name: "Settings" });
      expect(screen.getByRole("heading", { name: "About" })).toBeInTheDocument();
      expect(screen.getByText(/All Rights Reserved/)).toBeInTheDocument();
    });

    it("closes on Escape", async () => {
      backend();
      render(<App />);
      await screen.findByLabelText("Video stage");

      await userEvent.click(screen.getByRole("button", { name: "Settings" }));
      await screen.findByRole("dialog", { name: "Settings" });

      await userEvent.keyboard("{Escape}");
      await waitFor(() =>
        expect(screen.queryByRole("dialog", { name: "Settings" })).not.toBeInTheDocument(),
      );
    });
  });

  describe("the language picker", () => {
    /** The autonyms as the Language pane renders them, in document order. */
    const renderedOrder = () => {
      const autonyms: string[] = LOCALES.map((l) => l.autonym);
      return screen
        .getAllByRole("button")
        .map((button) => button.textContent ?? "")
        .filter((text) => autonyms.includes(text));
    };

    const openLanguagePane = async () => {
      await screen.findByLabelText("Video stage");
      await userEvent.click(screen.getByRole("button", { name: "Settings" }));
      await userEvent.click(await screen.findByRole("button", { name: "Language" }));
    };

    it("applies a language immediately and persists it", async () => {
      backend();
      render(<App />);
      await openLanguagePane();

      await userEvent.click(screen.getByRole("button", { name: "日本語" }));

      expect(invoke).toHaveBeenCalledWith("settings_set", {
        settings: {
          theme: "dark",
          minimizeToTray: false,
          subtitleStyle: { enabled: false, font: null, fontSize: null, color: null },
          openSubtitles: { enabled: false, apiKey: null, username: null },
          language: "ja",
        },
      });
      // Actually in Japanese, in the same session — no reload, no restart. The idle screen's
      // Open button behind the modal switches with the rest of the shell.
      expect(await screen.findByRole("button", { name: "メディアを開く…" })).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "Open media…" })).not.toBeInTheDocument();
    });

    it("starts in the stored language", async () => {
      backend({ settings_get: { theme: "dark", minimizeToTray: false, language: "fr" } });
      render(<App />);

      // The idle screen's Open button renders in the stored language from the first paint.
      expect(await screen.findByRole("button", { name: "Ouvrir un média…" })).toBeInTheDocument();
      await waitFor(() => expect(document.documentElement.lang).toBe("fr"));
    });

    /**
     * `<html lang>` is what `styles/fonts.css` keys its per-script font stacks off, so this is
     * the difference between Japanese rendering in JP letterforms and in Simplified Chinese
     * ones. `dir` mirrors the shell for Arabic — the only RTL locale of the 18.
     */
    it("stamps the document with the language and its direction", async () => {
      backend({ settings_get: { theme: "dark", minimizeToTray: false, language: "ar" } });
      render(<App />);
      await screen.findByLabelText("منطقة الفيديو");

      await waitFor(() => expect(document.documentElement.lang).toBe("ar"));
      expect(document.documentElement.dir).toBe("rtl");
    });

    it("returns to left-to-right when leaving Arabic", async () => {
      backend({ settings_get: { theme: "dark", minimizeToTray: false, language: "ar" } });
      render(<App />);
      await waitFor(() => expect(document.documentElement.dir).toBe("rtl"));

      await userEvent.click(screen.getByRole("button", { name: "الإعدادات" }));
      await userEvent.click(await screen.findByRole("button", { name: "اللغة" }));
      await userEvent.click(screen.getByRole("button", { name: "Deutsch" }));

      await waitFor(() => expect(document.documentElement.dir).toBe("ltr"));
      expect(document.documentElement.lang).toBe("de");
    });

    /**
     * English first, then alphabetical by the language's own name — and the SAME order in
     * every language. Collation is locale-sensitive, so sorting this list at render time would
     * rearrange the picker under the user each time they switched (Arabic lifts العربية to the
     * top, Russian lifts Cyrillic, Chinese lifts the CJK names). The order is a baked-in
     * literal for exactly that reason; this holds the rendered list to it.
     */
    it("lists English first and keeps one order in every language", async () => {
      backend();
      render(<App />);
      await openLanguagePane();

      const expected = LOCALES.map((l) => l.autonym);
      expect(expected[0]).toBe("English");
      expect(renderedOrder()).toEqual(expected);

      for (const code of ["ar", "ja", "ru", "zh-CN"]) {
        const autonym = LOCALES.find((l) => l.code === code)!.autonym;
        await userEvent.click(screen.getByRole("button", { name: autonym }));
        expect(renderedOrder(), `the picker reordered itself in ${code}`).toEqual(expected);
      }
    });
  });

  describe("the bug reporter", () => {
    it("auto-surfaces the report when a crash is pending from the last run", async () => {
      backend({
        bug_report_context: {
          appVersion: "0.1.0",
          os: "windows",
          arch: "x86_64",
          diagnostics: "App: Freally Player 0.1.0\nOS: windows / x86_64\n",
          pendingCrash: "Crashed: 2026-07-21\nMessage: engine went away\n",
        },
      });

      render(<App />);

      expect(await screen.findByRole("dialog", { name: "Report a bug" })).toBeInTheDocument();
      // `find`, not `get`: the dialog loads its own context, so the crash excerpt appears a
      // tick after the dialog itself. A sync assertion here passes or fails on timing.
      expect(await screen.findByText(/engine went away/)).toBeInTheDocument();
    });

    it("stays closed when there is no pending crash, and opens on request", async () => {
      backend();
      render(<App />);

      await screen.findByLabelText("Video stage");
      expect(screen.queryByRole("dialog", { name: "Report a bug" })).not.toBeInTheDocument();

      await userEvent.click(screen.getByRole("button", { name: "Report a bug" }));
      expect(await screen.findByRole("dialog", { name: "Report a bug" })).toBeInTheDocument();
    });

    it("submits through the Rust command rather than opening a link itself", async () => {
      backend({ bug_report_submit: undefined });
      render(<App />);

      await userEvent.click(await screen.findByRole("button", { name: "Report a bug" }));
      await userEvent.click(await screen.findByRole("button", { name: "Open GitHub issue" }));

      expect(invoke).toHaveBeenCalledWith("bug_report_submit", {
        target: "github",
        description: "",
        includeCrash: false,
      });
    });
  });
});
