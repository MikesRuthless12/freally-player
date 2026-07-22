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
mod paths;
mod settings;

use serde::Serialize;
use tauri::{Manager, PhysicalSize, State, WindowEvent};

use commands::playback::PlayerState;
use settings::{SettingsStore, Theme, WindowSettings};

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

/// The persisted UI colour scheme.
#[tauri::command]
fn theme_get(store: State<'_, SettingsStore>) -> Theme {
    store.get().theme
}

/// Persist the UI colour scheme.
#[tauri::command]
fn theme_set(store: State<'_, SettingsStore>, theme: Theme) -> Result<(), String> {
    store
        .set_theme(theme)
        .map_err(|err| format!("could not save the theme: {err}"))
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
        .invoke_handler(tauri::generate_handler![
            app_info,
            theme_get,
            theme_set,
            eula::eula_status,
            eula::eula_accept,
            commands::playback::open_media,
            commands::playback::play,
            commands::playback::pause,
            commands::playback::seek,
            commands::playback::get_state,
            commands::playback::set_video_rect,
            bugreport::bug_report_context,
            bugreport::bug_report_submit,
            bugreport::bug_report_clear_crash,
            bugreport::open_external,
        ])
        .setup(move |app| {
            restore_window(app.handle());
            attach_video_surface(app.handle());
            spawn_transport_ticker(app.handle());
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

    let app = app.clone();
    std::thread::Builder::new()
        .name("freally-transport-ticker".to_owned())
        .spawn(move || {
            let mut last: Option<freally_player_core::PlaybackState> = None;
            loop {
                std::thread::sleep(TICK);
                let state = app.state::<PlayerState>().snapshot();
                if last.as_ref() != Some(&state) {
                    events::emit_state(&app, &state);
                    last = Some(state);
                }
            }
        })
        .expect("the transport ticker thread should start");
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
    match commands::playback::open_media(app.clone(), state.clone(), target.to_owned()) {
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
