import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useRef, useState } from "react";

import {
  appInfo,
  bugReportContext,
  eulaStatus,
  getState,
  openMedia,
  pause,
  play,
  seek,
  settingsGet,
  settingsSet,
  setVideoRect,
} from "./ipc/commands";
import { onPlayerState } from "./ipc/events";
import type { AppInfo, EulaStatus, PlaybackState, Theme, UserSettings } from "./ipc/types";
import { applyLocale, I18nContext, useTranslator } from "./i18n";
import { resolveLocale } from "./i18n/locales";
import { TitleBar } from "./components/TitleBar";
import { BugReportDialog } from "./panels/BugReport";
import { EulaGate } from "./panels/EulaGate";
import { SettingsModal, type CategoryId } from "./panels/Settings";

/** Dark is the CSS default (no attribute), so only light needs to be stamped on the root. */
function applyTheme(theme: Theme) {
  const root = document.documentElement;
  if (theme === "light") root.setAttribute("data-theme", "light");
  else root.removeAttribute("data-theme");
}

const IDLE: PlaybackState = { status: "idle", positionSecs: 0, media: null };

/** `1h 02m 03s`-free, scrubber-friendly `h:mm:ss` / `m:ss`. */
function formatTime(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  const s = String(total % 60).padStart(2, "0");
  const m = Math.floor(total / 60) % 60;
  const h = Math.floor(total / 3600);
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
}

/**
 * The app shell: the first-run EULA gate, then the video stage plus the transport.
 *
 * The stage is deliberately an empty region — from P0.3 the decoded video is drawn by a
 * native GPU surface composited *underneath* this webview, and the web layer only ever paints
 * chrome on top. No decoded pixels cross the IPC boundary; the UI mirrors the transport from
 * `player://state` events.
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

  // Keep the native video surface aligned with the stage element. The surface is a sibling
  // window drawn OVER the webview (WebView2's transparent pixels reveal the desktop, not a
  // window beneath it), so it must match this rect exactly or it would cover the chrome.
  const stageRef = useRef<HTMLElement | null>(null);
  const hasMedia = playback.media !== null;
  useEffect(() => {
    if (!accepted) return;
    const stage = stageRef.current;
    if (!stage) return;

    const report = () => {
      const rect = stage.getBoundingClientRect();
      // The Rust side works in physical pixels; getBoundingClientRect is in CSS pixels.
      const scale = window.devicePixelRatio || 1;
      // Hidden until there is a picture: the surface sits OVER the webview, so a visible
      // empty one paints black across the stage and hides "No media loaded".
      setVideoRect(
        Math.round(rect.left * scale),
        Math.round(rect.top * scale),
        Math.round(rect.width * scale),
        Math.round(rect.height * scale),
        hasMedia,
      ).catch(() => {
        // No surface on this platform/build — the engine already reports that on open.
      });
    };

    report();
    const observer = new ResizeObserver(report);
    observer.observe(stage);
    window.addEventListener("resize", report);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", report);
    };
  }, [accepted, hasMedia]);

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

  // Every transport failure is shown verbatim — the honesty invariant forbids a silent
  // failure or a black screen.
  const report = (error: unknown) => setPlaybackError(String(error));

  const chooseMedia = useCallback(async () => {
    setPlaybackError(null);
    try {
      const picked = await openFileDialog({ multiple: false, directory: false });
      if (typeof picked !== "string") return;
      await openMedia(picked);
    } catch (error) {
      report(error);
    }
  }, []);

  const relativeSeek = useCallback(
    (delta: number) => {
      setPlaybackError(null);
      seek(Math.max(0, playback.positionSecs + delta)).catch(report);
    },
    [playback.positionSecs],
  );

  // The window is borderless, so the title bar is the ONLY way to move, minimise or close
  // it — it has to be present on every screen, including the gate. Settings and About are
  // hidden until the agreement is accepted, since nothing behind the gate is usable yet.
  const chrome = (body: React.ReactNode, showActions: boolean) => (
    <I18nContext.Provider value={t}>
      <div className="relative flex h-full flex-col bg-havoc-bg text-havoc-text">
        <TitleBar
          title="Freally Player"
          showActions={showActions}
          onOpenSettings={() => setSettingsCategory("general")}
          onOpenAbout={() => setSettingsCategory("about")}
        />
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

  const control =
    "rounded-md border border-havoc-border px-2.5 py-1 text-xs text-havoc-muted hover:border-havoc-accent hover:text-havoc-text";

  // The native video surface is a real window sitting over the stage, so the shell stays
  // opaque throughout — the picture covers this area rather than showing through it.
  const showingVideo = playback.media !== null;

  return chrome(
    <>
      <main className="flex flex-1 items-center justify-center overflow-hidden">
        <section
          ref={stageRef}
          aria-label={t("stage-label")}
          className="flex h-full w-full flex-col items-center justify-center gap-2 px-6 text-center"
        >
          {/* Nothing is drawn over the picture: while media is open this region stays empty
              and transparent so the native surface shows through. */}
          {!showingVideo && (
            <p className="m-0 text-sm tracking-wide text-havoc-muted">{t("stage-empty")}</p>
          )}
          {playbackError && (
            <p
              role="alert"
              className="m-0 max-w-xl rounded-md bg-havoc-panel/90 px-3 py-2 text-xs text-red-400"
            >
              {playbackError}
            </p>
          )}
        </section>
      </main>

      <div className="flex items-center gap-2 border-t border-havoc-border bg-havoc-panel px-4 py-2">
        <button type="button" onClick={() => void chooseMedia()} className={control}>
          {t("transport-open")}
        </button>
        <button
          type="button"
          onClick={() => {
            setPlaybackError(null);
            play().catch(report);
          }}
          className={control}
        >
          {t("transport-play")}
        </button>
        <button
          type="button"
          onClick={() => {
            setPlaybackError(null);
            pause().catch(report);
          }}
          className={control}
        >
          {t("transport-pause")}
        </button>
        {/* A signed number with a unit is an LTR expression in every language, so these two
            labels are pinned LTR. Without it the leading "−" is a bidi-neutral character: in
            the Arabic shell it resolves to the paragraph direction and lands on the far side
            of the digits, rendering "−10 ث" as "ث 10−" — right in reading order, but read as
            "10 minus" by anyone reading the numerals. Every other locale is LTR anyway, so
            this changes nothing for them. */}
        <button type="button" onClick={() => relativeSeek(-10)} dir="ltr" className={control}>
          {t("transport-back")}
        </button>
        <button type="button" onClick={() => relativeSeek(10)} dir="ltr" className={control}>
          {t("transport-forward")}
        </button>
        {playback.media && (
          <span className="ms-2 truncate text-xs text-havoc-muted">
            {playback.media.title} · {t(`status-${playback.status}`)} ·{" "}
            {formatTime(playback.positionSecs)}
            {playback.media.durationSecs !== null &&
              ` / ${formatTime(playback.media.durationSecs)}`}
          </span>
        )}
      </div>

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
  );
}
