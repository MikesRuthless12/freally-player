//! Freally Player — the Tauri v2 app shell.
//!
//! Local-first, cross-platform media player. This crate hosts the window shell, the
//! settings store, the first-run EULA gate, the opt-in anonymous bug reporter, and (from
//! P0.3) the native video surface composited *under* the webview plus the UI ↔ core
//! command/event bridge. The playback engine itself lives in the owned `freally-*` crates
//! behind the `Engine` boundary.
//!
//! Privacy invariant: the web UI has no direct network or filesystem access — every I/O
//! path goes through an audited Rust command here.

#![forbid(unsafe_code)]

mod bugreport;
mod commands;
mod eula;
mod events;
mod mediakeys;
mod paths;
mod settings;

use std::path::PathBuf;

use freally_library::WatchStore;
use freally_subtitles::SubtitlePrefsStore;
use serde::Serialize;
use tauri::{Manager, PhysicalSize, State, WindowEvent};

use commands::playback::PlayerState;
use commands::subtitles::OpenSubtitlesState;
use mediakeys::MediaKeys;
use settings::{SettingsStore, UserSettings, WindowSettings};

/// Identity shown in the UI's version banner.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    name: &'static str,
    version: &'static str,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: "Freally Player",
        version: env!("CARGO_PKG_VERSION"),
    }
}

/// The settings the Settings modal owns.
#[tauri::command]
fn settings_get(store: State<'_, SettingsStore>) -> UserSettings {
    store.user_settings()
}

/// Persist the settings the Settings modal owns.
#[tauri::command]
fn settings_set(store: State<'_, SettingsStore>, settings: UserSettings) -> Result<(), String> {
    store
        .set_user_settings(settings)
        .map_err(|err| format!("could not save settings: {err}"))
}

/// Build the tray icon and its menu.
///
/// The tray is always present so "minimize to tray" has somewhere to minimise *to* the moment
/// the user enables it — creating it on demand would mean the first minimise had nowhere to go.
fn build_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder};
    use tauri::tray::TrayIconBuilder;

    let show = MenuItemBuilder::with_id("show", "Show Freally Player").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
    let menu = MenuBuilder::new(app).items(&[&show, &quit]).build()?;

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().ok_or_else(|| {
            tauri::Error::AssetNotFound("the app has no default window icon".to_owned())
        })?)
        .tooltip("Freally Player")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => restore_from_tray(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // A plain left click is the obvious "give me the window back" gesture.
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                restore_from_tray(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Bring the main window back from the tray.
fn restore_from_tray(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

/// Build and run the app shell.
///
/// # Panics
/// Panics if Tauri cannot build the app (a broken `tauri.conf.json` or missing webview
/// runtime) — there is no usable app to fall back to at that point.
pub fn run() {
    let args: Vec<String> = std::env::args().collect();

    // The post-crash notice helper must be handled BEFORE any Tauri app is built: this
    // process exists only to show the native error box and relaunch the real app.
    if bugreport::run_crash_notice(&args) {
        return;
    }
    bugreport::install_panic_hook();
    bugreport::arm_test_crash(&args);
    let startup_args = args.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(SettingsStore::load_default())
        .manage(PlayerState::new())
        .manage(load_watch_store())
        .manage(load_subtitle_prefs_store())
        .manage(OpenSubtitlesState::default())
        .invoke_handler(tauri::generate_handler![
            app_info,
            settings_get,
            settings_set,
            eula::eula_status,
            eula::eula_accept,
            commands::playback::open_media,
            commands::playback::play,
            commands::playback::pause,
            commands::playback::seek,
            commands::playback::get_state,
            commands::playback::set_video_rect,
            commands::playback::toggle_play,
            commands::playback::set_volume,
            commands::playback::set_muted,
            commands::playback::set_speed,
            commands::playback::frame_step,
            commands::playback::set_ab_loop,
            commands::playback::set_chapter,
            commands::playback::capture_frame,
            commands::playback::recent_watch,
            commands::subtitles::set_audio_track,
            commands::subtitles::set_subtitle_track,
            commands::subtitles::set_secondary_subtitle_track,
            commands::subtitles::set_subtitle_visible,
            commands::subtitles::set_subtitle_delay,
            commands::subtitles::set_subtitle_pos,
            commands::subtitles::set_subtitle_scale,
            commands::subtitles::set_subtitle_style_override,
            commands::subtitles::add_subtitle_file,
            commands::subtitles::opensubtitles_search,
            commands::subtitles::opensubtitles_login,
            commands::subtitles::opensubtitles_download,
            bugreport::bug_report_context,
            bugreport::bug_report_submit,
            bugreport::bug_report_clear_crash,
            bugreport::open_external,
        ])
        .setup(move |app| {
            restore_window(app.handle());
            attach_video_surface(app.handle());
            app.manage(init_media_keys(app));
            spawn_transport_ticker(app.handle());
            if let Err(e) = build_tray(app.handle()) {
                // Not fatal: without a tray the app still runs, minimise just behaves
                // normally. Say why rather than silently ignoring the preference.
                log::error!("could not create the tray icon: {e}");
            }
            open_media_from_args(app.handle(), &startup_args);
            Ok(())
        })
        .on_window_event(|window, event| {
            // Persist geometry as the window goes away, so the next launch reopens where the
            // user left it. CloseRequested fires before the window is torn down, so the size
            // is still readable here.
            //
            // Resizes need nothing: the video surface follows the stage rect, which the UI
            // reports through `set_video_rect` whenever its own layout changes.
            if matches!(event, WindowEvent::CloseRequested { .. }) {
                save_window(window);
                // Remember where the open file was left and its subtitle timing, so both reopen
                // as the viewer left them next launch.
                commands::playback::persist_watch_state(
                    &window.state::<PlayerState>(),
                    &window.state::<WatchStore>(),
                );
                commands::subtitles::persist_subtitle_prefs(
                    &window.state::<PlayerState>(),
                    &window.state::<SubtitlePrefsStore>(),
                );
            }
            // "Minimize to system tray": hide the window entirely so it leaves the taskbar,
            // leaving the tray icon as the way back. Off by default, so a user who never
            // opens Settings sees ordinary minimise behaviour.
            if window.label() == "main" {
                if let WindowEvent::Resized(_) = event {
                    let minimized = window.is_minimized().unwrap_or(false);
                    if minimized && window.state::<SettingsStore>().get().minimize_to_tray {
                        let _ = window.hide();
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Freally Player");
}

/// Reapply the stored geometry to the main window.
fn restore_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let stored = app.state::<SettingsStore>().get().window;

    if let Err(e) = window.set_size(PhysicalSize::new(stored.width, stored.height)) {
        log::warn!("could not restore window size: {e}");
    }
    if stored.maximized {
        if let Err(e) = window.maximize() {
            log::warn!("could not restore maximized window: {e}");
        }
    }
}

/// Mirror the transport to the UI while it moves.
///
/// Commands emit `player://state` when the *user* does something, but nothing emits while a
/// file simply plays — so the position sat at 0:00 for the whole film. This samples a few
/// times a second and emits only when the snapshot actually changed, which also catches the
/// transitions no command covers: mpv pausing itself to buffer, or reaching end of file.
///
/// Emitting on change rather than on every tick keeps an idle or paused player silent.
fn spawn_transport_ticker(app: &tauri::AppHandle) {
    /// Four times a second: fine enough that a seconds display never looks stuck, coarse
    /// enough to stay invisible in CPU terms. Phase 1's scrubber can raise it if needed.
    const TICK: std::time::Duration = std::time::Duration::from_millis(250);
    /// Save a resume point about every five seconds of running, so a crash loses very little.
    const SAVE_EVERY_TICKS: u32 = 20;

    let app = app.clone();
    std::thread::Builder::new()
        .name("freally-transport-ticker".to_owned())
        .spawn(move || {
            let mut last: Option<freally_player_core::PlaybackState> = None;
            // The now-playing panel only needs refreshing when the title or its duration
            // changes, not on every position tick.
            let mut last_now_playing: Option<(String, Option<f64>)> = None;
            let mut ticks_since_save: u32 = 0;

            loop {
                std::thread::sleep(TICK);
                let state = app.state::<PlayerState>().snapshot();

                if last.as_ref() != Some(&state) {
                    events::emit_state(&app, &state);

                    let media_keys = app.state::<MediaKeys>();
                    media_keys.update_playback(&state);
                    // Refresh the OS now-playing metadata when the item (or its duration,
                    // known only after the header is parsed) changes.
                    let now_playing = state
                        .media
                        .as_ref()
                        .map(|m| (m.path.clone(), m.duration_secs));
                    if now_playing != last_now_playing {
                        if let Some(media) = &state.media {
                            media_keys.set_now_playing(media);
                        }
                        last_now_playing = now_playing;
                    }

                    last = Some(state);
                }

                ticks_since_save += 1;
                if ticks_since_save >= SAVE_EVERY_TICKS {
                    ticks_since_save = 0;
                    commands::playback::persist_watch_state(
                        &app.state::<PlayerState>(),
                        &app.state::<WatchStore>(),
                    );
                    commands::subtitles::persist_subtitle_prefs(
                        &app.state::<PlayerState>(),
                        &app.state::<SubtitlePrefsStore>(),
                    );
                }
            }
        })
        .expect("the transport ticker thread should start");
}

/// Load the watch-state store from the OS config dir, or a working-directory fallback when the
/// platform exposes no config directory (a headless/sandboxed environment) — matching how the
/// settings store degrades.
fn load_watch_store() -> WatchStore {
    match paths::config_dir() {
        Some(dir) => WatchStore::load_from(dir.join("watch_state.json")),
        None => {
            log::warn!("no OS config directory available — resume points will not persist");
            WatchStore::load_from(PathBuf::from("watch_state.json"))
        }
    }
}

/// Load the per-file subtitle-preferences store, degrading the same way as the watch store.
fn load_subtitle_prefs_store() -> SubtitlePrefsStore {
    match paths::config_dir() {
        Some(dir) => SubtitlePrefsStore::load_from(dir.join("subtitle_prefs.json")),
        None => {
            log::warn!("no OS config directory available — subtitle preferences will not persist");
            SubtitlePrefsStore::load_from(PathBuf::from("subtitle_prefs.json"))
        }
    }
}

/// Start OS media keys + now-playing, bound to the main window on Windows (SMTC needs its
/// handle). Degrades to a no-op handle if the controls cannot be created.
fn init_media_keys(app: &tauri::App) -> MediaKeys {
    let handle = app.handle().clone();

    #[cfg(windows)]
    {
        match app.get_webview_window("main").and_then(|w| w.hwnd().ok()) {
            Some(hwnd) => MediaKeys::spawn(handle, Some(hwnd.0 as isize)),
            None => {
                log::warn!("no window handle for media keys — OS media keys disabled");
                MediaKeys::disabled()
            }
        }
    }

    #[cfg(not(windows))]
    {
        MediaKeys::spawn(handle, None)
    }
}

/// The media target the app was launched with, if any — an OS "Open with", a shell
/// double-click, or a path typed on the command line.
///
/// Flags and their values are skipped so `--crash-notice 1234` never looks like a filename.
/// (Full single-instance enqueue-on-open is PLR-M21, in a later phase; this is the plain
/// open-on-launch case.)
fn media_from_args(args: &[String]) -> Option<&str> {
    let mut rest = args.iter().skip(1);
    while let Some(arg) = rest.next() {
        if arg == "--crash-notice" {
            // Consume its pid argument too.
            rest.next();
            continue;
        }
        if arg.starts_with('-') {
            continue;
        }
        return Some(arg.as_str());
    }
    None
}

/// Open whatever the app was launched with. A bad path is reported through the normal
/// playback-error path rather than blocking startup.
fn open_media_from_args(app: &tauri::AppHandle, args: &[String]) {
    let Some(target) = media_from_args(args) else {
        return;
    };
    let state = app.state::<PlayerState>();
    let watch = app.state::<WatchStore>();
    let subs = app.state::<SubtitlePrefsStore>();
    let settings = app.state::<SettingsStore>();
    match commands::playback::open_media(
        app.clone(),
        state.clone(),
        watch,
        subs,
        settings,
        target.to_owned(),
    ) {
        Ok(_) => {
            if let Err(err) = commands::playback::play(app.clone(), state) {
                log::warn!("could not start playback of {target}: {err}");
            }
        }
        Err(err) => log::error!("could not open {target}: {err}"),
    }
}

/// Create the native video surface under the webview.
///
/// This is the key architecture decision made concrete: mpv renders into a GPU surface
/// composited *below* the transparent webview, so decoded pixels never cross IPC. The window
/// handle is passed down as a plain integer — the player core never depends on Tauri, and
/// this crate never touches a raw pointer.
fn attach_video_surface(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let size = match window.inner_size() {
        Ok(size) => size,
        Err(e) => {
            log::warn!("could not read the window size for the video surface: {e}");
            return;
        }
    };

    #[cfg(windows)]
    match window.hwnd() {
        Ok(hwnd) => {
            app.state::<PlayerState>().attach_surface(
                freally_player_core::HostWindow::Win32(hwnd.0 as isize),
                size.width,
                size.height,
            );
        }
        Err(e) => log::error!("could not get the window handle for the video surface: {e}"),
    }

    // macOS/Linux hosts are not implemented yet; the engine reports that honestly the first
    // time media is opened rather than showing a silent black stage.
    #[cfg(not(windows))]
    {
        let _ = size;
    }
}

/// Store the main window's current geometry. Failures are logged, never fatal — losing a
/// remembered window size must not block shutdown.
fn save_window(window: &tauri::Window) {
    if window.label() != "main" {
        return;
    }
    let store = window.state::<SettingsStore>();

    let maximized = window.is_maximized().unwrap_or(false);
    // A maximized window's size is the screen, not the size to restore to — keep the
    // last known windowed geometry in that case.
    let size = if maximized {
        let stored = store.get().window;
        PhysicalSize::new(stored.width, stored.height)
    } else {
        match window.inner_size() {
            Ok(size) => size,
            Err(e) => {
                log::warn!("could not read window size: {e}");
                return;
            }
        }
    };

    if let Err(e) = store.set_window(WindowSettings {
        width: size.width,
        height: size.height,
        maximized,
    }) {
        log::warn!("could not save window geometry: {e}");
    }
}
