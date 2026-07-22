//! The `player://…` event channel the UI mirrors playback state from.
//!
//! The UI never polls and never receives pixels — it renders chrome from these snapshots
//! while the native surface underneath draws the video. Emitting is best-effort: a dropped
//! event is logged, never fatal, because the UI can always re-read `get_state`.

use freally_player_core::{MediaInfo, PlaybackState};
use tauri::{AppHandle, Emitter};

/// Emitted after every transport change.
pub const STATE: &str = "player://state";
/// Emitted when a new media item has been opened.
pub const MEDIA_OPENED: &str = "player://media-opened";

pub fn emit_state(app: &AppHandle, state: &PlaybackState) {
    if let Err(e) = app.emit(STATE, state) {
        log::warn!("could not emit {STATE}: {e}");
    }
}

pub fn emit_media_opened(app: &AppHandle, media: &MediaInfo) {
    if let Err(e) = app.emit(MEDIA_OPENED, media) {
        log::warn!("could not emit {MEDIA_OPENED}: {e}");
    }
}
