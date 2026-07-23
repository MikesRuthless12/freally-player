//! Subtitle & audio-track commands: switching, external load, timing/placement, the style
//! override, and the opt-in OpenSubtitles fetch.
//!
//! Rendering stays the engine's job (libass draws into the video surface) — these commands only
//! *select and adjust* it, and own the untrusted/online bits the engine should not be handed
//! raw. Every mutating command emits `player://state` so the menu mirrors the transport the
//! same way the control bar does.
//!
//! Per-file timing/placement is remembered in the subtitle-preferences store; the style
//! override is a global accessibility setting. Both are re-applied when a file opens (see
//! [`apply_on_open`]).

use std::path::Path;
use std::sync::Mutex;

use freally_player_core::SubStyleOverride;
use freally_subtitles::{
    Candidate, LoadedSubtitle, OpenSubtitlesClient, SubtitlePrefs, SubtitlePrefsStore,
};
use serde::Serialize;
use tauri::{AppHandle, State};

use super::playback::PlayerState;
use crate::events;
use crate::settings::SettingsStore;

/// The in-memory OpenSubtitles session. The login token lives only here, for the session — it
/// is never written to disk, and neither is the password that obtains it.
#[derive(Default)]
pub struct OpenSubtitlesState {
    token: Mutex<Option<String>>,
}

/// What loading an external/online subtitle produced, for the UI to confirm honestly (e.g.
/// "loaded as Windows-1251").
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedSubtitleInfo {
    /// The new subtitle track's id (its `sid`).
    pub track_id: i64,
    /// The charset a text subtitle was transcoded from, or `null` when it was already UTF-8 or
    /// is image-based.
    pub source_encoding: Option<String>,
    /// Whether the track is image-based (PGS/VobSub) — no style/font override applies.
    pub image_based: bool,
}

/// The User-Agent the OpenSubtitles API requires, identifying the app and version.
fn user_agent() -> String {
    format!("Freally Player v{}", env!("CARGO_PKG_VERSION"))
}

/// Persist the current subtitle timing/placement for the open file, so it is there next time.
/// Best-effort: a failed save is logged, never fatal. Called after every adjustment and before
/// switching files.
pub fn persist_subtitle_prefs(player: &PlayerState, subs: &SubtitlePrefsStore) {
    let state = player.snapshot();
    let Some(media) = state.media else {
        return;
    };
    let sub = state.subtitle;
    let prefs = SubtitlePrefs {
        delay_secs: sub.delay_secs,
        pos: sub.pos,
        scale: sub.scale,
        visible: sub.visible,
        secondary_id: sub.secondary_id,
    };
    if let Err(err) = subs.set(Path::new(&media.path), prefs) {
        log::warn!(
            "could not save subtitle preferences for {}: {err}",
            media.path
        );
    }
}

/// Re-apply saved subtitle state when a file opens: the global style override first, then this
/// file's remembered timing/placement. All best-effort — a failure here must never break the
/// open, so each step is logged and playback continues with the default.
pub fn apply_on_open(
    player: &PlayerState,
    subs: &SubtitlePrefsStore,
    settings: &SettingsStore,
    path: &str,
) {
    let style = settings.get().subtitle_style;
    let prefs = subs.get(Path::new(path));
    // Apply everything under a single lock acquisition rather than one per property.
    player.with(|engine| {
        if let Err(err) = engine.set_sub_style_override(&style) {
            log::warn!("could not apply the subtitle style override: {err}");
        }
        if let Some(prefs) = prefs {
            let _ = engine.set_sub_delay(prefs.delay_secs);
            let _ = engine.set_sub_pos(prefs.pos);
            let _ = engine.set_sub_scale(prefs.scale);
            let _ = engine.set_sub_visible(prefs.visible);
            if let Some(secondary) = prefs.secondary_id {
                let _ = engine.set_secondary_sub_track(Some(secondary));
            }
        }
    });
}

/// Emit the new transport snapshot — the tail of every subtitle-adjusting command.
///
/// Persistence is deliberately NOT done here: the position/scale controls are range sliders whose
/// `onChange` fires continuously during a drag, so persisting per event would be dozens of full-
/// store, fsync'd rewrites per gesture. Subtitle preferences flush on the same cadence as watch
/// state — the ~5 s transport ticker, on file-switch (`open_media`), and on window close.
fn emit_after_change(app: &AppHandle, player: &PlayerState) {
    events::emit_state(app, &player.snapshot());
}

#[tauri::command]
pub fn set_audio_track(
    app: AppHandle,
    player: State<'_, PlayerState>,
    id: Option<i64>,
) -> Result<(), String> {
    player
        .with(|engine| engine.set_audio_track(id))
        .map_err(|err| err.to_string())?;
    // Audio-track selection is remembered by the watch-state store, not the subtitle prefs.
    events::emit_state(&app, &player.snapshot());
    Ok(())
}

#[tauri::command]
pub fn set_subtitle_track(
    app: AppHandle,
    player: State<'_, PlayerState>,
    id: Option<i64>,
) -> Result<(), String> {
    player
        .with(|engine| engine.set_sub_track(id))
        .map_err(|err| err.to_string())?;
    events::emit_state(&app, &player.snapshot());
    Ok(())
}

#[tauri::command]
pub fn set_secondary_subtitle_track(
    app: AppHandle,
    player: State<'_, PlayerState>,
    id: Option<i64>,
) -> Result<(), String> {
    player
        .with(|engine| engine.set_secondary_sub_track(id))
        .map_err(|err| err.to_string())?;
    emit_after_change(&app, &player);
    Ok(())
}

#[tauri::command]
pub fn set_subtitle_visible(
    app: AppHandle,
    player: State<'_, PlayerState>,
    visible: bool,
) -> Result<(), String> {
    player
        .with(|engine| engine.set_sub_visible(visible))
        .map_err(|err| err.to_string())?;
    emit_after_change(&app, &player);
    Ok(())
}

#[tauri::command]
pub fn set_subtitle_delay(
    app: AppHandle,
    player: State<'_, PlayerState>,
    secs: f64,
) -> Result<(), String> {
    player
        .with(|engine| engine.set_sub_delay(secs))
        .map_err(|err| err.to_string())?;
    emit_after_change(&app, &player);
    Ok(())
}

#[tauri::command]
pub fn set_subtitle_pos(
    app: AppHandle,
    player: State<'_, PlayerState>,
    pos: i64,
) -> Result<(), String> {
    player
        .with(|engine| engine.set_sub_pos(pos))
        .map_err(|err| err.to_string())?;
    emit_after_change(&app, &player);
    Ok(())
}

#[tauri::command]
pub fn set_subtitle_scale(
    app: AppHandle,
    player: State<'_, PlayerState>,
    scale: f64,
) -> Result<(), String> {
    player
        .with(|engine| engine.set_sub_scale(scale))
        .map_err(|err| err.to_string())?;
    emit_after_change(&app, &player);
    Ok(())
}

/// Apply (and persist as a global setting) the subtitle style override. Applies live to the open
/// file and is re-applied to every file opened afterward.
#[tauri::command]
pub fn set_subtitle_style_override(
    app: AppHandle,
    player: State<'_, PlayerState>,
    settings: State<'_, SettingsStore>,
    style: SubStyleOverride,
) -> Result<(), String> {
    let mut next = settings.user_settings();
    next.subtitle_style = style.clone();
    settings
        .set_user_settings(next)
        .map_err(|err| format!("could not save the subtitle style: {err}"))?;
    // Best-effort on the live engine: nothing open just means it takes effect on the next open.
    let _ = player.with(|engine| engine.set_sub_style_override(&style));
    events::emit_state(&app, &player.snapshot());
    Ok(())
}

/// Attach an already-loaded subtitle to the engine and report what was loaded. Shared by the
/// local-file and online-download commands, which differ only in how they obtain the
/// [`LoadedSubtitle`].
fn attach_loaded(
    app: &AppHandle,
    player: &PlayerState,
    loaded: LoadedSubtitle,
) -> Result<LoadedSubtitleInfo, String> {
    let engine_path = loaded.path.to_string_lossy().into_owned();
    let track_id = player
        .with(|engine| engine.add_sub_file(&engine_path, true))
        .map_err(|err| err.to_string())?;
    emit_after_change(app, player);
    Ok(LoadedSubtitleInfo {
        track_id,
        source_encoding: loaded.source_encoding,
        image_based: loaded.image_based,
    })
}

/// Load an external subtitle file. The path comes from the UI's native file picker — the web
/// layer never touches the filesystem itself. The file is treated as untrusted: bounded, and
/// transcoded to UTF-8 when it is legacy-charset text, before the engine renders it.
#[tauri::command]
pub fn add_subtitle_file(
    app: AppHandle,
    player: State<'_, PlayerState>,
    path: String,
) -> Result<LoadedSubtitleInfo, String> {
    let loaded =
        freally_subtitles::load_external(Path::new(&path)).map_err(|err| err.to_string())?;
    attach_loaded(&app, &player, loaded)
}

// --- Opt-in OpenSubtitles ---------------------------------------------------

/// Build a client from the stored, opt-in configuration, refusing clearly when it is off or has
/// no key rather than silently doing nothing.
fn opensubtitles_client(settings: &SettingsStore) -> Result<OpenSubtitlesClient, String> {
    let config = settings.get().opensubtitles;
    if !config.enabled {
        return Err("online subtitle fetch is turned off".to_owned());
    }
    let key = config
        .api_key
        .filter(|k| !k.trim().is_empty())
        .ok_or("no OpenSubtitles API key is set — add yours in Settings")?;
    OpenSubtitlesClient::new(key, user_agent()).map_err(|err| err.to_string())
}

/// Search OpenSubtitles by a free-text query and language codes. Only these identifiers leave
/// the machine.
#[tauri::command]
pub fn opensubtitles_search(
    settings: State<'_, SettingsStore>,
    query: String,
    languages: Vec<String>,
) -> Result<Vec<Candidate>, String> {
    if query.trim().is_empty() {
        return Err("enter something to search for".to_owned());
    }
    opensubtitles_client(&settings)?
        .search(query.trim(), &languages)
        .map_err(|err| err.to_string())
}

/// Sign in to OpenSubtitles for the session. The password is used to obtain a token and then
/// dropped — it is never stored.
#[tauri::command]
pub fn opensubtitles_login(
    settings: State<'_, SettingsStore>,
    os_state: State<'_, OpenSubtitlesState>,
    username: String,
    password: String,
) -> Result<(), String> {
    let token = opensubtitles_client(&settings)?
        .login(username.trim(), &password)
        .map_err(|err| err.to_string())?;
    *os_state.token.lock().unwrap_or_else(|e| e.into_inner()) = Some(token);
    Ok(())
}

/// Download a chosen candidate and attach it as a subtitle track. Needs a session login first;
/// the downloaded file is run through the same untrusted loader as a local one.
#[tauri::command]
pub fn opensubtitles_download(
    app: AppHandle,
    player: State<'_, PlayerState>,
    settings: State<'_, SettingsStore>,
    os_state: State<'_, OpenSubtitlesState>,
    file_id: i64,
) -> Result<LoadedSubtitleInfo, String> {
    let client = opensubtitles_client(&settings)?;
    let token = os_state
        .token
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .ok_or("sign in to OpenSubtitles first")?;

    let link = client
        .download_link(&token, file_id)
        .map_err(|err| err.to_string())?;
    let bytes = client.fetch_bytes(&link).map_err(|err| err.to_string())?;

    // Downloaded content is as untrusted as a local file — run the bytes through the same
    // bounded, transcoding loader (no on-disk round trip), then attach it like any other.
    let loaded =
        freally_subtitles::load_external_bytes(&bytes, "srt").map_err(|err| err.to_string())?;
    attach_loaded(&app, &player, loaded)
}
