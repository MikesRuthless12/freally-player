import { useT } from "../i18n";
import { formatTime } from "../lib/time";
import type { PlaybackState } from "../ipc/types";
import type { Transport } from "../lib/transport";
import { AudioMenu } from "./AudioMenu";
import { Menu, MenuItem } from "./Menu";
import { Scrubber } from "./Scrubber";
import { SubtitleMenu } from "./SubtitleMenu";

/** The speed presets the menu offers, within the engine's 0.25×–4.0× range. */
export const SPEED_PRESETS = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 1.75, 2, 3, 4] as const;

/** How far ± skip and the volume keys move. */
const SKIP_SECS = 10;
const VOLUME_STEP = 5;

/**
 * The glassy transport bar beneath the picture: play/pause, frame-step, skip, a live clock,
 * volume, speed, A–B repeat, a chapters menu, snapshot and fullscreen — over the scrubber.
 *
 * It sits in its own strip rather than floating over the frame: on Windows the native video
 * surface is composited *above* the webview, so the web layer can only paint chrome in the
 * area the picture does not cover (see `crates/player/src/surface/windows.rs`). The strip
 * auto-hides during playback so the picture fills the window; `Player` owns that timing.
 */
export function ControlBar({
  playback,
  transport,
  fullscreen,
  onToggleFullscreen,
}: {
  playback: PlaybackState;
  transport: Transport;
  fullscreen: boolean;
  onToggleFullscreen: () => void;
}) {
  const t = useT();
  const { media, status, positionSecs, volume, muted, speed, bufferedSecs, abLoop } = playback;
  const duration = media?.durationSecs ?? null;
  const chapters = media?.chapters ?? [];
  const playing = status === "playing";

  const seekBy = (delta: number) => transport.seekTo(positionSecs + delta);

  // A–B repeat cycles: set A → set B → clear.
  const abLabel =
    abLoop.a === null
      ? t("transport-ab-set-a")
      : abLoop.b === null
        ? t("transport-ab-set-b")
        : t("transport-ab-clear");
  const cycleAb = () => {
    if (abLoop.a === null) transport.setAbLoop(positionSecs, null);
    else if (abLoop.b === null) {
      // B must come after A; a second click before A just moves A instead.
      if (positionSecs > abLoop.a) transport.setAbLoop(abLoop.a, positionSecs);
      else transport.setAbLoop(positionSecs, null);
    } else transport.setAbLoop(null, null);
  };
  const abActive = abLoop.a !== null;

  const iconBtn =
    "grid h-8 w-8 place-items-center rounded-md text-havoc-muted transition-colors hover:bg-havoc-surface hover:text-havoc-text disabled:cursor-not-allowed disabled:opacity-40";

  return (
    <div className="flex flex-col gap-1.5 border-t border-havoc-border bg-havoc-panel/85 px-3 pt-1.5 pb-2 backdrop-blur-md">
      <Scrubber
        positionSecs={positionSecs}
        durationSecs={duration}
        bufferedSecs={bufferedSecs}
        chapters={chapters}
        abLoop={abLoop}
        onSeek={transport.seekTo}
      />

      <div className="flex items-center gap-1">
        {/* Play / pause. */}
        <button
          type="button"
          onClick={transport.toggle}
          aria-label={playing ? t("transport-pause") : t("transport-play")}
          className={iconBtn}
        >
          {playing ? <PauseIcon /> : <PlayIcon />}
        </button>

        {/* Frame step — pauses as it moves. */}
        <button
          type="button"
          onClick={() => transport.frameStep(false)}
          aria-label={t("transport-frame-back")}
          className={iconBtn}
        >
          <FrameBackIcon />
        </button>
        <button
          type="button"
          onClick={() => transport.frameStep(true)}
          aria-label={t("transport-frame-forward")}
          className={iconBtn}
        >
          <FrameForwardIcon />
        </button>

        {/* Skip ±10s. The translated label is the visible text (and the accessible name);
            pinned LTR so the leading "−" stays on the digits' left in Arabic — otherwise the
            bidi-neutral sign resolves to the RTL paragraph and "−10 ث" draws as "ث 10−". */}
        <button
          type="button"
          onClick={() => seekBy(-SKIP_SECS)}
          dir="ltr"
          className={`${iconBtn} w-auto px-1.5 text-[11px]`}
        >
          {t("transport-back")}
        </button>
        <button
          type="button"
          onClick={() => seekBy(SKIP_SECS)}
          dir="ltr"
          className={`${iconBtn} w-auto px-1.5 text-[11px]`}
        >
          {t("transport-forward")}
        </button>

        {/* Clock. Pinned LTR: it is a numeric expression, not shell text. */}
        <span className="ms-1 text-[11px] tabular-nums text-havoc-muted" dir="ltr">
          {formatTime(positionSecs)}
          {duration !== null && ` / ${formatTime(duration)}`}
        </span>

        <div className="flex-1" />

        {/* Volume. */}
        <Volume
          muted={muted}
          volume={volume}
          onMute={() => transport.setMuted(!muted)}
          onVolume={(v) => transport.setVolume(v)}
          step={VOLUME_STEP}
        />

        {/* Speed. */}
        <Menu
          label={`${formatSpeed(speed)}×`}
          title={t("transport-speed")}
          className="text-[11px] tabular-nums"
        >
          {(close) =>
            SPEED_PRESETS.map((preset) => (
              <MenuItem
                key={preset}
                selected={Math.abs(preset - speed) < 0.001}
                onSelect={() => {
                  transport.setSpeed(preset);
                  close();
                }}
              >
                {formatSpeed(preset)}×
              </MenuItem>
            ))
          }
        </Menu>

        {/* A–B repeat. */}
        <button
          type="button"
          onClick={cycleAb}
          aria-label={abLabel}
          title={abLabel}
          aria-pressed={abActive}
          className={`${iconBtn} w-auto px-1.5 text-[11px] font-semibold ${
            abActive ? "text-havoc-accent-2" : ""
          }`}
        >
          A–B
        </button>

        {/* Chapters. */}
        <Menu
          icon={<ChaptersIcon />}
          title={t("transport-chapters")}
          disabled={chapters.length === 0}
          align="end"
        >
          {(close) =>
            chapters.map((chapter, i) => (
              <MenuItem
                key={`${i}-${chapter.startSecs}`}
                onSelect={() => {
                  transport.seekChapter(i);
                  close();
                }}
              >
                <span className="tabular-nums text-havoc-muted" dir="ltr">
                  {formatTime(chapter.startSecs)}
                </span>
                <span className="truncate">
                  {chapter.title ?? t("transport-chapter-n", { n: i + 1 })}
                </span>
              </MenuItem>
            ))
          }
        </Menu>

        {/* Audio tracks. */}
        <AudioMenu tracks={media?.tracks ?? []} audioId={playback.audioId} transport={transport} />

        {/* Subtitles: tracks, sync/position/scale, external load, online fetch. */}
        <SubtitleMenu
          tracks={media?.tracks ?? []}
          subtitle={playback.subtitle}
          transport={transport}
          mediaTitle={media?.title}
        />

        {/* Snapshot. */}
        <button
          type="button"
          onClick={() => transport.snapshot(true)}
          aria-label={t("transport-snapshot")}
          title={t("transport-snapshot")}
          className={iconBtn}
        >
          <CameraIcon />
        </button>

        {/* Fullscreen. */}
        <button
          type="button"
          onClick={onToggleFullscreen}
          aria-label={fullscreen ? t("transport-exit-fullscreen") : t("transport-fullscreen")}
          title={fullscreen ? t("transport-exit-fullscreen") : t("transport-fullscreen")}
          aria-pressed={fullscreen}
          className={iconBtn}
        >
          {fullscreen ? <ExitFullscreenIcon /> : <FullscreenIcon />}
        </button>
      </div>
    </div>
  );
}

/** One or two decimals as needed — `1×`, `1.5×`, `0.25×` — never `1.00×`. */
function formatSpeed(speed: number): string {
  return Number(speed.toFixed(2)).toString();
}

function Volume({
  muted,
  volume,
  onMute,
  onVolume,
  step,
}: {
  muted: boolean;
  volume: number;
  onMute: () => void;
  onVolume: (value: number) => void;
  step: number;
}) {
  const t = useT();
  const iconBtn =
    "grid h-8 w-8 place-items-center rounded-md text-havoc-muted transition-colors hover:bg-havoc-surface hover:text-havoc-text";
  return (
    <div className="flex items-center gap-1">
      <button
        type="button"
        onClick={onMute}
        aria-label={muted ? t("transport-unmute") : t("transport-mute")}
        title={muted ? t("transport-unmute") : t("transport-mute")}
        className={iconBtn}
      >
        {muted || volume === 0 ? <MuteIcon /> : <VolumeIcon />}
      </button>
      <input
        type="range"
        min={0}
        max={100}
        step={step}
        value={muted ? 0 : Math.round(volume)}
        onChange={(event) => onVolume(Number(event.target.value))}
        aria-label={t("transport-volume")}
        className="h-1 w-20 cursor-pointer accent-havoc-accent"
        dir="ltr"
      />
    </div>
  );
}

// --- Icons (currentColor, 16px viewbox) -------------------------------------

function PlayIcon() {
  return (
    <svg viewBox="0 0 16 16" className="h-4 w-4" aria-hidden="true">
      <path d="M4 3l9 5-9 5z" fill="currentColor" />
    </svg>
  );
}
function PauseIcon() {
  return (
    <svg viewBox="0 0 16 16" className="h-4 w-4" aria-hidden="true">
      <path d="M4 3h3v10H4zM9 3h3v10H9z" fill="currentColor" />
    </svg>
  );
}
function FrameBackIcon() {
  return (
    <svg viewBox="0 0 16 16" className="h-4 w-4" fill="currentColor" aria-hidden="true">
      <path d="M4 3h1.5v10H4zM12 3l-6 5 6 5z" />
    </svg>
  );
}
function FrameForwardIcon() {
  return (
    <svg viewBox="0 0 16 16" className="h-4 w-4" fill="currentColor" aria-hidden="true">
      <path d="M10.5 3H12v10h-1.5zM4 3l6 5-6 5z" />
    </svg>
  );
}
function VolumeIcon() {
  return (
    <svg
      viewBox="0 0 16 16"
      className="h-4 w-4"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.3"
      aria-hidden="true"
    >
      <path d="M3 6v4h2l3 2.5v-9L5 6z" fill="currentColor" stroke="none" />
      <path d="M10.5 5.5a3.5 3.5 0 010 5M12.5 3.5a6.5 6.5 0 010 9" strokeLinecap="round" />
    </svg>
  );
}
function MuteIcon() {
  return (
    <svg
      viewBox="0 0 16 16"
      className="h-4 w-4"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.3"
      aria-hidden="true"
    >
      <path d="M3 6v4h2l3 2.5v-9L5 6z" fill="currentColor" stroke="none" />
      <path d="M10.5 6.5l3 3M13.5 6.5l-3 3" strokeLinecap="round" />
    </svg>
  );
}
function ChaptersIcon() {
  return (
    <svg
      viewBox="0 0 16 16"
      className="h-4 w-4"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.3"
      aria-hidden="true"
    >
      <path d="M2.5 4h11M2.5 8h11M2.5 12h7" strokeLinecap="round" />
    </svg>
  );
}
function CameraIcon() {
  return (
    <svg
      viewBox="0 0 16 16"
      className="h-4 w-4"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.3"
      aria-hidden="true"
    >
      <rect x="2" y="4.5" width="12" height="8" rx="1.5" />
      <circle cx="8" cy="8.5" r="2.2" />
      <path d="M5.5 4.5l1-1.5h3l1 1.5" strokeLinejoin="round" />
    </svg>
  );
}
function FullscreenIcon() {
  return (
    <svg
      viewBox="0 0 16 16"
      className="h-4 w-4"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.4"
      aria-hidden="true"
    >
      <path
        d="M2.5 6V2.5H6M10 2.5h3.5V6M13.5 10v3.5H10M6 13.5H2.5V10"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
function ExitFullscreenIcon() {
  return (
    <svg
      viewBox="0 0 16 16"
      className="h-4 w-4"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.4"
      aria-hidden="true"
    >
      <path
        d="M6 2.5V6H2.5M13.5 6H10V2.5M10 13.5V10h3.5M2.5 10H6v3.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
