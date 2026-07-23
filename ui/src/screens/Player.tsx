import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useCallback, useEffect, useRef, useState } from "react";

import { ControlBar } from "../components/ControlBar";
import { useT } from "../i18n";
import { recentWatch, setVideoRect } from "../ipc/commands";
import type { PlaybackState, RecentWatch } from "../ipc/types";
import { useShortcuts } from "../hooks/useShortcuts";
import { formatTime } from "../lib/time";
import type { Transport } from "../lib/transport";

/** Hide the control bar this long after the last interaction, while playing. */
const AUTO_HIDE_MS = 3000;

/**
 * The player screen: the video stage plus the transport chrome beneath it.
 *
 * The stage is a deliberately empty region. On Windows the decoded picture is drawn by a
 * native GPU surface composited *over* this webview at the stage's rect, so the web layer only
 * paints chrome around it (an idle screen when nothing is open, the control bar below) and
 * reports the stage geometry to Rust through `set_video_rect`. Anything shown while a file is
 * open — the error banner, the controls — lives outside the stage rect, or the picture would
 * cover it.
 */
export function Player({
  playback,
  error,
  transport,
  fullscreen,
  onToggleFullscreen,
}: {
  playback: PlaybackState;
  error: string | null;
  transport: Transport;
  fullscreen: boolean;
  onToggleFullscreen: () => void;
}) {
  const t = useT();
  const stageRef = useRef<HTMLElement | null>(null);
  const hasMedia = playback.media !== null;
  const playing = playback.status === "playing";

  // Shortcuts only when a file is open — on the idle screen there is no transport to drive, and
  // firing seek/pause there would only raise "no media is open" errors.
  useShortcuts({ playback, transport, onToggleFullscreen, enabled: hasMedia });

  // Keep the native video surface aligned with the stage element. The surface is a sibling
  // window drawn OVER the webview, so it must match this rect exactly or it would cover the
  // chrome. It stays hidden until there is a picture, so an empty black surface never paints
  // over the idle screen.
  useEffect(() => {
    const stage = stageRef.current;
    if (!stage) return;

    const report = () => {
      const rect = stage.getBoundingClientRect();
      const scale = window.devicePixelRatio || 1;
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
  }, [hasMedia]);

  // Open a file dropped anywhere on the window — the drop zone is the whole player.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "drop" && event.payload.paths.length > 0) {
          transport.open(event.payload.paths[0]);
        }
      })
      .then((off) => {
        unlisten = off;
      })
      .catch(() => {
        // No webview drag-drop in this environment (e.g. the test harness).
      });
    return () => unlisten?.();
  }, [transport]);

  // --- Auto-hiding controls -------------------------------------------------
  const [controlsVisible, setControlsVisible] = useState(true);
  const hideTimer = useRef<number | null>(null);
  const hoveringControls = useRef(false);
  // Mirrors `playing` for the timeout to read; refreshed after render, not during it.
  const playingRef = useRef(playing);
  useEffect(() => {
    playingRef.current = playing;
  });

  const scheduleHide = useCallback(() => {
    if (hideTimer.current) window.clearTimeout(hideTimer.current);
    hideTimer.current = window.setTimeout(() => {
      // Only a playing file hides its controls; a paused or idle one keeps them up.
      if (playingRef.current && !hoveringControls.current) setControlsVisible(false);
    }, AUTO_HIDE_MS);
  }, []);

  const revealControls = useCallback(() => {
    setControlsVisible(true);
    scheduleHide();
  }, [scheduleHide]);

  // A paused or idle file always shows its controls; only a playing one lets them fade. Derived
  // rather than stored, so pausing never has to push state back through an effect.
  const showControls = !playing || controlsVisible;

  // Any movement or keypress reveals the controls and restarts the hide timer. The timer only
  // starts once the pointer actually moves — controls stay up on a freshly opened file until
  // the viewer touches the mouse, so they never vanish out from under a still cursor.
  useEffect(() => {
    window.addEventListener("pointermove", revealControls);
    window.addEventListener("keydown", revealControls);
    return () => {
      window.removeEventListener("pointermove", revealControls);
      window.removeEventListener("keydown", revealControls);
      if (hideTimer.current) window.clearTimeout(hideTimer.current);
    };
  }, [revealControls]);

  const cursorHidden = hasMedia && !showControls;

  return (
    <div
      className={`relative flex flex-1 flex-col overflow-hidden ${cursorHidden ? "cursor-none" : ""}`}
    >
      <main className="flex flex-1 items-center justify-center overflow-hidden">
        <section
          ref={stageRef}
          aria-label={t("stage-label")}
          className="flex h-full w-full items-center justify-center overflow-hidden px-6 text-center"
        >
          {/* While a file is open this stays empty for the native surface to show through. */}
          {!hasMedia && <IdleScreen transport={transport} />}
        </section>
      </main>

      {/* The error banner lives outside the stage rect, so the picture never covers it. */}
      {error && (
        <div className="border-t border-havoc-border bg-havoc-panel px-4 py-2">
          <p role="alert" className="m-0 text-xs text-red-400">
            {error}
          </p>
        </div>
      )}

      {/* The transport strip collapses to nothing when hidden, so the picture grows to fill. */}
      {hasMedia && (
        <div
          className="grid transition-[grid-template-rows] duration-200 ease-out"
          style={{ gridTemplateRows: showControls ? "1fr" : "0fr" }}
          onPointerEnter={() => {
            hoveringControls.current = true;
          }}
          onPointerLeave={() => {
            hoveringControls.current = false;
          }}
        >
          <div
            className={`overflow-hidden transition-opacity duration-200 ${
              showControls ? "opacity-100" : "opacity-0"
            }`}
          >
            <ControlBar
              playback={playback}
              transport={transport}
              fullscreen={fullscreen}
              onToggleFullscreen={onToggleFullscreen}
            />
          </div>
        </div>
      )}
    </div>
  );
}

/** The last path component without its extension — a display title for a resume card. */
function fileStem(path: string): string {
  const name = path.split(/[/\\]/).pop() ?? path;
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(0, dot) : name;
}

/**
 * The idle screen: a drop zone with an Open button, and a Continue-Watching row of the files
 * the viewer last left partway through.
 */
function IdleScreen({ transport }: { transport: Transport }) {
  const t = useT();
  const [recent, setRecent] = useState<RecentWatch[]>([]);

  useEffect(() => {
    let cancelled = false;
    recentWatch(12).then(
      (items) => !cancelled && setRecent(items),
      () => {},
    );
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="flex w-full max-w-3xl flex-col items-center gap-8">
      <div className="flex flex-col items-center gap-3 rounded-2xl border border-dashed border-havoc-border px-12 py-10">
        <FilmIcon />
        <p className="m-0 text-sm font-semibold text-havoc-text">{t("idle-title")}</p>
        <p className="m-0 text-xs text-havoc-muted">{t("idle-drop-hint")}</p>
        <button
          type="button"
          onClick={() => transport.open()}
          className="mt-1 rounded-md border border-havoc-accent bg-havoc-accent/15 px-4 py-1.5 text-xs font-semibold text-havoc-text hover:bg-havoc-accent/25"
        >
          {t("transport-open")}
        </button>
      </div>

      {recent.length > 0 && (
        <section className="w-full" aria-label={t("idle-continue")}>
          <h2 className="mb-2 text-start text-xs font-semibold text-havoc-muted">
            {t("idle-continue")}
          </h2>
          <div className="flex gap-2 overflow-x-auto pb-1">
            {recent.map((item) => (
              <ResumeCard key={item.path} item={item} onOpen={() => transport.open(item.path)} />
            ))}
          </div>
        </section>
      )}
    </div>
  );
}

function ResumeCard({ item, onOpen }: { item: RecentWatch; onOpen: () => void }) {
  const fraction =
    item.durationSecs && item.durationSecs > 0
      ? Math.min(1, item.positionSecs / item.durationSecs)
      : 0;
  return (
    <button
      type="button"
      onClick={onOpen}
      title={item.path}
      className="flex w-40 shrink-0 flex-col gap-1.5 rounded-lg border border-havoc-border bg-havoc-surface p-2 text-start hover:border-havoc-accent"
    >
      <span className="truncate text-xs font-medium text-havoc-text">{fileStem(item.path)}</span>
      <span className="h-1 w-full overflow-hidden rounded-full bg-havoc-border" dir="ltr">
        <span
          className="block h-full rounded-full bg-gradient-to-r from-havoc-accent to-havoc-accent-2"
          style={{ width: `${fraction * 100}%` }}
        />
      </span>
      <span className="text-[10px] tabular-nums text-havoc-muted" dir="ltr">
        {formatTime(item.positionSecs)}
        {item.durationSecs !== null && ` / ${formatTime(item.durationSecs)}`}
      </span>
    </button>
  );
}

function FilmIcon() {
  return (
    <svg
      viewBox="0 0 24 24"
      className="h-8 w-8 text-havoc-muted"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.4"
      aria-hidden="true"
    >
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <path d="M7 4v16M17 4v16M3 9h4M3 15h4M17 9h4M17 15h4" strokeLinecap="round" />
    </svg>
  );
}
