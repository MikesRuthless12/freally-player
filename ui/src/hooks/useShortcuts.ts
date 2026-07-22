import { useEffect, useRef } from "react";

import { SPEED_PRESETS } from "../components/ControlBar";
import { SPEED_MAX, SPEED_MIN, type PlaybackState } from "../ipc/types";
import type { Transport } from "../lib/transport";

/** Keyboard skip/volume steps. */
const SKIP_SHORT = 5;
const SKIP_LONG = 10;
const VOLUME_STEP = 5;

/**
 * The keyboard-first shortcut layer. Bound once to the window and reading the latest transport
 * state through refs, so the handler is stable while never acting on stale values.
 *
 * Shortcuts are ignored when focus is on an interactive element (a text field, a button, the
 * scrubber's own slider), so typing a filename or nudging the scrubber never also toggles
 * playback. That is the one rule that keeps a global key layer from fighting the widgets.
 */
export function useShortcuts({
  playback,
  transport,
  onToggleFullscreen,
  enabled,
}: {
  playback: PlaybackState;
  transport: Transport;
  onToggleFullscreen: () => void;
  /** Off behind the first-run gate, where there is nothing to control yet. */
  enabled: boolean;
}) {
  // The window handler is bound once but must always read the freshest values; refresh the ref
  // after each render rather than during it.
  const latest = useRef({ playback, transport, onToggleFullscreen });
  useEffect(() => {
    latest.current = { playback, transport, onToggleFullscreen };
  });

  useEffect(() => {
    if (!enabled) return;

    const onKey = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.ctrlKey || event.metaKey || event.altKey) return;
      if (isInteractive(event.target)) return;
      // An open modal (Settings, the bug reporter) owns the keyboard — its own container is
      // focused but is a plain div, so the interactive check above does not catch it. The
      // player's shortcuts must not act behind it.
      if (document.querySelector('[role="dialog"]')) return;

      const { playback, transport, onToggleFullscreen } = latest.current;
      const { positionSecs, volume, speed, media } = playback;
      const duration = media?.durationSecs ?? null;

      const seekBy = (delta: number) => transport.seekTo(positionSecs + delta);
      const setVolume = (value: number) => transport.setVolume(Math.min(100, Math.max(0, value)));

      switch (event.key) {
        case " ":
        case "k":
          transport.toggle();
          break;
        case "ArrowLeft":
          seekBy(-SKIP_SHORT);
          break;
        case "ArrowRight":
          seekBy(SKIP_SHORT);
          break;
        case "j":
          seekBy(-SKIP_LONG);
          break;
        case "l":
          seekBy(SKIP_LONG);
          break;
        case "ArrowUp":
          setVolume(volume + VOLUME_STEP);
          break;
        case "ArrowDown":
          setVolume(volume - VOLUME_STEP);
          break;
        case "m":
          transport.setMuted(!playback.muted);
          break;
        case "f":
          onToggleFullscreen();
          break;
        case ",":
          transport.frameStep(false);
          break;
        case ".":
          transport.frameStep(true);
          break;
        case "[":
          transport.setSpeed(stepSpeed(speed, -1));
          break;
        case "]":
          transport.setSpeed(stepSpeed(speed, 1));
          break;
        default:
          // Digits 0–9 jump to 0%–90% of the file.
          if (/^[0-9]$/.test(event.key) && duration !== null && duration > 0) {
            transport.seekTo((Number(event.key) / 10) * duration);
            break;
          }
          return;
      }
      event.preventDefault();
    };

    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [enabled]);
}

/** The neighbouring speed preset in `direction`, clamped to the supported range. */
function stepSpeed(current: number, direction: 1 | -1): number {
  if (direction > 0) {
    const next = SPEED_PRESETS.find((s) => s > current + 0.001);
    return next ?? SPEED_MAX;
  }
  const below = [...SPEED_PRESETS].reverse().find((s) => s < current - 0.001);
  return below ?? SPEED_MIN;
}

/** Is the event aimed at something that consumes keys itself? Then the global layer stands down. */
function isInteractive(target: EventTarget | null): boolean {
  const element = target as HTMLElement | null;
  if (!element) return false;
  if (element.isContentEditable) return true;
  const tag = element.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || tag === "BUTTON") return true;
  const role = element.getAttribute?.("role");
  return role === "slider" || role === "menu" || role === "menuitem";
}
