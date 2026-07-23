//! OS media keys + now-playing metadata, via `souvlaki`.
//!
//! The play/pause/seek keys on a keyboard and the system's now-playing panel (Windows SMTC,
//! macOS MediaRemote, Linux MPRIS) drive the same single [`PlayerState`] engine the UI does,
//! and reflect its transport back so the OS panel shows the right title and play state.
//!
//! # Why a dedicated thread
//!
//! The platform `MediaControls` handle is not uniformly `Send`/`Sync` across the three OSes, so
//! rather than store it in shared state we give it its own thread that **owns** it for the
//! app's lifetime. Everything else holds only a channel [`Sender`], which is always `Send` —
//! so nothing here constrains what can live in Tauri's managed state on any platform. The
//! thread receives transport updates to push to the OS panel; the OS's own button presses come
//! back through the `attach` callback and are forwarded to the engine.

use std::sync::mpsc::{self, Sender};
use std::time::Duration;

use freally_player_core::{MediaInfo, PlaybackState, Status};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};
use tauri::{AppHandle, Manager};

use crate::commands::playback::PlayerState;

/// How far a bare "seek forward/back" media key moves, when the OS does not say by how much.
const MEDIA_KEY_SEEK_SECS: f64 = 10.0;

/// A transport update pushed to the OS now-playing panel.
enum Update {
    NowPlaying {
        title: String,
        duration_secs: Option<f64>,
    },
    Playback {
        status: Status,
        position_secs: f64,
    },
}

/// A handle to the OS media controls. Holds only a channel, so it is trivially `Send`/`Sync`
/// and safe to keep in Tauri state on every platform.
pub struct MediaKeys {
    tx: Option<Sender<Update>>,
}

impl MediaKeys {
    /// A no-op handle for when the OS gives us no media controls — the app still runs, the
    /// system panel just stays empty.
    pub fn disabled() -> Self {
        Self { tx: None }
    }

    /// Start the media-controls thread. `hwnd` is required on Windows (SMTC binds to a window)
    /// and ignored elsewhere. Failure to create the controls is logged and degrades to
    /// [`MediaKeys::disabled`] rather than being fatal.
    pub fn spawn(app: AppHandle, hwnd: Option<isize>) -> Self {
        let (tx, rx) = mpsc::channel::<Update>();
        let spawned = std::thread::Builder::new()
            .name("freally-media-keys".to_owned())
            .spawn(move || run(app, hwnd, rx));

        match spawned {
            Ok(_) => Self { tx: Some(tx) },
            Err(e) => {
                log::warn!("could not start the media-keys thread: {e}");
                Self::disabled()
            }
        }
    }

    /// Tell the OS panel what is now playing.
    pub fn set_now_playing(&self, media: &MediaInfo) {
        self.send(Update::NowPlaying {
            title: media.title.clone(),
            duration_secs: media.duration_secs,
        });
    }

    /// Reflect the current transport (play/pause + position) to the OS panel.
    pub fn update_playback(&self, state: &PlaybackState) {
        self.send(Update::Playback {
            status: state.status,
            position_secs: state.position_secs,
        });
    }

    fn send(&self, update: Update) {
        if let Some(tx) = &self.tx {
            // A closed channel means the controls thread has gone; nothing to do but drop it.
            let _ = tx.send(update);
        }
    }
}

/// The controls thread: owns the OS handle, forwards its button presses to the engine, and
/// applies the transport updates the app sends.
fn run(app: AppHandle, hwnd: Option<isize>, rx: mpsc::Receiver<Update>) {
    let config = PlatformConfig {
        display_name: "Freally Player",
        dbus_name: "com.freally.player",
        hwnd: hwnd.map(|h| h as *mut std::ffi::c_void),
    };

    let mut controls = match MediaControls::new(config) {
        Ok(controls) => controls,
        Err(e) => {
            log::warn!("no OS media controls on this platform: {e:?}");
            return;
        }
    };

    let handler_app = app.clone();
    if let Err(e) = controls.attach(move |event| handle_event(&handler_app, event)) {
        log::warn!("could not attach OS media controls: {e:?}");
        return;
    }
    // Start from a known state, so a panel that appears before anything plays reads "stopped".
    let _ = controls.set_playback(MediaPlayback::Stopped);

    // Blocks until the app drops the sender at shutdown.
    while let Ok(update) = rx.recv() {
        match update {
            Update::NowPlaying {
                title,
                duration_secs,
            } => {
                let _ = controls.set_metadata(MediaMetadata {
                    title: Some(&title),
                    duration: duration_secs
                        .filter(|d| d.is_finite() && *d > 0.0)
                        .map(Duration::from_secs_f64),
                    ..Default::default()
                });
            }
            Update::Playback {
                status,
                position_secs,
            } => {
                let progress = Some(MediaPosition(Duration::from_secs_f64(
                    position_secs.max(0.0),
                )));
                let playback = match status {
                    Status::Playing => MediaPlayback::Playing { progress },
                    Status::Paused => MediaPlayback::Paused { progress },
                    Status::Idle => MediaPlayback::Stopped,
                };
                let _ = controls.set_playback(playback);
            }
        }
    }
}

/// Forward one OS media-control event to the engine, emitting the resulting state so the UI
/// mirrors a change the user made from outside the app.
fn handle_event(app: &AppHandle, event: MediaControlEvent) {
    use crate::commands::playback as pb;

    let player = app.state::<PlayerState>();
    let result: Result<(), String> = match event {
        MediaControlEvent::Play => pb::play(app.clone(), player.clone()),
        MediaControlEvent::Pause => pb::pause(app.clone(), player.clone()),
        MediaControlEvent::Toggle => pb::toggle_play(app.clone(), player.clone()),
        // Nothing to stop *to* without a playlist; treat Stop as pause rather than inventing
        // behaviour. Next/Previous wait for playlists in a later phase.
        MediaControlEvent::Stop => pb::pause(app.clone(), player.clone()),
        MediaControlEvent::Next | MediaControlEvent::Previous => return,
        MediaControlEvent::Seek(direction) => {
            seek_relative(app, &player, signed(direction, MEDIA_KEY_SEEK_SECS))
        }
        MediaControlEvent::SeekBy(direction, amount) => {
            seek_relative(app, &player, signed(direction, amount.as_secs_f64()))
        }
        MediaControlEvent::SetPosition(position) => {
            pb::seek(app.clone(), player.clone(), position.0.as_secs_f64())
        }
        _ => return,
    };
    if let Err(e) = result {
        log::debug!("media-key event had no effect: {e}");
    }
}

/// A positive amount for forward, negative for back.
fn signed(direction: SeekDirection, amount: f64) -> f64 {
    match direction {
        SeekDirection::Forward => amount,
        SeekDirection::Backward => -amount,
    }
}

/// Seek relative to the position last known, never below zero.
fn seek_relative(
    app: &AppHandle,
    player: &tauri::State<'_, PlayerState>,
    delta: f64,
) -> Result<(), String> {
    let target = (player.snapshot().position_secs + delta).max(0.0);
    crate::commands::playback::seek(app.clone(), player.clone(), target)
}
