//! The libmpv backend — the non-owned decode/render engine behind the owned [`Engine`]
//! boundary.
//!
//! mpv is configured with `vo=libmpv`, its **render API** video output. That is the whole
//! point of the architecture: mpv hands decoded frames to a render context we own and draw
//! into a native GPU surface composited *under* the webview, instead of opening a window of
//! its own. Decoded pixels therefore never cross IPC.
//!
//! Until the per-OS surface hosts attach a render context, mpv decodes and plays audio but
//! has nowhere to put video — transport is fully live, video output is not. That is stated
//! plainly rather than papered over; the honesty invariant forbids a silent black screen.
//!
//! Hardware decode is requested as `auto-safe`, which uses the platform decoder
//! (D3D11VA/DXVA2, VideoToolbox, VA-API/VDPAU) when the driver is known-good and falls back
//! to software otherwise — a fallback the diagnostics surface reports rather than hides.

use std::sync::Arc;

use libmpv2::Mpv;

use crate::surface::{self, VideoSurface};
use crate::{
    clamp_speed, clamp_volume, is_seekable_position, title_for, AbLoop, Chapter, Engine,
    EngineError, HostWindow, MediaInfo, OpenOptions, PlaybackState, Status,
};

/// Playback driven by libmpv.
pub struct MpvEngine {
    /// Shared with the surface's render thread, which needs the same core.
    mpv: Arc<Mpv>,
    media: Option<MediaInfo>,
    /// The native video surface, once a window exists to host it.
    surface: Option<VideoSurface>,
}

impl MpvEngine {
    /// Create and initialise mpv.
    pub fn new() -> Result<Self, EngineError> {
        let mpv = Mpv::with_initializer(|init| {
            // The render API: mpv renders into a context we own, never its own window.
            init.set_property("vo", "libmpv")?;
            // Platform hardware decode where the driver is trustworthy, software otherwise.
            init.set_property("hwdec", "auto-safe")?;
            // This is a player, not a shell: never let a file's own config or scripts run.
            init.set_property("config", false)?;
            init.set_property("load-scripts", false)?;
            init.set_property("ytdl", false)?;
            // Keep the core alive at end-of-file so the transport can still be queried.
            init.set_property("idle", "yes")?;
            Ok(())
        })
        .map_err(backend_error)?;

        Ok(Self {
            mpv: Arc::new(mpv),
            media: None,
            surface: None,
        })
    }

    fn property_f64(&self, name: &str) -> Option<f64> {
        self.mpv.get_property::<f64>(name).ok()
    }

    fn property_bool(&self, name: &str) -> Option<bool> {
        self.mpv.get_property::<bool>(name).ok()
    }

    fn property_i64(&self, name: &str) -> Option<i64> {
        self.mpv.get_property::<i64>(name).ok()
    }

    /// The chapter markers mpv currently knows about. Empty until the demuxer has parsed them,
    /// and for media with none.
    fn chapters(&self) -> Vec<Chapter> {
        let count = self.property_i64("chapter-list/count").unwrap_or(0).max(0) as usize;
        (0..count)
            .map(|i| Chapter {
                // An unnamed chapter has no title property — show it by number in the UI.
                title: self
                    .mpv
                    .get_property::<String>(&format!("chapter-list/{i}/title"))
                    .ok()
                    .filter(|t| !t.is_empty()),
                start_secs: self
                    .property_f64(&format!("chapter-list/{i}/time"))
                    .unwrap_or(0.0),
            })
            .collect()
    }
}

impl Engine for MpvEngine {
    fn open(&mut self, path: &str, options: &OpenOptions) -> Result<MediaInfo, EngineError> {
        // Apply resume position + track selection as loadfile options, so they take effect
        // atomically with the load rather than as a seek/property-set that races it. Passed as
        // a comma-separated `key=value` list, mpv's fourth loadfile argument.
        let per_file = open_options(options);
        let load_args: Vec<&str> = match &per_file {
            Some(opts) => vec![path, "replace", "0", opts],
            None => vec![path, "replace"],
        };
        self.mpv
            .command("loadfile", &load_args)
            .map_err(backend_error)?;

        // `duration` and the chapter list are not known until the demuxer has read the header;
        // report what we have and let the transport events fill the rest in rather than
        // blocking the open call.
        let media = MediaInfo {
            path: path.to_owned(),
            title: title_for(path),
            duration_secs: self.property_f64("duration"),
            chapters: self.chapters(),
        };
        self.media = Some(media.clone());
        Ok(media)
    }

    fn play(&mut self) -> Result<(), EngineError> {
        if self.media.is_none() {
            return Err(EngineError::NothingOpen);
        }
        self.mpv.set_property("pause", false).map_err(backend_error)
    }

    fn pause(&mut self) -> Result<(), EngineError> {
        if self.media.is_none() {
            return Err(EngineError::NothingOpen);
        }
        self.mpv.set_property("pause", true).map_err(backend_error)
    }

    fn seek(&mut self, position_secs: f64) -> Result<(), EngineError> {
        if self.media.is_none() {
            return Err(EngineError::NothingOpen);
        }
        if !is_seekable_position(position_secs) {
            return Err(EngineError::InvalidSeek);
        }
        self.mpv
            .command("seek", &[&position_secs.to_string(), "absolute"])
            .map_err(backend_error)
    }

    fn set_volume(&mut self, volume: f64) -> Result<(), EngineError> {
        if self.media.is_none() {
            return Err(EngineError::NothingOpen);
        }
        self.mpv
            .set_property("volume", clamp_volume(volume))
            .map_err(backend_error)
    }

    fn set_muted(&mut self, muted: bool) -> Result<(), EngineError> {
        if self.media.is_none() {
            return Err(EngineError::NothingOpen);
        }
        self.mpv.set_property("mute", muted).map_err(backend_error)
    }

    fn set_speed(&mut self, speed: f64) -> Result<(), EngineError> {
        if self.media.is_none() {
            return Err(EngineError::NothingOpen);
        }
        self.mpv
            .set_property("speed", clamp_speed(speed))
            .map_err(backend_error)
    }

    fn frame_step(&mut self, forward: bool) -> Result<(), EngineError> {
        if self.media.is_none() {
            return Err(EngineError::NothingOpen);
        }
        let command = if forward {
            "frame-step"
        } else {
            "frame-back-step"
        };
        self.mpv.command(command, &[]).map_err(backend_error)
    }

    fn set_ab_loop(&mut self, a: Option<f64>, b: Option<f64>) -> Result<(), EngineError> {
        if self.media.is_none() {
            return Err(EngineError::NothingOpen);
        }
        // mpv reads "no" to clear an A–B endpoint and a number to set it.
        set_ab_endpoint(&self.mpv, "ab-loop-a", a)?;
        set_ab_endpoint(&self.mpv, "ab-loop-b", b)
    }

    fn set_chapter(&mut self, index: usize) -> Result<(), EngineError> {
        if self.media.is_none() {
            return Err(EngineError::NothingOpen);
        }
        self.mpv
            .set_property("chapter", index as i64)
            .map_err(backend_error)
    }

    fn current_tracks(&self) -> (Option<i64>, Option<i64>) {
        // `aid`/`sid` read as "auto"/"no" when not a concrete track, which is not an i64 —
        // so a non-numeric selection reads back as None, exactly what we want to persist.
        (self.property_i64("aid"), self.property_i64("sid"))
    }

    fn capture_frame(&mut self, path: &str, with_subs: bool) -> Result<(), EngineError> {
        if self.media.is_none() {
            return Err(EngineError::NothingOpen);
        }
        // "subtitles" bakes the subtitle overlay in; "video" is the clean decoded frame.
        let flag = if with_subs { "subtitles" } else { "video" };
        self.mpv
            .command("screenshot-to-file", &[path, flag])
            .map_err(backend_error)
    }

    fn attach_surface(
        &mut self,
        host: HostWindow,
        width: u32,
        height: u32,
    ) -> Result<(), EngineError> {
        if self.surface.is_some() {
            return Ok(());
        }
        self.surface = Some(surface::attach(host, Arc::clone(&self.mpv), width, height)?);
        Ok(())
    }

    fn set_surface_rect(&self, x: i32, y: i32, width: u32, height: u32, visible: bool) {
        if let Some(surface) = &self.surface {
            surface.set_rect(x, y, width, height, visible);
        }
    }

    fn state(&self) -> PlaybackState {
        let status = match &self.media {
            None => Status::Idle,
            // `pause` is the authority: mpv may pause itself (buffering, end of file).
            Some(_) => match self.property_bool("pause") {
                Some(true) => Status::Paused,
                _ => Status::Playing,
            },
        };

        PlaybackState {
            status,
            position_secs: self.property_f64("time-pos").unwrap_or(0.0),
            media: self.media.clone().map(|mut media| {
                // Duration and chapters usually only become known after the header is parsed,
                // so refresh anything still missing from the open() snapshot.
                if media.duration_secs.is_none() {
                    media.duration_secs = self.property_f64("duration");
                }
                if media.chapters.is_empty() {
                    media.chapters = self.chapters();
                }
                media
            }),
            volume: self.property_f64("volume").unwrap_or(100.0),
            muted: self.property_bool("mute").unwrap_or(false),
            speed: self.property_f64("speed").unwrap_or(1.0),
            buffered_secs: self.property_f64("demuxer-cache-time").unwrap_or(0.0),
            ab_loop: AbLoop {
                a: self.property_f64("ab-loop-a"),
                b: self.property_f64("ab-loop-b"),
            },
        }
    }
}

fn backend_error(err: libmpv2::Error) -> EngineError {
    EngineError::Backend(err.to_string())
}

/// Build mpv's per-file loadfile options string from resume/track choices, or `None` when
/// there is nothing to set (the common first-open case).
fn open_options(options: &OpenOptions) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(start) = options.start_secs.filter(|s| s.is_finite() && *s > 0.0) {
        parts.push(format!("start={start}"));
    }
    if let Some(aid) = options.audio_id {
        parts.push(format!("aid={aid}"));
    }
    if let Some(sid) = options.sub_id {
        parts.push(format!("sid={sid}"));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(","))
    }
}

/// Set one A–B loop endpoint: a number to arm it, mpv's "no" to clear it.
fn set_ab_endpoint(mpv: &Mpv, property: &str, value: Option<f64>) -> Result<(), EngineError> {
    match value {
        Some(secs) => mpv.set_property(property, secs).map_err(backend_error),
        None => mpv.set_property(property, "no").map_err(backend_error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Proves the vendored libmpv actually links and initialises — the whole architecture
    /// rests on this. It does not open any media, so it needs no fixture and no audio device.
    #[test]
    fn libmpv_links_and_initialises() {
        let engine = MpvEngine::new().expect("libmpv should initialise");
        assert_eq!(engine.state().status, Status::Idle);
        assert!(engine.state().media.is_none());
    }

    #[test]
    fn transport_commands_refuse_when_nothing_is_open() {
        let mut engine = MpvEngine::new().expect("libmpv should initialise");
        assert_eq!(engine.play(), Err(EngineError::NothingOpen));
        assert_eq!(engine.pause(), Err(EngineError::NothingOpen));
        assert_eq!(engine.seek(5.0), Err(EngineError::NothingOpen));
        // The Phase 1 transport extras refuse the same way, rather than quietly touching a
        // core with nothing loaded.
        assert_eq!(engine.set_volume(50.0), Err(EngineError::NothingOpen));
        assert_eq!(engine.set_muted(true), Err(EngineError::NothingOpen));
        assert_eq!(engine.set_speed(2.0), Err(EngineError::NothingOpen));
        assert_eq!(engine.frame_step(true), Err(EngineError::NothingOpen));
        assert_eq!(
            engine.set_ab_loop(Some(1.0), None),
            Err(EngineError::NothingOpen)
        );
        assert_eq!(engine.set_chapter(0), Err(EngineError::NothingOpen));
        assert_eq!(
            engine.capture_frame("shot.png", false),
            Err(EngineError::NothingOpen)
        );
    }

    #[test]
    fn loadfile_options_are_only_emitted_when_something_is_set() {
        assert_eq!(open_options(&OpenOptions::default()), None);
        assert_eq!(
            open_options(&OpenOptions {
                start_secs: Some(90.0),
                audio_id: Some(2),
                sub_id: Some(1),
            }),
            Some("start=90,aid=2,sid=1".to_owned())
        );
        // A zero/negative/non-finite start is not a resume point and is dropped.
        assert_eq!(
            open_options(&OpenOptions {
                start_secs: Some(0.0),
                audio_id: None,
                sub_id: None,
            }),
            None
        );
    }

    /// Volume, speed, mute and A–B all read back from the idle core at sensible resting
    /// values, so the UI never opens on a muted 0% or a stalled 0×.
    #[test]
    fn an_idle_engine_reports_resting_transport_values() {
        let engine = MpvEngine::new().expect("libmpv should initialise");
        let state = engine.state();
        assert_eq!(state.volume, 100.0);
        assert!(!state.muted);
        assert_eq!(state.speed, 1.0);
        assert_eq!(state.ab_loop, AbLoop::default());
    }

    #[test]
    fn the_engine_reports_a_usable_version() {
        // A sanity check that we are talking to a real library, not a stub.
        let engine = MpvEngine::new().expect("libmpv should initialise");
        let version: String = engine
            .mpv
            .get_property("mpv-version")
            .expect("mpv-version is always readable");
        assert!(version.starts_with("mpv"), "unexpected version: {version}");
    }
}
