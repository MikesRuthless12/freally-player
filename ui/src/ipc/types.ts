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
  /** The subtitle-styling override for readability (off by default). */
  subtitleStyle: SubStyleOverride;
  /** Opt-in online subtitle fetch configuration. */
  openSubtitles: OpenSubtitlesSettings;
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

/** What a selectable track carries (`freally_player_core::TrackKind`). */
export type TrackKind = "audio" | "sub" | "video";

/** A selectable media track — audio, subtitle, or video (`freally_player_core::Track`). */
export interface Track {
  /** The backend's per-kind track id (its `aid`/`sid`). */
  id: number;
  kind: TrackKind;
  /** The declared language tag, when the file declares one. */
  lang: string | null;
  /** The track's own title, when the file names it. */
  title: string | null;
  /** Whether the demuxer marks this the default track for its kind. */
  default: boolean;
  /** Whether this subtitle was added from an external file rather than the container. */
  external: boolean;
  /** Whether the subtitle is image-based (PGS/VobSub) — no style/font override applies. */
  imageBased: boolean;
}

/** What is currently open (`freally_player_core::MediaInfo`). */
export interface MediaInfo {
  path: string;
  title: string;
  durationSecs: number | null;
  /** Chapter markers, empty until the demuxer has read them (and for media with none). */
  chapters: Chapter[];
  /** The audio/subtitle/video tracks the media exposes, plus any externally added subtitles. */
  tracks: Track[];
}

/** An A–B repeat range (`freally_player_core::AbLoop`). Either end may be unset. */
export interface AbLoop {
  a: number | null;
  b: number | null;
}

/** The subtitle transport (`freally_player_core::SubtitleState`). */
export interface SubtitleState {
  /** The primary subtitle track (`sid`), or `null` when subtitles are off. */
  id: number | null;
  /** The secondary subtitle track (`secondary-sid`) shown at once with the primary. */
  secondaryId: number | null;
  /** Whether the primary subtitle is displayed. */
  visible: boolean;
  /** Timing offset in seconds (`sub-delay`); positive shows subtitles later. */
  delaySecs: number;
  /** Vertical position on mpv's 0–150 scale (`sub-pos`); 100 is the default bottom line. */
  pos: number;
  /** Size multiplier (`sub-scale`); 1.0 is the author's / default size. */
  scale: number;
}

/** The subtitle-styling override (`freally_player_core::SubStyleOverride`). */
export interface SubStyleOverride {
  /** Whether the override is active. When off, the file's own ASS styling is respected. */
  enabled: boolean;
  /** Font family to force, or `null` for the default sans face. */
  font: string | null;
  /** Font size, or `null` for the default. */
  fontSize: number | null;
  /** Text colour as `#RRGGBB`, or `null` for the default. */
  color: string | null;
}

/** The default subtitle position (mirrors `SUB_POS_DEFAULT`). */
export const SUB_POS_DEFAULT = 100;
/** The subtitle-delay range in seconds, either way (mirrors `SUB_DELAY_MAX_SECS`). */
export const SUB_DELAY_MAX_SECS = 120;
/** The subtitle-scale range (mirrors `SUB_SCALE_MIN`/`MAX`). */
export const SUB_SCALE_MIN = 0.25;
export const SUB_SCALE_MAX = 4.0;

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
  /** The selected audio track (`aid`), or `null` when the backend has not chosen one. */
  audioId: number | null;
  /** The subtitle transport — selected tracks, timing, and placement. */
  subtitle: SubtitleState;
}

/** What loading an external/online subtitle produced (`commands::subtitles::LoadedSubtitleInfo`). */
export interface LoadedSubtitleInfo {
  /** The new subtitle track's id (`sid`). */
  trackId: number;
  /** The charset a text subtitle was transcoded from, or `null` when already UTF-8/image-based. */
  sourceEncoding: string | null;
  /** Whether the track is image-based (PGS/VobSub). */
  imageBased: boolean;
}

/** One OpenSubtitles search result (`freally_subtitles::Candidate`). */
export interface SubtitleCandidate {
  fileId: number;
  fileName: string;
  language: string | null;
  release: string | null;
  downloadCount: number;
}

/** The opt-in OpenSubtitles configuration (`settings::OpenSubtitlesSettings`). */
export interface OpenSubtitlesSettings {
  enabled: boolean;
  apiKey: string | null;
  username: string | null;
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
