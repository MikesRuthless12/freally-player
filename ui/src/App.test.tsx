import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import App from "./App";
import type { PlaybackState } from "./ipc/types";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/plugin-process", () => ({ exit: vi.fn() }));

const pickFile = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: pickFile }));

/** Captures `listen(event, handler)` so tests can push events the way Rust would. */
const listeners = vi.hoisted(() => new Map<string, (event: { payload: unknown }) => void>());
const listen = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/event", () => ({ listen }));

const IDLE: PlaybackState = { status: "idle", positionSecs: 0, media: null };

/** Canned backend: every command answers, EULA accepted and no pending crash by default. */
function backend(overrides: Record<string, unknown> = {}) {
  const responses: Record<string, unknown> = {
    app_info: { name: "Freally Player", version: "0.1.0" },
    theme_get: "dark",
    theme_set: undefined,
    eula_status: { version: "2026-07-01", text: "# EULA\n\nTerms.", accepted: true },
    eula_accept: undefined,
    get_state: IDLE,
    play: undefined,
    pause: undefined,
    seek: undefined,
    set_video_rect: undefined,
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
      backend({ theme_get: "light" });
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
      expect(invoke).toHaveBeenCalledWith("theme_set", { theme: "light" });

      await userEvent.click(await screen.findByRole("button", { name: "Switch to dark mode" }));
      expect(document.documentElement.hasAttribute("data-theme")).toBe(false);
    });
  });

  describe("the playback bus", () => {
    it("mirrors transport state pushed over player://state", async () => {
      backend();
      render(<App />);
      await screen.findByText("No media loaded");

      await emit("player://state", {
        status: "playing",
        positionSecs: 75,
        media: { path: "C:/v/Arrival.mkv", title: "Arrival", durationSecs: 7200 },
      } satisfies PlaybackState);

      expect(await screen.findByText(transportLine("Arrival"))).toBeInTheDocument();
      expect(screen.getByText(transportLine("playing · 1:15 / 2:00:00"))).toBeInTheDocument();
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

      await emit("player://state", {
        status: "playing",
        positionSecs: 40,
        media: { path: "C:/v/clip.mkv", title: "clip", durationSecs: null },
      } satisfies PlaybackState);
      await screen.findByText(transportLine("clip"));

      await userEvent.click(screen.getByRole("button", { name: "+10s" }));
      expect(invoke).toHaveBeenCalledWith("seek", { positionSecs: 50 });

      // Never below zero, whatever the engine last reported.
      await userEvent.click(screen.getByRole("button", { name: "−10s" }));
      expect(invoke).toHaveBeenCalledWith("seek", { positionSecs: 30 });
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
