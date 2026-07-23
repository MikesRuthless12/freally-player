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

use freally_library::{RecentWatch, WatchState, WatchStore};
use freally_player_core::{
    is_seekable_position, Engine, MediaInfo, NullEngine, OpenOptions, PlaybackState, Status,
};
use freally_subtitles::SubtitlePrefsStore;

use crate::settings::SettingsStore;
// Only the Windows surface host can be named yet — see `attach_surface`.
#[cfg(windows)]
use freally_player_core::HostWindow;
use tauri::{AppHandle, State};

use crate::events;

/// Below this many seconds in, nothing meaningful was watched — a resume point would only
/// annoy, so none is saved and none is offered.
const MIN_RESUME_SECS: f64 = 5.0;
/// Within this many seconds of the end, the file is effectively finished: its resume point is
/// forgotten so it reopens from the start.
const END_MARGIN_SECS: f64 = 10.0;

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

    /// The currently selected `(audio_id, sub_id)`, for persisting last-used tracks.
    pub fn current_tracks(&self) -> (Option<i64>, Option<i64>) {
        self.with(|engine| engine.current_tracks())
    }

    pub(crate) fn with<R>(&self, f: impl FnOnce(&mut dyn Engine) -> R) -> R {
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

/// The resume point stored for `path`, as an [`OpenOptions`] — a start position plus the tracks
/// that were playing — or the defaults when there is nothing worth resuming to.
fn resume_options(watch: &WatchStore, path: &str) -> OpenOptions {
    let Some(saved) = watch.get(Path::new(path)) else {
        return OpenOptions::default();
    };
    match resume_target(saved.position_secs, saved.duration_secs) {
        Some(start) => OpenOptions {
            start_secs: Some(start),
            audio_id: saved.audio_id,
            sub_id: saved.sub_id,
        },
        None => OpenOptions::default(),
    }
}

/// The position to resume at, or `None` when it is too near the start (nothing watched) or the
/// end (effectively finished) to be worth it.
fn resume_target(position_secs: f64, duration_secs: Option<f64>) -> Option<f64> {
    if !position_secs.is_finite() || position_secs < MIN_RESUME_SECS {
        return None;
    }
    if let Some(duration) = duration_secs {
        if position_secs >= duration - END_MARGIN_SECS {
            return None;
        }
    }
    Some(position_secs)
}

/// Remember where the currently-open file was left, so it reopens there. Called before opening
/// a new file, periodically while one plays, and as the app closes.
///
/// A file watched to its end forgets its point instead (so it reopens from the start); one
/// barely started saves nothing. Best-effort throughout: a failed save is logged, never fatal.
pub fn persist_watch_state(player: &PlayerState, watch: &WatchStore) {
    let state = player.snapshot();
    let Some(media) = state.media else {
        return;
    };
    let path = Path::new(&media.path);
    let position = state.position_secs;

    if let Some(duration) = media.duration_secs {
        if duration > 0.0 && position >= duration - END_MARGIN_SECS {
            let _ = watch.clear(path);
            return;
        }
    }
    if !position.is_finite() || position < MIN_RESUME_SECS {
        return;
    }

    let (audio_id, sub_id) = player.current_tracks();
    if let Err(err) = watch.set(
        path,
        WatchState {
            position_secs: position,
            duration_secs: media.duration_secs,
            audio_id,
            sub_id,
        },
    ) {
        log::warn!("could not save the resume point for {}: {err}", media.path);
    }
}

#[tauri::command]
pub fn open_media(
    app: AppHandle,
    player: State<'_, PlayerState>,
    watch: State<'_, WatchStore>,
    subs: State<'_, SubtitlePrefsStore>,
    settings: State<'_, SettingsStore>,
    path: String,
) -> Result<MediaInfo, String> {
    validate_target(&path)?;

    // Save where the outgoing file was left, and its subtitle timing, before it is replaced;
    // then resume the new one where it was last stopped — with the tracks that were playing.
    persist_watch_state(&player, &watch);
    crate::commands::subtitles::persist_subtitle_prefs(&player, &subs);
    let options = resume_options(&watch, &path);

    let media = player
        .with(|engine| engine.open(&path, &options))
        .map_err(|err| err.to_string())?;

    // Re-apply the global subtitle style override and this file's remembered timing/placement.
    crate::commands::subtitles::apply_on_open(&player, &subs, &settings, &path);

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

/// Toggle play/pause — what Space and the OS play/pause key do. Pausing when already paused
/// would be a no-op, so this reads the current status and does the opposite.
#[tauri::command]
pub fn toggle_play(app: AppHandle, player: State<'_, PlayerState>) -> Result<(), String> {
    let playing = matches!(player.snapshot().status, Status::Playing);
    let outcome = if playing {
        player.with(|engine| engine.pause())
    } else {
        player.with(|engine| engine.play())
    };
    outcome.map_err(|err| err.to_string())?;
    events::emit_state(&app, &player.with(|engine| engine.state()));
    Ok(())
}

#[tauri::command]
pub fn set_volume(
    app: AppHandle,
    player: State<'_, PlayerState>,
    volume: f64,
) -> Result<(), String> {
    player
        .with(|engine| engine.set_volume(volume))
        .map_err(|err| err.to_string())?;
    events::emit_state(&app, &player.with(|engine| engine.state()));
    Ok(())
}

#[tauri::command]
pub fn set_muted(
    app: AppHandle,
    player: State<'_, PlayerState>,
    muted: bool,
) -> Result<(), String> {
    player
        .with(|engine| engine.set_muted(muted))
        .map_err(|err| err.to_string())?;
    events::emit_state(&app, &player.with(|engine| engine.state()));
    Ok(())
}

#[tauri::command]
pub fn set_speed(app: AppHandle, player: State<'_, PlayerState>, speed: f64) -> Result<(), String> {
    player
        .with(|engine| engine.set_speed(speed))
        .map_err(|err| err.to_string())?;
    events::emit_state(&app, &player.with(|engine| engine.state()));
    Ok(())
}

/// Step a single frame forward (`forward: true`) or back. mpv pauses as it does.
#[tauri::command]
pub fn frame_step(
    app: AppHandle,
    player: State<'_, PlayerState>,
    forward: bool,
) -> Result<(), String> {
    player
        .with(|engine| engine.frame_step(forward))
        .map_err(|err| err.to_string())?;
    events::emit_state(&app, &player.with(|engine| engine.state()));
    Ok(())
}

/// Set or clear the A–B repeat range. A `null` end clears it.
#[tauri::command]
pub fn set_ab_loop(
    app: AppHandle,
    player: State<'_, PlayerState>,
    a: Option<f64>,
    b: Option<f64>,
) -> Result<(), String> {
    player
        .with(|engine| engine.set_ab_loop(a, b))
        .map_err(|err| err.to_string())?;
    events::emit_state(&app, &player.with(|engine| engine.state()));
    Ok(())
}

/// Jump to a chapter by its index in the open media's chapter list.
#[tauri::command]
pub fn set_chapter(
    app: AppHandle,
    player: State<'_, PlayerState>,
    index: usize,
) -> Result<(), String> {
    player
        .with(|engine| engine.set_chapter(index))
        .map_err(|err| err.to_string())?;
    events::emit_state(&app, &player.with(|engine| engine.state()));
    Ok(())
}

/// The recently-watched items for the idle screen's Continue-Watching row, newest first.
#[tauri::command]
pub fn recent_watch(watch: State<'_, WatchStore>, limit: usize) -> Vec<RecentWatch> {
    // Bound the ask so a bad `limit` can never make the store walk itself pathologically.
    watch.recent(limit.min(50))
}

/// Write the current frame to `path`. `with_subs` bakes in the subtitle overlay.
///
/// The path comes from the UI's native save dialog — the web layer never chooses a filesystem
/// location itself. mpv reports its own error (e.g. an unwritable location) verbatim.
#[tauri::command]
pub fn capture_frame(
    player: State<'_, PlayerState>,
    path: String,
    with_subs: bool,
) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("no snapshot path was given".to_owned());
    }
    player
        .with(|engine| engine.capture_frame(&path, with_subs))
        .map_err(|err| err.to_string())
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

    #[test]
    fn a_barely_started_file_offers_no_resume() {
        // Nothing meaningful was watched, so reopening starts from the beginning.
        assert_eq!(resume_target(2.0, Some(3600.0)), None);
        assert_eq!(resume_target(0.0, None), None);
    }

    #[test]
    fn a_mid_file_position_resumes() {
        assert_eq!(resume_target(1200.0, Some(3600.0)), Some(1200.0));
        // Duration unknown (still parsing / a stream) — resume on position alone.
        assert_eq!(resume_target(1200.0, None), Some(1200.0));
    }

    #[test]
    fn a_finished_file_offers_no_resume() {
        // Within the end margin counts as finished; it reopens from the start.
        assert_eq!(resume_target(3595.0, Some(3600.0)), None);
        assert_eq!(resume_target(3600.0, Some(3600.0)), None);
    }

    #[test]
    fn a_non_finite_position_never_resumes() {
        assert_eq!(resume_target(f64::NAN, Some(3600.0)), None);
        assert_eq!(resume_target(f64::INFINITY, None), None);
    }

    /// With nothing remembered, opening a file uses plain defaults — no start, no forced
    /// tracks.
    #[test]
    fn a_file_with_no_saved_point_opens_at_defaults() {
        let dir = std::env::temp_dir().join(format!("freally-resume-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let watch = WatchStore::load_from(dir.join("watch_state.json"));
        assert_eq!(
            resume_options(&watch, "C:/Videos/never-seen.mkv"),
            OpenOptions::default()
        );
    }
}
