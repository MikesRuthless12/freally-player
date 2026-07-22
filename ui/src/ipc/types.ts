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

/** What is currently open (`freally_player_core::MediaInfo`). */
export interface MediaInfo {
  path: string;
  title: string;
  durationSecs: number | null;
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
}
