import { open as openFileDialog, save as saveFileDialog } from "@tauri-apps/plugin-dialog";
import { useMemo } from "react";

import * as cmd from "../ipc/commands";
import type { LoadedSubtitleInfo } from "../ipc/types";

/** Subtitle file extensions offered by the "load subtitle" picker. */
const SUBTITLE_EXTENSIONS = ["srt", "ass", "ssa", "vtt", "sub", "idx", "sup", "txt"];

/**
 * The transport actions the player chrome drives, each wrapping an audited Rust command and
 * routing failures to one error sink. The honesty invariant lives here: nothing swallows an
 * error silently — every rejection becomes a message the user sees.
 */
export interface Transport {
  /** Open a path, or the native file picker when none is given. */
  open: (path?: string) => void;
  toggle: () => void;
  /** Seek to an absolute position; clamped at zero. */
  seekTo: (secs: number) => void;
  frameStep: (forward: boolean) => void;
  setVolume: (volume: number) => void;
  setMuted: (muted: boolean) => void;
  setSpeed: (speed: number) => void;
  setAbLoop: (a: number | null, b: number | null) => void;
  seekChapter: (index: number) => void;
  /** Save a snapshot of the current frame via the native save dialog. */
  snapshot: (withSubs: boolean) => void;
  // --- Subtitles & audio tracks ---
  setAudioTrack: (id: number | null) => void;
  setSubtitleTrack: (id: number | null) => void;
  setSecondarySubtitleTrack: (id: number | null) => void;
  setSubtitleVisible: (visible: boolean) => void;
  setSubtitleDelay: (secs: number) => void;
  setSubtitlePos: (pos: number) => void;
  setSubtitleScale: (scale: number) => void;
  /**
   * Pick and load an external subtitle file via the native picker. Resolves to what was loaded
   * (so the caller can show an honest "loaded as Windows-1251" note), or `null` when the picker
   * was cancelled or the load failed (the error is routed to the sink either way).
   */
  addSubtitleFile: () => Promise<LoadedSubtitleInfo | null>;
}

/**
 * Build the transport actions. `onError` must be stable (memoise it in the caller) so the
 * actions are only rebuilt when it truly changes.
 */
export function useTransport(onError: (message: string | null) => void): Transport {
  return useMemo<Transport>(() => {
    const fail = (error: unknown) => onError(String(error));
    // Clear any prior error the moment a new action starts; surface a rejection if it comes.
    const run = (action: Promise<unknown>) => {
      onError(null);
      action.catch(fail);
    };

    return {
      open: async (path) => {
        onError(null);
        try {
          const picked = path ?? (await openFileDialog({ multiple: false, directory: false }));
          if (typeof picked !== "string") return;
          await cmd.openMedia(picked);
        } catch (error) {
          fail(error);
        }
      },
      toggle: () => run(cmd.togglePlay()),
      seekTo: (secs) => run(cmd.seek(Math.max(0, secs))),
      frameStep: (forward) => run(cmd.frameStep(forward)),
      setVolume: (volume) => run(cmd.setVolume(volume)),
      setMuted: (muted) => run(cmd.setMuted(muted)),
      setSpeed: (speed) => run(cmd.setSpeed(speed)),
      setAbLoop: (a, b) => run(cmd.setAbLoop(a, b)),
      seekChapter: (index) => run(cmd.setChapter(index)),
      snapshot: async (withSubs) => {
        onError(null);
        try {
          const path = await saveFileDialog({
            defaultPath: "snapshot.png",
            filters: [{ name: "PNG image", extensions: ["png"] }],
          });
          if (typeof path !== "string") return;
          await cmd.captureFrame(path, withSubs);
        } catch (error) {
          fail(error);
        }
      },
      setAudioTrack: (id) => run(cmd.setAudioTrack(id)),
      setSubtitleTrack: (id) => run(cmd.setSubtitleTrack(id)),
      setSecondarySubtitleTrack: (id) => run(cmd.setSecondarySubtitleTrack(id)),
      setSubtitleVisible: (visible) => run(cmd.setSubtitleVisible(visible)),
      setSubtitleDelay: (secs) => run(cmd.setSubtitleDelay(secs)),
      setSubtitlePos: (pos) => run(cmd.setSubtitlePos(pos)),
      setSubtitleScale: (scale) => run(cmd.setSubtitleScale(scale)),
      addSubtitleFile: async () => {
        onError(null);
        try {
          const picked = await openFileDialog({
            multiple: false,
            directory: false,
            filters: [{ name: "Subtitles", extensions: SUBTITLE_EXTENSIONS }],
          });
          if (typeof picked !== "string") return null;
          return await cmd.addSubtitleFile(picked);
        } catch (error) {
          fail(error);
          return null;
        }
      },
    };
  }, [onError]);
}
