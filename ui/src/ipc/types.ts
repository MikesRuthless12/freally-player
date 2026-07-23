/** Typed mirrors of the Rust command payloads (`src-tauri/src/`). */

/** `app_info` — identity for the version banner. */
export interface AppInfo {
  name: string;
  version: string;
}

/** The UI colour scheme (serde `rename_all = "camelCase"` on `settings::Theme`). */
export type Theme = "dark" | "light";

/** `settings_get` / `settings_set` — the settings the Settings modal owns. */
export interface UserSettings {
  theme: Theme;
  /** Minimising hides to the system tray instead of the taskbar. */
  minimizeToTray: boolean;
  /**
   * The chosen UI locale as a BCP-47 tag, or `null` when the user has never picked one — in
   * which case the first run detects it from the OS. Rust keeps this a plain `Option<String>`
   * and does not police the value: the catalogs are the source of truth for which locales
   * exist, and they live in the UI, which falls back to English for anything it does not ship.
   */
  language: string | null;
}

/** `eula_status` — the embedded agreement plus whether this version is accepted. */
export interface EulaStatus {
  version: string;
  text: string;
  accepted: boolean;
}

/** `bug_report_context` — the exact anonymous data a report would carry. */
export interface BugReportContext {
  appVersion: string;
  os: string;
  arch: string;
  diagnostics: string;
  /** The scrubbed crash text from the previous run, if the app crashed. */
  pendingCrash: string | null;
}

/** Where a bug report is submitted. Every target is a pre-filled draft the user sends. */
export type BugReportTarget = "github" | "gmail" | "email";

/** Transport status (`freally_player_core::Status`). */
export type PlaybackStatus = "idle" | "playing" | "paused";

/** A chapter marker within the open media (`freally_player_core::Chapter`). */
export interface Chapter {
  /** The chapter's title, or `null` when the file does not name it. */
  title: string | null;
  startSecs: number;
}

/** What is currently open (`freally_player_core::MediaInfo`). */
export interface MediaInfo {
  path: string;
  title: string;
  durationSecs: number | null;
  /** Chapter markers, empty until the demuxer has read them (and for media with none). */
  chapters: Chapter[];
}

/** An A–B repeat range (`freally_player_core::AbLoop`). Either end may be unset. */
export interface AbLoop {
  a: number | null;
  b: number | null;
}

/**
 * The transport snapshot the UI mirrors (`freally_player_core::PlaybackState`).
 *
 * This is the *only* thing playback sends across IPC — decoded video is drawn by a native
 * GPU surface composited under the webview and never crosses this boundary.
 */
export interface PlaybackState {
  status: PlaybackStatus;
  positionSecs: number;
  media: MediaInfo | null;
  /** Output volume on mpv's 0–100 scale. */
  volume: number;
  muted: boolean;
  /** Playback speed; 1.0 is normal. */
  speed: number;
  /** How far the media is buffered, in seconds — the scrubber's buffered bar. */
  bufferedSecs: number;
  abLoop: AbLoop;
}

/** The supported playback-speed range (mirrors `freally_player_core::SPEED_MIN`/`MAX`). */
export const SPEED_MIN = 0.25;
export const SPEED_MAX = 4.0;

/** A recently-watched item for the idle screen's Continue-Watching row (`freally_library::RecentWatch`). */
export interface RecentWatch {
  path: string;
  positionSecs: number;
  durationSecs: number | null;
}
