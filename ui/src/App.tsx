import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useState } from "react";

import {
  appInfo,
  bugReportContext,
  eulaStatus,
  getState,
  settingsGet,
  settingsSet,
} from "./ipc/commands";
import { onPlayerState } from "./ipc/events";
import type { AppInfo, EulaStatus, PlaybackState, Theme, UserSettings } from "./ipc/types";
import { applyLocale, I18nContext, useTranslator } from "./i18n";
import { resolveLocale } from "./i18n/locales";
import { useTransport } from "./lib/transport";
import { TitleBar } from "./components/TitleBar";
import { BugReportDialog } from "./panels/BugReport";
import { EulaGate } from "./panels/EulaGate";
import { Player } from "./screens/Player";
import { SettingsModal, type CategoryId } from "./panels/Settings";

/** Dark is the CSS default (no attribute), so only light needs to be stamped on the root. */
function applyTheme(theme: Theme) {
  const root = document.documentElement;
  if (theme === "light") root.setAttribute("data-theme", "light");
  else root.removeAttribute("data-theme");
}

/** The transport at rest — full volume, normal speed — matching the Rust default. */
const IDLE: PlaybackState = {
  status: "idle",
  positionSecs: 0,
  media: null,
  volume: 100,
  muted: false,
  speed: 1,
  bufferedSecs: 0,
  abLoop: { a: null, b: null },
};

/**
 * The app shell: the first-run EULA gate, then the player screen. The shell owns the settings,
 * the EULA state, and the transport snapshot the UI mirrors from `player://state` events; the
 * player screen owns the stage and the transport chrome.
 */
export default function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [infoError, setInfoError] = useState<string | null>(null);
  const [eula, setEula] = useState<EulaStatus | null>(null);
  const [settings, setSettings] = useState<UserSettings>({
    theme: "dark",
    minimizeToTray: false,
    language: null,
  });
  const [settingsCategory, setSettingsCategory] = useState<CategoryId | null>(null);
  const [showBugReport, setShowBugReport] = useState(false);
  const [playback, setPlayback] = useState<PlaybackState>(IDLE);
  const [playbackError, setPlaybackError] = useState<string | null>(null);
  const [fullscreen, setFullscreen] = useState(false);

  const transport = useTransport(setPlaybackError);

  // The active locale is DERIVED from the stored setting rather than held as its own state:
  // one source of truth means the language and what is persisted can never drift apart. With
  // nothing stored — a first run — `resolveLocale` takes the OS's preference.
  const locale = resolveLocale(settings.language);
  const t = useTranslator(locale);

  // `<html lang>` is not only for assistive tech: `styles/fonts.css` keys its per-script font
  // stacks off `:lang()`, so this is what makes each language render in the right letterforms.
  // `dir` mirrors the shell for Arabic.
  useEffect(() => {
    applyLocale(locale);
  }, [locale]);

  useEffect(() => {
    let cancelled = false;

    appInfo().then(
      (result) => !cancelled && setInfo(result),
      (error: unknown) => !cancelled && setInfoError(String(error)),
    );

    settingsGet().then(
      (stored) => {
        if (cancelled) return;
        setSettings(stored);
        applyTheme(stored.theme);
      },
      // Settings we cannot read are not worth blocking on — dark is already applied.
      () => {},
    );

    eulaStatus().then(
      (status) => !cancelled && setEula(status),
      // Fail CLOSED in the shipped app: with no backend we cannot prove acceptance, so the
      // gate stays up rather than letting the app open unaccepted.
      (error: unknown) =>
        !cancelled && setEula({ version: "", text: String(error), accepted: false }),
    );

    return () => {
      cancelled = true;
    };
  }, []);

  const accepted = eula?.accepted ?? false;

  // Mirror the transport: read once, then follow events. Subscribing only after acceptance
  // keeps the gate free of any player state.
  useEffect(() => {
    if (!accepted) return;
    let cancelled = false;
    // The initial read and the event stream race. If an event lands first, the in-flight
    // `get_state` answer is already stale — applying it would rewind the transport to
    // whatever was true when we asked.
    let sawEvent = false;

    getState().then(
      (state) => {
        if (!cancelled && !sawEvent) setPlayback(state);
      },
      () => {},
    );
    const unlisten = onPlayerState((state) => {
      sawEvent = true;
      if (!cancelled) setPlayback(state);
    });

    return () => {
      cancelled = true;
      void unlisten.then((off) => off()).catch(() => {});
    };
  }, [accepted]);

  // A pending crash from the last run auto-surfaces the report — but only once the user is
  // actually in the app, never over the EULA gate.
  useEffect(() => {
    if (!accepted) return;
    let cancelled = false;
    bugReportContext().then(
      (ctx) => !cancelled && ctx.pendingCrash && setShowBugReport(true),
      () => {},
    );
    return () => {
      cancelled = true;
    };
  }, [accepted]);

  // Applied immediately so the UI reflects the choice while it persists.
  const applySettings = useCallback((next: UserSettings) => {
    setSettings(next);
    applyTheme(next.theme);
  }, []);

  const toggleTheme = useCallback(() => {
    const next: Theme = settings.theme === "dark" ? "light" : "dark";
    const updated = { ...settings, theme: next };
    applySettings(updated);
    settingsSet(updated).catch(() => {
      // Persisting failed; the session still honours the choice.
    });
  }, [settings, applySettings]);

  const toggleFullscreen = useCallback(() => {
    setFullscreen((current) => {
      const next = !current;
      getCurrentWindow()
        .setFullscreen(next)
        // If the OS refuses, snap the flag back so the chrome does not lie about its state.
        .catch(() => setFullscreen(current));
      return next;
    });
  }, []);

  // Escape leaves fullscreen — the one place the video fills the window and the title bar
  // (with its close button) is hidden.
  useEffect(() => {
    if (!fullscreen) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") toggleFullscreen();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [fullscreen, toggleFullscreen]);

  // The window is borderless, so the title bar is the ONLY way to move, minimise or close it —
  // it is present on every screen except fullscreen, where the video fills the window. It shows
  // the open media's title when there is one, and the product name otherwise.
  const windowTitle = playback.media?.title ?? "Freally Player";
  const chrome = (body: React.ReactNode, showActions: boolean, isFullscreen = false) => (
    <I18nContext.Provider value={t}>
      <div className="relative flex h-full flex-col bg-havoc-bg text-havoc-text">
        {!isFullscreen && (
          <TitleBar
            title={showActions ? windowTitle : "Freally Player"}
            showActions={showActions}
            onOpenSettings={() => setSettingsCategory("general")}
            onOpenAbout={() => setSettingsCategory("about")}
          />
        )}
        {body}
      </div>
    </I18nContext.Provider>
  );

  if (eula === null) {
    return chrome(<div className="flex-1" />, false);
  }
  if (!eula.accepted) {
    return chrome(
      <EulaGate status={eula} onAccepted={() => setEula({ ...eula, accepted: true })} />,
      false,
    );
  }

  return chrome(
    <>
      <Player
        playback={playback}
        error={playbackError}
        transport={transport}
        fullscreen={fullscreen}
        onToggleFullscreen={toggleFullscreen}
      />

      {!fullscreen && (
        <footer className="flex items-center justify-between gap-3 border-t border-havoc-border bg-havoc-panel px-4 py-2 text-xs">
          <span className="bg-gradient-to-r from-havoc-accent to-havoc-accent-2 bg-clip-text font-semibold text-transparent">
            Freally Player
          </span>
          <div className="flex items-center gap-3">
            <button
              type="button"
              onClick={() => setShowBugReport(true)}
              className="text-havoc-muted hover:text-havoc-text"
            >
              {t("footer-report-bug")}
            </button>
            <button
              type="button"
              onClick={toggleTheme}
              aria-label={
                settings.theme === "dark" ? t("footer-switch-to-light") : t("footer-switch-to-dark")
              }
              className="text-havoc-muted hover:text-havoc-text"
            >
              {settings.theme === "dark" ? t("footer-theme-light") : t("footer-theme-dark")}
            </button>
            {infoError ? (
              <span className="text-havoc-muted">{t("footer-version-unavailable")}</span>
            ) : (
              <span className="text-havoc-muted">{info ? `v${info.version}` : "…"}</span>
            )}
          </div>
        </footer>
      )}

      {settingsCategory && (
        <SettingsModal
          settings={settings}
          info={info}
          initialCategory={settingsCategory}
          onChange={applySettings}
          onClose={() => setSettingsCategory(null)}
        />
      )}

      {showBugReport && <BugReportDialog onClose={() => setShowBugReport(false)} />}
    </>,
    true,
    fullscreen,
  );
}
