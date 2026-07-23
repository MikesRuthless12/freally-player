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
  LoadedSubtitleInfo,
  MediaInfo,
  PlaybackState,
  RecentWatch,
  SubStyleOverride,
  SubtitleCandidate,
  UserSettings,
} from "./types";

export const appInfo = (): Promise<AppInfo> => invoke<AppInfo>("app_info");

export const settingsGet = (): Promise<UserSettings> => invoke<UserSettings>("settings_get");

export const settingsSet = (settings: UserSettings): Promise<void> =>
  invoke<void>("settings_set", { settings });

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

/** Toggle play/pause — what Space and the OS play/pause key do. */
export const togglePlay = (): Promise<void> => invoke<void>("toggle_play");

export const seek = (positionSecs: number): Promise<void> => invoke<void>("seek", { positionSecs });

export const getState = (): Promise<PlaybackState> => invoke<PlaybackState>("get_state");

export const setVolume = (volume: number): Promise<void> => invoke<void>("set_volume", { volume });

export const setMuted = (muted: boolean): Promise<void> => invoke<void>("set_muted", { muted });

export const setSpeed = (speed: number): Promise<void> => invoke<void>("set_speed", { speed });

/** Step one frame forward (`true`) or back; mpv pauses as it does. */
export const frameStep = (forward: boolean): Promise<void> =>
  invoke<void>("frame_step", { forward });

/** Set or clear the A–B repeat range; `null` for an end clears it. */
export const setAbLoop = (a: number | null, b: number | null): Promise<void> =>
  invoke<void>("set_ab_loop", { a, b });

/** Jump to a chapter by its index in the open media's chapter list. */
export const setChapter = (index: number): Promise<void> => invoke<void>("set_chapter", { index });

/** Write the current frame to `path`; `withSubs` bakes in the subtitle overlay. */
export const captureFrame = (path: string, withSubs: boolean): Promise<void> =>
  invoke<void>("capture_frame", { path, withSubs });

/** The recently-watched items for the idle screen's Continue-Watching row, newest first. */
export const recentWatch = (limit: number): Promise<RecentWatch[]> =>
  invoke<RecentWatch[]>("recent_watch", { limit });

// --- Subtitles & audio tracks ----------------------------------------------
// Every mutating command also emits `player://state`; see `ipc/events.ts`.

/** Select the audio track (`aid`), or `null` to disable audio. */
export const setAudioTrack = (id: number | null): Promise<void> =>
  invoke<void>("set_audio_track", { id });

/** Select the primary subtitle track (`sid`), or `null` to turn subtitles off. */
export const setSubtitleTrack = (id: number | null): Promise<void> =>
  invoke<void>("set_subtitle_track", { id });

/** Select the secondary subtitle track (two at once), or `null` to turn it off. */
export const setSecondarySubtitleTrack = (id: number | null): Promise<void> =>
  invoke<void>("set_secondary_subtitle_track", { id });

/** Show or hide the primary subtitle without forgetting which track is selected. */
export const setSubtitleVisible = (visible: boolean): Promise<void> =>
  invoke<void>("set_subtitle_visible", { visible });

/** Set the subtitle timing offset in seconds (`sub-delay`). */
export const setSubtitleDelay = (secs: number): Promise<void> =>
  invoke<void>("set_subtitle_delay", { secs });

/** Set the subtitle vertical position on mpv's 0–150 scale (`sub-pos`). */
export const setSubtitlePos = (pos: number): Promise<void> =>
  invoke<void>("set_subtitle_pos", { pos });

/** Set the subtitle size multiplier (`sub-scale`). */
export const setSubtitleScale = (scale: number): Promise<void> =>
  invoke<void>("set_subtitle_scale", { scale });

/** Apply and persist the global subtitle style override (accessibility). */
export const setSubtitleStyleOverride = (style: SubStyleOverride): Promise<void> =>
  invoke<void>("set_subtitle_style_override", { style });

/**
 * Load an external subtitle file. The path comes from the UI's native picker — the web layer
 * never touches the filesystem itself. The file is treated as untrusted (bounded, transcoded to
 * UTF-8 when it is legacy-charset text) before the engine renders it.
 */
export const addSubtitleFile = (path: string): Promise<LoadedSubtitleInfo> =>
  invoke<LoadedSubtitleInfo>("add_subtitle_file", { path });

// --- Opt-in OpenSubtitles ---------------------------------------------------

/** Search OpenSubtitles by query and language codes. Only these identifiers leave the machine. */
export const openSubtitlesSearch = (
  query: string,
  languages: string[],
): Promise<SubtitleCandidate[]> =>
  invoke<SubtitleCandidate[]>("opensubtitles_search", { query, languages });

/** Sign in to OpenSubtitles for the session. The password is never stored. */
export const openSubtitlesLogin = (username: string, password: string): Promise<void> =>
  invoke<void>("opensubtitles_login", { username, password });

/** Download a chosen candidate and attach it as a subtitle track (needs a session login). */
export const openSubtitlesDownload = (fileId: number): Promise<LoadedSubtitleInfo> =>
  invoke<LoadedSubtitleInfo>("opensubtitles_download", { fileId });

/**
 * Report where the video stage is, in physical pixels relative to the window's client area.
 *
 * The native video surface is a sibling window placed *over* the webview, so it has to track
 * this rect exactly or it would cover the chrome.
 */
export const setVideoRect = (
  x: number,
  y: number,
  width: number,
  height: number,
  visible: boolean,
): Promise<void> => invoke<void>("set_video_rect", { x, y, width, height, visible });
