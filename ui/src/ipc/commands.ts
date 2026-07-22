/**
 * The typed IPC surface. The web UI has no direct network or filesystem access — every I/O
 * path goes through one of these audited Rust commands.
 */
import { invoke } from "@tauri-apps/api/core";

import type {
  AppInfo,
  BugReportContext,
  BugReportTarget,
  EulaStatus,
  MediaInfo,
  PlaybackState,
  Theme,
} from "./types";

export const appInfo = (): Promise<AppInfo> => invoke<AppInfo>("app_info");

export const themeGet = (): Promise<Theme> => invoke<Theme>("theme_get");

export const themeSet = (theme: Theme): Promise<void> => invoke<void>("theme_set", { theme });

export const eulaStatus = (): Promise<EulaStatus> => invoke<EulaStatus>("eula_status");

export const eulaAccept = (): Promise<void> => invoke<void>("eula_accept");

export const bugReportContext = (): Promise<BugReportContext> =>
  invoke<BugReportContext>("bug_report_context");

/** Opens a pre-filled draft. Nothing is sent — the user still clicks send. */
export const bugReportSubmit = (
  target: BugReportTarget,
  description: string,
  includeCrash: boolean,
): Promise<void> => invoke<void>("bug_report_submit", { target, description, includeCrash });

export const bugReportClearCrash = (): Promise<void> => invoke<void>("bug_report_clear_crash");

// --- Playback transport ----------------------------------------------------
// Every mutating command also emits `player://state`; see `ipc/events.ts`.

export const openMedia = (path: string): Promise<MediaInfo> =>
  invoke<MediaInfo>("open_media", { path });

export const play = (): Promise<void> => invoke<void>("play");

export const pause = (): Promise<void> => invoke<void>("pause");

export const seek = (positionSecs: number): Promise<void> => invoke<void>("seek", { positionSecs });

export const getState = (): Promise<PlaybackState> => invoke<PlaybackState>("get_state");

/**
 * Report where the video stage is, in physical pixels relative to the window's client area.
 *
 * The native video surface is a sibling window placed *over* the webview, so it has to track
 * this rect exactly or it would cover the chrome.
 */
export const setVideoRect = (x: number, y: number, width: number, height: number): Promise<void> =>
  invoke<void>("set_video_rect", { x, y, width, height });
