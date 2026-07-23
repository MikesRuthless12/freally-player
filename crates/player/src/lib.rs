//! `freally-player-core` — owned playback orchestration over the libmpv/ffmpeg engine.
//!
//! This crate owns the **`Engine` boundary**: the transport types the rest of the app speaks
//! and the trait a decode/render backend must implement. The backend itself is non-owned —
//! libmpv via its render API is the primary one (P0.3), with ffmpeg for coverage — and is
//! deliberately replaceable.
//!
//! Nothing here ever carries decoded pixels. The engine draws into a native GPU surface
//! composited *under* the webview; this type only describes the transport, and that is the
//! only thing that crosses IPC.
//!
//! Unlike the other owned crates this one cannot `forbid(unsafe_code)` — the audited engine
//! FFI module will be the only `unsafe` in the whole app. Until it exists, `unsafe` is denied
//! crate-wide; the FFI module will opt itself out explicitly.

#![deny(unsafe_code)]

use std::fmt;

use serde::{Deserialize, Serialize};

#[cfg(feature = "engine-libmpv")]
mod mpv;

#[cfg(feature = "engine-libmpv")]
pub mod surface;

#[cfg(feature = "engine-libmpv")]
pub use mpv::MpvEngine;

/// Transport status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    /// Nothing is open.
    #[default]
    Idle,
    Playing,
    Paused,
}

/// A chapter marker within the open media.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chapter {
    /// The chapter's title, when the file names it.
    pub title: Option<String>,
    /// Where the chapter starts, in seconds.
    pub start_secs: f64,
}

/// What a selectable track carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrackKind {
    Audio,
    /// A subtitle track, embedded or added from an external file.
    Sub,
    /// A video track. Enumerated for completeness; the transport menus only offer audio and
    /// subtitle switching.
    Video,
}

/// A selectable media track the backend reports — an audio, subtitle, or video stream.
///
/// The `id` is the backend's per-kind selector (mpv's `aid`/`sid`): switching a track means
/// asking the engine to select this id for its kind. Nothing here decodes; it only names what
/// the demuxer (and any added external subtitle) makes available so the UI can offer a choice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    /// The backend's per-kind track id — its `aid` for audio, `sid` for subtitles.
    pub id: i64,
    pub kind: TrackKind,
    /// The language tag the file declares for the track, when it declares one.
    pub lang: Option<String>,
    /// The track's own title, when the file names it.
    pub title: Option<String>,
    /// Whether the demuxer marks this the default track for its kind.
    pub default: bool,
    /// Whether this subtitle was added from an external file rather than the media container.
    pub external: bool,
    /// Whether the track is image-based (PGS/VobSub) rather than text. Style and font overrides
    /// only apply to text subtitles, so the UI can say so honestly instead of offering controls
    /// that do nothing.
    pub image_based: bool,
}

/// What is currently open.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    /// The path or URL that was opened.
    pub path: String,
    /// A display name — the file stem until a backend reports real metadata.
    pub title: String,
    /// Total duration, once the backend knows it.
    pub duration_secs: Option<f64>,
    /// Chapter markers, empty until the demuxer has read them (and for files with none).
    #[serde(default)]
    pub chapters: Vec<Chapter>,
    /// The audio/subtitle/video tracks the media exposes, plus any externally added subtitles.
    /// Empty until the demuxer has read the header (and grows when an external subtitle loads).
    #[serde(default)]
    pub tracks: Vec<Track>,
}

/// An A–B repeat range. Either end may be unset while the user is still marking it.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbLoop {
    pub a: Option<f64>,
    pub b: Option<f64>,
}

/// The subtitle transport: which tracks are showing and how they are timed and placed.
///
/// The rendering itself is the engine's (libass into the video surface) — none of this crosses
/// IPC as pixels. These values only mirror what the engine is doing so the menu can reflect and
/// adjust it. A second, independent track ([`secondary_id`]) drives the two-subtitles-at-once
/// case (e.g. language learning); it sits at the top of the frame by default.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleState {
    /// The primary subtitle track (mpv `sid`), or `None` when subtitles are off.
    pub id: Option<i64>,
    /// The secondary subtitle track (mpv `secondary-sid`), shown at once with the primary.
    pub secondary_id: Option<i64>,
    /// Whether the primary subtitle is being displayed (mpv `sub-visibility`).
    pub visible: bool,
    /// Timing offset in seconds (mpv `sub-delay`); positive shows subtitles later.
    pub delay_secs: f64,
    /// Vertical position on mpv's 0–150 scale (`sub-pos`); 100 is the default bottom line.
    pub pos: i64,
    /// Size multiplier (mpv `sub-scale`); 1.0 is the author's / default size.
    pub scale: f64,
}

/// Subtitles at rest: primary auto-selected by the backend, none forced off, default timing.
impl Default for SubtitleState {
    fn default() -> Self {
        Self {
            id: None,
            secondary_id: None,
            visible: true,
            delay_secs: 0.0,
            pos: SUB_POS_DEFAULT,
            scale: 1.0,
        }
    }
}

/// A user override of subtitle styling — forcing a font, size, or colour over what the file's
/// ASS styling asks for. Off by default: it exists for readability/accessibility, and turning
/// it on overrides the author's intended styling, which the UI states plainly.
///
/// It applies to text subtitles only; image-based tracks (PGS/VobSub) carry their own bitmaps
/// and ignore it. All fields default to "no override", which is why `Default` is derived.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubStyleOverride {
    /// Whether the override is active. When off, the file's own ASS styling is respected.
    pub enabled: bool,
    /// Font family to force, or `None` to keep the default sans face.
    pub font: Option<String>,
    /// Font size (mpv `sub-font-size`), or `None` for the default.
    pub font_size: Option<f64>,
    /// Text colour as `#RRGGBB`, or `None` for the default.
    pub color: Option<String>,
}

/// The default subtitle position on mpv's 0–150 scale — the bottom line.
pub const SUB_POS_DEFAULT: i64 = 100;
/// The subtitle-timing offset range the transport exposes, in seconds either way.
pub const SUB_DELAY_MAX_SECS: f64 = 120.0;
/// The subtitle size-multiplier range (`sub-scale`).
pub const SUB_SCALE_MIN: f64 = 0.25;
pub const SUB_SCALE_MAX: f64 = 4.0;

/// Clamp a subtitle position onto mpv's 0–150 scale.
pub fn clamp_sub_pos(pos: i64) -> i64 {
    pos.clamp(0, 150)
}

/// Clamp a subtitle delay to a sane range, treating a non-finite request as no delay.
pub fn clamp_sub_delay(secs: f64) -> f64 {
    if secs.is_finite() {
        secs.clamp(-SUB_DELAY_MAX_SECS, SUB_DELAY_MAX_SECS)
    } else {
        0.0
    }
}

/// Clamp a subtitle scale into the supported range, treating a non-finite request as 1.0.
pub fn clamp_sub_scale(scale: f64) -> f64 {
    if scale.is_finite() {
        scale.clamp(SUB_SCALE_MIN, SUB_SCALE_MAX)
    } else {
        1.0
    }
}

/// The transport snapshot the UI mirrors. **No pixels travel this path.**
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackState {
    pub status: Status,
    pub position_secs: f64,
    pub media: Option<MediaInfo>,
    /// Output volume on mpv's 0–100 scale.
    pub volume: f64,
    pub muted: bool,
    /// Playback speed; 1.0 is normal, clamped to [`SPEED_MIN`]..=[`SPEED_MAX`].
    pub speed: f64,
    /// How far the media is buffered, in seconds — the scrubber's buffered bar. For a local
    /// file this reaches the duration quickly; for a stream it trails the download.
    pub buffered_secs: f64,
    pub ab_loop: AbLoop,
    /// The selected audio track (mpv `aid`), or `None` when the backend has not chosen one.
    pub audio_id: Option<i64>,
    /// The subtitle transport — selected tracks, timing, and placement.
    pub subtitle: SubtitleState,
}

/// A player that has nothing open sits at these transport defaults — full volume, normal
/// speed — so the UI never shows a muted-looking 0% or a stalled 0× before a file loads.
impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            status: Status::Idle,
            position_secs: 0.0,
            media: None,
            volume: 100.0,
            muted: false,
            speed: 1.0,
            buffered_secs: 0.0,
            ab_loop: AbLoop::default(),
            audio_id: None,
            subtitle: SubtitleState::default(),
        }
    }
}

/// The playback-speed range the transport exposes (0.25×–4.0×), clamped in the engine so a
/// bad value from the UI can never reach the backend.
pub const SPEED_MIN: f64 = 0.25;
pub const SPEED_MAX: f64 = 4.0;

/// Clamp a requested playback speed into the supported range, treating a non-finite request
/// as "normal speed" rather than letting it reach the backend.
pub fn clamp_speed(speed: f64) -> f64 {
    if speed.is_finite() {
        speed.clamp(SPEED_MIN, SPEED_MAX)
    } else {
        1.0
    }
}

/// Clamp a requested volume onto mpv's 0–100 scale, treating a non-finite request as full
/// volume rather than passing it through.
pub fn clamp_volume(volume: f64) -> f64 {
    if volume.is_finite() {
        volume.clamp(0.0, 100.0)
    } else {
        100.0
    }
}

/// Why an engine operation failed. Every variant is reported to the user verbatim — the
/// honesty invariant forbids a silent failure or a black screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// This build has no decode/render backend compiled in.
    NoBackend,
    /// A transport command arrived with nothing open.
    NothingOpen,
    /// The requested seek target is not a usable time.
    InvalidSeek,
    /// The backend refused, with its own reason.
    Backend(String),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoBackend => write!(
                f,
                "this build has no playback engine — it was built without the libmpv backend"
            ),
            Self::NothingOpen => write!(f, "no media is open"),
            Self::InvalidSeek => write!(f, "seek target is not a valid position"),
            Self::Backend(reason) => write!(f, "{reason}"),
        }
    }
}

impl std::error::Error for EngineError {}

/// A handle to the OS window the native video surface is hosted inside.
///
/// Deliberately a plain integer so the Tauri layer can pass its window handle down without
/// this crate depending on Tauri, and so nothing above the engine boundary touches a raw
/// pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostWindow {
    /// A Win32 `HWND`.
    Win32(isize),
}

/// How to open a file: where to start it and which tracks to select. Threaded into the
/// backend's open call so resume-from-position and last-used tracks are applied *atomically*
/// with the load, rather than as a seek afterward that could race the file becoming ready.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OpenOptions {
    /// Start position in seconds — the resume point, or `None` to start at the beginning.
    pub start_secs: Option<f64>,
    /// Audio track to select (mpv `aid`), or `None` to let the backend choose.
    pub audio_id: Option<i64>,
    /// Subtitle track to select (mpv `sid`).
    pub sub_id: Option<i64>,
}

/// A decode/render backend. Implementors drive a native GPU surface; the orchestration above
/// them only ever moves transport state.
pub trait Engine: Send {
    /// Open `path` (a local file or a URL) and describe what was opened, applying `options`
    /// (resume position + track selection) as part of the load.
    fn open(&mut self, path: &str, options: &OpenOptions) -> Result<MediaInfo, EngineError>;
    fn play(&mut self) -> Result<(), EngineError>;
    fn pause(&mut self) -> Result<(), EngineError>;
    /// Seek to an absolute position in seconds.
    fn seek(&mut self, position_secs: f64) -> Result<(), EngineError>;
    /// The current transport snapshot.
    fn state(&self) -> PlaybackState;

    // --- Transport extras (Phase 1). Each has a default that refuses with `NothingOpen`, so
    // an engine that decodes nothing (the null engine) inherits an honest refusal and a real
    // backend overrides with actual behaviour. The UI only reaches these once media is open,
    // which the null engine never allows. ---

    /// Set output volume on mpv's 0–100 scale.
    fn set_volume(&mut self, volume: f64) -> Result<(), EngineError> {
        let _ = volume;
        Err(EngineError::NothingOpen)
    }
    fn set_muted(&mut self, muted: bool) -> Result<(), EngineError> {
        let _ = muted;
        Err(EngineError::NothingOpen)
    }
    /// Set playback speed; the caller clamps to [`SPEED_MIN`]..=[`SPEED_MAX`].
    fn set_speed(&mut self, speed: f64) -> Result<(), EngineError> {
        let _ = speed;
        Err(EngineError::NothingOpen)
    }
    /// Step one frame forward (`forward`) or back, pausing as it does.
    fn frame_step(&mut self, forward: bool) -> Result<(), EngineError> {
        let _ = forward;
        Err(EngineError::NothingOpen)
    }
    /// Set or clear the A–B repeat range. `None` for an end clears it.
    fn set_ab_loop(&mut self, a: Option<f64>, b: Option<f64>) -> Result<(), EngineError> {
        let _ = (a, b);
        Err(EngineError::NothingOpen)
    }
    /// Jump to the chapter at `index`.
    fn set_chapter(&mut self, index: usize) -> Result<(), EngineError> {
        let _ = index;
        Err(EngineError::NothingOpen)
    }
    /// The currently selected `(audio_id, sub_id)`, for persisting last-used tracks.
    fn current_tracks(&self) -> (Option<i64>, Option<i64>) {
        (None, None)
    }
    /// Write the current frame to `path`. `with_subs` includes the subtitle overlay.
    fn capture_frame(&mut self, path: &str, with_subs: bool) -> Result<(), EngineError> {
        let _ = (path, with_subs);
        Err(EngineError::NothingOpen)
    }

    // --- Tracks & subtitles (Phase 2). Same pattern as the Phase 1 extras: a default that
    // refuses with `NothingOpen`, overridden by a real backend. The rendering stays the
    // engine's (libass into the surface) — these only select and adjust it. ---

    /// The audio/subtitle/video tracks the backend currently knows about, including any
    /// externally added subtitles. Empty until a header is parsed.
    fn tracks(&self) -> Vec<Track> {
        Vec::new()
    }
    /// Select the audio track by id (mpv `aid`), or `None` to disable audio.
    fn set_audio_track(&mut self, id: Option<i64>) -> Result<(), EngineError> {
        let _ = id;
        Err(EngineError::NothingOpen)
    }
    /// Select the primary subtitle track (mpv `sid`), or `None` to turn subtitles off.
    fn set_sub_track(&mut self, id: Option<i64>) -> Result<(), EngineError> {
        let _ = id;
        Err(EngineError::NothingOpen)
    }
    /// Select the secondary subtitle track (mpv `secondary-sid`) shown at once with the
    /// primary, or `None` to turn the second track off.
    fn set_secondary_sub_track(&mut self, id: Option<i64>) -> Result<(), EngineError> {
        let _ = id;
        Err(EngineError::NothingOpen)
    }
    /// Add an external subtitle file already decoded to a safe UTF-8 form by the owned
    /// subtitle pipeline, returning the new track's id. `select` makes it the primary track.
    fn add_sub_file(&mut self, path: &str, select: bool) -> Result<i64, EngineError> {
        let _ = (path, select);
        Err(EngineError::NothingOpen)
    }
    /// Show or hide the primary subtitle without forgetting which track is selected.
    fn set_sub_visible(&mut self, visible: bool) -> Result<(), EngineError> {
        let _ = visible;
        Err(EngineError::NothingOpen)
    }
    /// Set the subtitle timing offset in seconds (mpv `sub-delay`); the caller clamps it.
    fn set_sub_delay(&mut self, secs: f64) -> Result<(), EngineError> {
        let _ = secs;
        Err(EngineError::NothingOpen)
    }
    /// Set the primary subtitle's vertical position on mpv's 0–150 scale (`sub-pos`).
    fn set_sub_pos(&mut self, pos: i64) -> Result<(), EngineError> {
        let _ = pos;
        Err(EngineError::NothingOpen)
    }
    /// Set the subtitle size multiplier (mpv `sub-scale`); the caller clamps it.
    fn set_sub_scale(&mut self, scale: f64) -> Result<(), EngineError> {
        let _ = scale;
        Err(EngineError::NothingOpen)
    }
    /// Apply (or clear) a user override of subtitle styling — forcing a font/size/colour over
    /// the file's ASS styling for readability. Ignored by image-based tracks.
    fn set_sub_style_override(&mut self, style: &SubStyleOverride) -> Result<(), EngineError> {
        let _ = style;
        Err(EngineError::NothingOpen)
    }

    /// Create the native video surface inside `host`, sized in physical pixels.
    ///
    /// Called once the window exists. A backend with no video output reports why rather than
    /// leaving the user with a silent black stage.
    fn attach_surface(
        &mut self,
        host: HostWindow,
        width: u32,
        height: u32,
    ) -> Result<(), EngineError>;

    /// Place the video surface at the stage rect, in physical pixels relative to the host
    /// window's client area. A no-op when there is no surface.
    fn set_surface_rect(&self, x: i32, y: i32, width: u32, height: u32, visible: bool) {
        let _ = (x, y, width, height, visible);
    }
}

/// The engine used when no usable decode/render backend exists.
///
/// It refuses every operation rather than pretending to play something. That is deliberate: a
/// stub that advanced a clock without decoding would satisfy the UI and violate the honesty
/// invariant.
///
/// Two distinct situations, and the difference matters to the user: the build genuinely has
/// no backend compiled in ([`NullEngine::default`]), or a backend *is* compiled in but failed
/// to start ([`NullEngine::unavailable`]) — in which case the real reason is reported instead
/// of the misleading "built without libmpv".
#[derive(Debug, Default)]
pub struct NullEngine {
    reason: Option<String>,
}

impl NullEngine {
    /// A backend was compiled in but could not start; `reason` is shown to the user.
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            reason: Some(reason.into()),
        }
    }

    fn refusal(&self) -> EngineError {
        match &self.reason {
            Some(reason) => EngineError::Backend(reason.clone()),
            None => EngineError::NoBackend,
        }
    }
}

impl Engine for NullEngine {
    fn open(&mut self, _path: &str, _options: &OpenOptions) -> Result<MediaInfo, EngineError> {
        Err(self.refusal())
    }

    fn play(&mut self) -> Result<(), EngineError> {
        Err(self.refusal())
    }

    fn pause(&mut self) -> Result<(), EngineError> {
        Err(self.refusal())
    }

    fn seek(&mut self, _position_secs: f64) -> Result<(), EngineError> {
        Err(self.refusal())
    }

    fn state(&self) -> PlaybackState {
        PlaybackState::default()
    }

    fn attach_surface(
        &mut self,
        _host: HostWindow,
        _width: u32,
        _height: u32,
    ) -> Result<(), EngineError> {
        Err(self.refusal())
    }
}

/// Is `position_secs` a time an engine can actually seek to?
pub fn is_seekable_position(position_secs: f64) -> bool {
    position_secs.is_finite() && position_secs >= 0.0
}

/// A display title for `path` — the file stem, falling back to the whole string.
pub fn title_for(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or(path)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_null_engine_refuses_honestly_instead_of_pretending() {
        let mut engine = NullEngine::default();
        assert_eq!(
            engine.open("clip.mkv", &OpenOptions::default()),
            Err(EngineError::NoBackend)
        );
        assert_eq!(engine.play(), Err(EngineError::NoBackend));
        assert_eq!(engine.pause(), Err(EngineError::NoBackend));
        assert_eq!(engine.seek(1.0), Err(EngineError::NoBackend));
        assert_eq!(engine.state(), PlaybackState::default());
    }

    /// A backend that failed to start must report *why*, not claim it was never built in.
    #[test]
    fn an_unavailable_backend_reports_its_real_reason() {
        let mut engine = NullEngine::unavailable("libmpv: no audio device");
        assert_eq!(
            engine.open("clip.mkv", &OpenOptions::default()),
            Err(EngineError::Backend("libmpv: no audio device".to_owned()))
        );
        assert!(engine
            .play()
            .unwrap_err()
            .to_string()
            .contains("no audio device"));
    }

    #[test]
    fn the_no_backend_message_explains_itself() {
        let message = EngineError::NoBackend.to_string();
        assert!(message.contains("no playback engine"));
        assert!(message.contains("libmpv"));
    }

    #[test]
    fn only_finite_non_negative_seek_targets_are_accepted() {
        assert!(is_seekable_position(0.0));
        assert!(is_seekable_position(12.5));
        assert!(!is_seekable_position(-1.0));
        assert!(!is_seekable_position(f64::NAN));
        assert!(!is_seekable_position(f64::INFINITY));
    }

    #[test]
    fn a_title_is_the_file_stem() {
        assert_eq!(title_for("/media/movies/Arrival.2016.mkv"), "Arrival.2016");
        assert_eq!(title_for("clip.mp4"), "clip");
        // A URL has no usable stem component; show it whole rather than inventing one.
        assert_eq!(title_for(""), "");
    }

    #[test]
    fn an_idle_state_serializes_as_the_ui_expects() {
        let json = serde_json::to_value(PlaybackState::default()).expect("serialize");
        assert_eq!(json["status"], "idle");
        assert_eq!(json["positionSecs"], 0.0);
        assert!(json["media"].is_null());
        // Full volume and normal speed at rest, so the UI never renders a muted 0% or 0×.
        assert_eq!(json["volume"], 100.0);
        assert_eq!(json["muted"], false);
        assert_eq!(json["speed"], 1.0);
        assert!(json["abLoop"]["a"].is_null());
        // Phase 2 subtitle transport: nothing forced, visible, default placement.
        assert!(json["audioId"].is_null());
        assert!(json["subtitle"]["id"].is_null());
        assert!(json["subtitle"]["secondaryId"].is_null());
        assert_eq!(json["subtitle"]["visible"], true);
        assert_eq!(json["subtitle"]["delaySecs"], 0.0);
        assert_eq!(json["subtitle"]["pos"], SUB_POS_DEFAULT);
        assert_eq!(json["subtitle"]["scale"], 1.0);
    }

    #[test]
    fn a_track_serializes_as_the_ui_expects() {
        let track = Track {
            id: 2,
            kind: TrackKind::Sub,
            lang: Some("en".to_owned()),
            title: Some("English (SDH)".to_owned()),
            default: true,
            external: false,
            image_based: false,
        };
        let json = serde_json::to_value(&track).expect("serialize");
        assert_eq!(json["id"], 2);
        assert_eq!(json["kind"], "sub");
        assert_eq!(json["lang"], "en");
        assert_eq!(json["imageBased"], false);
    }

    #[test]
    fn subtitle_position_is_clamped_to_mpvs_scale() {
        assert_eq!(clamp_sub_pos(100), 100);
        assert_eq!(clamp_sub_pos(-20), 0);
        assert_eq!(clamp_sub_pos(999), 150);
    }

    #[test]
    fn subtitle_delay_is_clamped_and_rejects_nonfinite() {
        assert_eq!(clamp_sub_delay(2.5), 2.5);
        assert_eq!(clamp_sub_delay(1_000.0), SUB_DELAY_MAX_SECS);
        assert_eq!(clamp_sub_delay(-1_000.0), -SUB_DELAY_MAX_SECS);
        assert_eq!(clamp_sub_delay(f64::NAN), 0.0);
    }

    #[test]
    fn subtitle_scale_is_clamped_and_rejects_nonfinite() {
        assert_eq!(clamp_sub_scale(1.0), 1.0);
        assert_eq!(clamp_sub_scale(0.01), SUB_SCALE_MIN);
        assert_eq!(clamp_sub_scale(99.0), SUB_SCALE_MAX);
        assert_eq!(clamp_sub_scale(f64::INFINITY), 1.0);
    }

    #[test]
    fn speed_is_clamped_to_the_supported_range() {
        assert_eq!(clamp_speed(1.0), 1.0);
        assert_eq!(clamp_speed(0.1), SPEED_MIN);
        assert_eq!(clamp_speed(9.0), SPEED_MAX);
        // A non-finite request must never reach the backend.
        assert_eq!(clamp_speed(f64::NAN), 1.0);
        assert_eq!(clamp_speed(f64::INFINITY), 1.0);
    }

    #[test]
    fn volume_is_clamped_onto_the_zero_hundred_scale() {
        assert_eq!(clamp_volume(50.0), 50.0);
        assert_eq!(clamp_volume(-5.0), 0.0);
        assert_eq!(clamp_volume(250.0), 100.0);
        assert_eq!(clamp_volume(f64::NAN), 100.0);
    }
}
