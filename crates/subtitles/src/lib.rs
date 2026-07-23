//! `freally-subtitles` — owned subtitle pipeline: load, sync, and manage around the engine's
//! renderer.
//!
//! The engine (libass / libavcodec) does the **rendering**; this crate owns the parts it should
//! not be handed raw:
//!
//! - [`load`] — bounded, untrusted loading of external subtitle files, with **encoding
//!   auto-detect** so a legacy-charset text subtitle is transcoded to UTF-8 before libass sees
//!   it, and image-based tracks (PGS/VobSub) are passed through by path.
//! - [`prefs`] — per-file subtitle timing/placement/scale, remembered like watch-state.
//! - [`opensubtitles`] — the **opt-in** online fetch, off unless the user enables it and
//!   supplies their own API key.
//!
//! Subtitle files are untrusted input; online fetch is opt-in.

#![forbid(unsafe_code)]

mod load;
mod opensubtitles;
mod prefs;

pub use load::{LoadedSubtitle, SubtitleError, MAX_IMAGE_SUBTITLE_BYTES, MAX_TEXT_SUBTITLE_BYTES};
pub use opensubtitles::{Candidate, FetchError, OpenSubtitlesClient};
pub use prefs::{SubtitlePrefs, SubtitlePrefsStore};

/// Load an external subtitle file for the engine to render — bounded, and transcoded to UTF-8
/// when it is legacy-charset text. See [`load::load`].
pub fn load_external(path: &std::path::Path) -> Result<LoadedSubtitle, SubtitleError> {
    load::load(path)
}

/// Load an external subtitle from an in-memory byte buffer (e.g. an online download) — bounded,
/// charset-detected, and transcoded to a UTF-8 temp file. `ext` sets the format hint.
pub fn load_external_bytes(bytes: &[u8], ext: &str) -> Result<LoadedSubtitle, SubtitleError> {
    load::load_bytes(bytes, ext)
}
