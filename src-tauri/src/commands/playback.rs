//! Playback transport commands: `open_media`, `play`, `pause`, `seek`, `get_state`.
//!
//! These own the app's single [`Engine`] instance and are the *only* way the UI can touch
//! playback. Every mutating command emits `player://state` so the UI mirrors the transport
//! from events rather than polling — and so that a change made anywhere (media keys, the
//! engine reaching end-of-file) reaches the UI by the same path.
//!
//! Errors are surfaced to the user verbatim. Per the honesty invariant, a build with no
//! decode backend says exactly that instead of failing silently.

use std::path::Path;
use std::sync::Mutex;

use freally_player_core::{is_seekable_position, Engine, MediaInfo, NullEngine, PlaybackState};
// Only the Windows surface host can be named yet — see `attach_surface`.
#[cfg(windows)]
use freally_player_core::HostWindow;
use tauri::{AppHandle, State};

use crate::events;

/// The app's single engine instance, behind a mutex because Tauri commands run on a pool.
pub struct PlayerState {
    engine: Mutex<Box<dyn Engine>>,
}

impl PlayerState {
    /// The engine this build ships.
    pub fn new() -> Self {
        Self {
            engine: Mutex::new(Self::backend()),
        }
    }

    /// libmpv when it is compiled in and starts; otherwise an engine that refuses honestly.
    ///
    /// A failed libmpv start must not be reported as "built without libmpv" — the user would
    /// chase the wrong problem — so the real reason is carried through.
    #[cfg(feature = "engine-libmpv")]
    fn backend() -> Box<dyn Engine> {
        match freally_player_core::MpvEngine::new() {
            Ok(engine) => Box::new(engine),
            Err(err) => {
                log::error!("libmpv failed to initialise: {err}");
                Box::new(NullEngine::unavailable(err.to_string()))
            }
        }
    }

    #[cfg(not(feature = "engine-libmpv"))]
    fn backend() -> Box<dyn Engine> {
        Box::new(NullEngine::default())
    }

    /// The current transport snapshot.
    pub fn snapshot(&self) -> PlaybackState {
        self.with(|engine| engine.state())
    }

    fn with<R>(&self, f: impl FnOnce(&mut dyn Engine) -> R) -> R {
        let mut guard = self
            .engine
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut **guard)
    }

    /// Create the native video surface inside the app window.
    ///
    /// Failure is logged, not fatal: the rest of the app (settings, bug reporter, audio-only
    /// playback) still works, and the reason reaches the user the first time they open media.
    ///
    /// Windows-only for now because [`HostWindow`] can only name a Win32 handle; the macOS
    /// and Linux hosts add their own variants when those surfaces land.
    #[cfg(windows)]
    pub fn attach_surface(&self, host: HostWindow, width: u32, height: u32) {
        if let Err(err) = self.with(|engine| engine.attach_surface(host, width, height)) {
            log::error!("could not create the native video surface: {err}");
        }
    }

    /// Keep the video surface matched to the stage rect the UI reports.
    pub fn set_surface_rect(&self, x: i32, y: i32, width: u32, height: u32, visible: bool) {
        self.with(|engine| engine.set_surface_rect(x, y, width, height, visible));
    }
}

impl Default for PlayerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Does this look like a network target rather than a local path?
///
/// Network playback lands in Phase 5; until then the check exists so a URL is not rejected as
/// a missing file by [`validate_target`].
fn looks_like_url(target: &str) -> bool {
    target.contains("://")
}

/// Reject a target the UI has no business asking for before it reaches a backend.
///
/// A local path must exist and be a regular file, so a typo or a directory fails here with a
/// clear message instead of deep inside the decoder. This is also the audit point for the
/// privacy invariant: the web layer never touches the filesystem itself.
fn validate_target(target: &str) -> Result<(), String> {
    if target.trim().is_empty() {
        return Err("no media path was given".to_owned());
    }
    if looks_like_url(target) {
        return Ok(());
    }
    let path = Path::new(target);
    if !path.exists() {
        return Err(format!("no such file: {target}"));
    }
    if !path.is_file() {
        return Err(format!("not a file: {target}"));
    }
    Ok(())
}

#[tauri::command]
pub fn open_media(
    app: AppHandle,
    player: State<'_, PlayerState>,
    path: String,
) -> Result<MediaInfo, String> {
    validate_target(&path)?;

    let media = player
        .with(|engine| engine.open(&path))
        .map_err(|err| err.to_string())?;

    events::emit_media_opened(&app, &media);
    events::emit_state(&app, &player.with(|engine| engine.state()));
    Ok(media)
}

#[tauri::command]
pub fn play(app: AppHandle, player: State<'_, PlayerState>) -> Result<(), String> {
    player
        .with(|engine| engine.play())
        .map_err(|err| err.to_string())?;
    events::emit_state(&app, &player.with(|engine| engine.state()));
    Ok(())
}

#[tauri::command]
pub fn pause(app: AppHandle, player: State<'_, PlayerState>) -> Result<(), String> {
    player
        .with(|engine| engine.pause())
        .map_err(|err| err.to_string())?;
    events::emit_state(&app, &player.with(|engine| engine.state()));
    Ok(())
}

#[tauri::command]
pub fn seek(
    app: AppHandle,
    player: State<'_, PlayerState>,
    position_secs: f64,
) -> Result<(), String> {
    // Checked here as well as in the backend: a NaN crossing IPC must never reach an engine.
    if !is_seekable_position(position_secs) {
        return Err(format!("cannot seek to {position_secs} seconds"));
    }
    player
        .with(|engine| engine.seek(position_secs))
        .map_err(|err| err.to_string())?;
    events::emit_state(&app, &player.with(|engine| engine.state()));
    Ok(())
}

/// The current transport snapshot — the UI's initial read before events take over.
#[tauri::command]
pub fn get_state(player: State<'_, PlayerState>) -> PlaybackState {
    player.with(|engine| engine.state())
}

/// Tell the core where the video stage is, in physical pixels relative to the window's
/// client area.
///
/// The native video surface is a sibling window placed **over** the webview, so it must track
/// the stage element's geometry exactly or it would cover the chrome. The UI reports this
/// whenever the stage is laid out or resized.
#[tauri::command]
pub fn set_video_rect(
    player: State<'_, PlayerState>,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    visible: bool,
) {
    player.set_surface_rect(x, y, width, height, visible);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_target_is_refused() {
        assert!(validate_target("").is_err());
        assert!(validate_target("   ").is_err());
    }

    #[test]
    fn a_missing_local_file_is_refused_by_name() {
        let err = validate_target("Z:/definitely/not/here.mkv").expect_err("should fail");
        assert!(err.contains("no such file"));
    }

    #[test]
    fn a_directory_is_not_playable() {
        let dir = std::env::temp_dir();
        let err = validate_target(&dir.to_string_lossy()).expect_err("should fail");
        assert!(err.contains("not a file"));
    }

    #[test]
    fn an_existing_file_passes() {
        let path = std::env::temp_dir().join(format!("freally-open-{}.mkv", std::process::id()));
        std::fs::write(&path, b"not really a video").expect("write fixture");
        assert!(validate_target(&path.to_string_lossy()).is_ok());
        let _ = std::fs::remove_file(&path);
    }

    /// A URL must not be filesystem-checked — Phase 5 plays these for real.
    #[test]
    fn a_url_is_passed_through_without_a_filesystem_check() {
        assert!(validate_target("https://example.com/stream.m3u8").is_ok());
        assert!(validate_target("rtsp://camera.local/live").is_ok());
        assert!(looks_like_url("smb://nas/share/movie.mkv"));
        assert!(!looks_like_url("C:/Videos/movie.mkv"));
    }

    #[test]
    fn a_fresh_player_state_is_idle() {
        let player = PlayerState::new();
        assert_eq!(
            player.with(|engine| engine.state()),
            PlaybackState::default()
        );
    }
}
