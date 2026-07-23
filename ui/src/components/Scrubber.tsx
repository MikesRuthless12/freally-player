import { useCallback, useRef, useState } from "react";

import { useT } from "../i18n";
import { formatTime } from "../lib/time";
import type { AbLoop, Chapter } from "../ipc/types";

/** How far the arrow keys nudge the scrubber, in seconds. */
const KEY_STEP_SECS = 5;

/**
 * The seek bar: a played fill over a buffered range, chapter ticks, the A–B loop region, and a
 * timecode that follows the cursor. Click or drag to seek anywhere; arrow keys nudge.
 *
 * **Pinned `dir="ltr"`.** A media timeline is a left-to-right temporal axis in every locale, so
 * the fill grows from the start edge and the pointer maths read from the left regardless of the
 * shell's direction — the same reason the ±10s buttons are pinned. Mirroring it in Arabic would
 * put "0:00" on the right and invert every drag.
 */
export function Scrubber({
  positionSecs,
  durationSecs,
  bufferedSecs,
  chapters,
  abLoop,
  onSeek,
}: {
  positionSecs: number;
  durationSecs: number | null;
  bufferedSecs: number;
  chapters: Chapter[];
  abLoop: AbLoop;
  onSeek: (secs: number) => void;
}) {
  const t = useT();
  const trackRef = useRef<HTMLDivElement>(null);
  // The timecode preview follows the cursor; null when not hovering.
  const [hover, setHover] = useState<{ x: number; secs: number } | null>(null);

  // A live duration is required to map a position to a fraction. Until the demuxer reports one
  // the bar renders as an inert track rather than pretending to be seekable.
  const seekable = durationSecs !== null && durationSecs > 0;
  const duration = durationSecs ?? 0;

  const fractionAt = useCallback((clientX: number): number => {
    const track = trackRef.current;
    if (!track) return 0;
    const rect = track.getBoundingClientRect();
    if (rect.width <= 0) return 0;
    return Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
  }, []);

  const seekToClientX = useCallback(
    (clientX: number) => {
      if (!seekable) return;
      onSeek(fractionAt(clientX) * duration);
    },
    [seekable, duration, fractionAt, onSeek],
  );

  const onPointerDown = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!seekable) return;
    // Capture so a drag that leaves the track keeps seeking until the button is released.
    event.currentTarget.setPointerCapture(event.pointerId);
    seekToClientX(event.clientX);
  };

  const onPointerMove = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!seekable) return;
    // Store the hover position as an offset WITHIN the track, computed here from the live rect,
    // so the render never has to read the ref back.
    const rect = event.currentTarget.getBoundingClientRect();
    const fraction =
      rect.width > 0 ? Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width)) : 0;
    setHover({ x: event.clientX - rect.left, secs: fraction * duration });
    // While the primary button is held, a move is a drag-seek.
    if (event.buttons & 1) seekToClientX(event.clientX);
  };

  const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (!seekable) return;
    let next: number | null = null;
    switch (event.key) {
      case "ArrowLeft":
        next = positionSecs - KEY_STEP_SECS;
        break;
      case "ArrowRight":
        next = positionSecs + KEY_STEP_SECS;
        break;
      case "Home":
        next = 0;
        break;
      case "End":
        next = duration;
        break;
      default:
        return;
    }
    // Handle it here and keep it from also reaching the global shortcut layer.
    event.preventDefault();
    onSeek(Math.min(duration, Math.max(0, next)));
  };

  const pct = (secs: number) =>
    seekable ? `${Math.min(100, Math.max(0, (secs / duration) * 100))}%` : "0%";

  // The A–B loop shading: from A to B when both are set, or a thin marker at A while the user
  // is still choosing B.
  const loopLeft = abLoop.a;
  const loopRight = abLoop.b;

  return (
    <div className="relative flex items-center" dir="ltr">
      <div
        ref={trackRef}
        role="slider"
        tabIndex={seekable ? 0 : -1}
        aria-label={t("scrubber-label")}
        aria-valuemin={0}
        aria-valuemax={seekable ? Math.floor(duration) : 0}
        aria-valuenow={Math.floor(positionSecs)}
        aria-valuetext={
          seekable
            ? `${formatTime(positionSecs)} / ${formatTime(duration)}`
            : formatTime(positionSecs)
        }
        aria-disabled={!seekable}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerLeave={() => setHover(null)}
        onKeyDown={onKeyDown}
        className={`group relative h-6 w-full ${seekable ? "cursor-pointer" : "cursor-default"} outline-none`}
      >
        {/* The rail. */}
        <div className="pointer-events-none absolute inset-x-0 top-1/2 h-1 -translate-y-1/2 rounded-full bg-havoc-border">
          {/* Buffered range. */}
          <div
            className="absolute inset-y-0 left-0 rounded-full bg-havoc-muted/50"
            style={{ width: pct(bufferedSecs) }}
          />
          {/* A–B loop region. */}
          {loopLeft !== null && (
            <div
              className="absolute inset-y-0 bg-havoc-accent-2/30"
              style={{
                left: pct(loopLeft),
                width: loopRight !== null ? `calc(${pct(loopRight)} - ${pct(loopLeft)})` : "2px",
              }}
            />
          )}
          {/* Played fill. */}
          <div
            className="absolute inset-y-0 left-0 rounded-full bg-gradient-to-r from-havoc-accent to-havoc-accent-2"
            style={{ width: pct(positionSecs) }}
          />
          {/* Chapter ticks. */}
          {seekable &&
            chapters.map((chapter, i) => (
              <span
                key={`${i}-${chapter.startSecs}`}
                className="absolute top-1/2 h-2 w-px -translate-y-1/2 bg-havoc-bg/80"
                style={{ left: pct(chapter.startSecs) }}
              />
            ))}
        </div>

        {/* Playhead knob, shown on hover/focus. */}
        {seekable && (
          <span
            className="pointer-events-none absolute top-1/2 h-3 w-3 -translate-x-1/2 -translate-y-1/2 rounded-full bg-havoc-text opacity-0 shadow transition-opacity group-hover:opacity-100 group-focus:opacity-100"
            style={{ left: pct(positionSecs) }}
          />
        )}
      </div>

      {/* Hover timecode, tracking the cursor. `hover.x` is already relative to the track. */}
      {seekable && hover && (
        <span
          className="pointer-events-none absolute bottom-full mb-1 -translate-x-1/2 rounded bg-havoc-bg/95 px-1.5 py-0.5 text-[10px] tabular-nums text-havoc-text shadow"
          style={{ left: hover.x }}
          dir="ltr"
        >
          {formatTime(hover.secs)}
        </span>
      )}
    </div>
  );
}
