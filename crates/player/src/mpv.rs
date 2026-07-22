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
    is_seekable_position, title_for, Engine, EngineError, HostWindow, MediaInfo, PlaybackState,
    Status,
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
}

impl Engine for MpvEngine {
    fn open(&mut self, path: &str) -> Result<MediaInfo, EngineError> {
        self.mpv
            .command("loadfile", &[path, "replace"])
            .map_err(backend_error)?;

        // `duration` is not known until the demuxer has read the header; report what we have
        // and let the transport events fill it in rather than blocking the open call.
        let media = MediaInfo {
            path: path.to_owned(),
            title: title_for(path),
            duration_secs: self.property_f64("duration"),
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

    fn set_surface_rect(&self, x: i32, y: i32, width: u32, height: u32) {
        if let Some(surface) = &self.surface {
            surface.set_rect(x, y, width, height);
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
                // Duration usually only becomes known after the header is parsed.
                if media.duration_secs.is_none() {
                    media.duration_secs = self.property_f64("duration");
                }
                media
            }),
        }
    }
}

fn backend_error(err: libmpv2::Error) -> EngineError {
    EngineError::Backend(err.to_string())
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
