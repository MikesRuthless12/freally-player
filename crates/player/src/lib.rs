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
}

/// The transport snapshot the UI mirrors. **No pixels travel this path.**
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackState {
    pub status: Status,
    pub position_secs: f64,
    pub media: Option<MediaInfo>,
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

/// A decode/render backend. Implementors drive a native GPU surface; the orchestration above
/// them only ever moves transport state.
pub trait Engine: Send {
    /// Open `path` (a local file or a URL) and describe what was opened.
    fn open(&mut self, path: &str) -> Result<MediaInfo, EngineError>;
    fn play(&mut self) -> Result<(), EngineError>;
    fn pause(&mut self) -> Result<(), EngineError>;
    /// Seek to an absolute position in seconds.
    fn seek(&mut self, position_secs: f64) -> Result<(), EngineError>;
    /// The current transport snapshot.
    fn state(&self) -> PlaybackState;

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
    fn set_surface_rect(&self, x: i32, y: i32, width: u32, height: u32) {
        let _ = (x, y, width, height);
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
    fn open(&mut self, _path: &str) -> Result<MediaInfo, EngineError> {
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
        assert_eq!(engine.open("clip.mkv"), Err(EngineError::NoBackend));
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
            engine.open("clip.mkv"),
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
    }
}
