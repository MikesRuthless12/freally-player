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
    clamp_speed, clamp_sub_delay, clamp_sub_pos, clamp_sub_scale, clamp_volume,
    is_seekable_position, title_for, AbLoop, Chapter, Engine, EngineError, HostWindow, MediaInfo,
    OpenOptions, PlaybackState, Status, SubStyleOverride, SubtitleState, Track, TrackKind,
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

    /// A non-empty string property, or `None` when absent or blank.
    fn property_string(&self, name: &str) -> Option<String> {
        self.mpv
            .get_property::<String>(name)
            .ok()
            .filter(|s| !s.is_empty())
    }

    /// The audio/subtitle/video tracks mpv currently exposes, including externally added
    /// subtitles. Empty until the demuxer has read the header.
    ///
    /// mpv numbers tracks per kind, and that per-kind number is exactly the `aid`/`sid` a
    /// caller selects with — so it is what we report as the track id.
    fn tracks(&self) -> Vec<Track> {
        let count = self.property_i64("track-list/count").unwrap_or(0).max(0) as usize;
        (0..count)
            .filter_map(|i| {
                let kind = match self
                    .property_string(&format!("track-list/{i}/type"))?
                    .as_str()
                {
                    "audio" => TrackKind::Audio,
                    "sub" => TrackKind::Sub,
                    "video" => TrackKind::Video,
                    // Attachments (fonts, cover art) are tracks to mpv but not selectable
                    // streams — drop anything we do not offer a choice over.
                    _ => return None,
                };
                let id = self.property_i64(&format!("track-list/{i}/id"))?;
                Some(Track {
                    id,
                    kind,
                    lang: self.property_string(&format!("track-list/{i}/lang")),
                    title: self.property_string(&format!("track-list/{i}/title")),
                    default: self
                        .property_bool(&format!("track-list/{i}/default"))
                        .unwrap_or(false),
                    external: self
                        .property_bool(&format!("track-list/{i}/external"))
                        .unwrap_or(false),
                    image_based: matches!(kind, TrackKind::Sub)
                        && is_image_subtitle(
                            self.property_string(&format!("track-list/{i}/codec"))
                                .as_deref(),
                        ),
                })
            })
            .collect()
    }

    /// Select a per-kind track: a concrete id, or mpv's "no" to turn the kind off.
    fn select_track(&mut self, property: &str, id: Option<i64>) -> Result<(), EngineError> {
        if self.media.is_none() {
            return Err(EngineError::NothingOpen);
        }
        match id {
            Some(id) => self.mpv.set_property(property, id).map_err(backend_error),
            None => self.mpv.set_property(property, "no").map_err(backend_error),
        }
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
            tracks: self.tracks(),
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

    fn tracks(&self) -> Vec<Track> {
        if self.media.is_none() {
            return Vec::new();
        }
        MpvEngine::tracks(self)
    }

    fn set_audio_track(&mut self, id: Option<i64>) -> Result<(), EngineError> {
        self.select_track("aid", id)
    }

    fn set_sub_track(&mut self, id: Option<i64>) -> Result<(), EngineError> {
        self.select_track("sid", id)
    }

    fn set_secondary_sub_track(&mut self, id: Option<i64>) -> Result<(), EngineError> {
        self.select_track("secondary-sid", id)
    }

    fn add_sub_file(&mut self, path: &str, select: bool) -> Result<i64, EngineError> {
        if self.media.is_none() {
            return Err(EngineError::NothingOpen);
        }
        // "select" makes it the primary track now; "auto" adds it without switching. Either
        // way mpv appends it, so the new track is the highest sub id afterward.
        let flag = if select { "select" } else { "auto" };
        self.mpv
            .command("sub-add", &[path, flag])
            .map_err(backend_error)?;
        // Refresh the cached track list so `state()` reflects the new track without having to
        // rebuild the whole list every tick, and read the new sub id off that one read.
        let tracks = MpvEngine::tracks(self);
        let new_id = tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Sub)
            .map(|t| t.id)
            .max();
        if let Some(media) = self.media.as_mut() {
            media.tracks = tracks;
        }
        new_id.ok_or_else(|| {
            EngineError::Backend("the subtitle file loaded but no track appeared".to_owned())
        })
    }

    fn set_sub_visible(&mut self, visible: bool) -> Result<(), EngineError> {
        if self.media.is_none() {
            return Err(EngineError::NothingOpen);
        }
        self.mpv
            .set_property("sub-visibility", visible)
            .map_err(backend_error)
    }

    fn set_sub_delay(&mut self, secs: f64) -> Result<(), EngineError> {
        if self.media.is_none() {
            return Err(EngineError::NothingOpen);
        }
        self.mpv
            .set_property("sub-delay", clamp_sub_delay(secs))
            .map_err(backend_error)
    }

    fn set_sub_pos(&mut self, pos: i64) -> Result<(), EngineError> {
        if self.media.is_none() {
            return Err(EngineError::NothingOpen);
        }
        self.mpv
            .set_property("sub-pos", clamp_sub_pos(pos))
            .map_err(backend_error)
    }

    fn set_sub_scale(&mut self, scale: f64) -> Result<(), EngineError> {
        if self.media.is_none() {
            return Err(EngineError::NothingOpen);
        }
        self.mpv
            .set_property("sub-scale", clamp_sub_scale(scale))
            .map_err(backend_error)
    }

    fn set_sub_style_override(&mut self, style: &SubStyleOverride) -> Result<(), EngineError> {
        if self.media.is_none() {
            return Err(EngineError::NothingOpen);
        }
        // "force" makes mpv's own sub-font/size/color win over the file's ASS styling; "yes"
        // (mpv's default) respects the author. Turning the override off restores the default
        // look so a forced font never lingers — so both branches set the same four properties,
        // just with different values.
        let (mode, font, size, color) = if style.enabled {
            (
                "force",
                style.font.as_deref().unwrap_or(DEFAULT_SUB_FONT),
                style.font_size.unwrap_or(DEFAULT_SUB_FONT_SIZE),
                style.color.as_deref().unwrap_or(DEFAULT_SUB_COLOR),
            )
        } else {
            (
                "yes",
                DEFAULT_SUB_FONT,
                DEFAULT_SUB_FONT_SIZE,
                DEFAULT_SUB_COLOR,
            )
        };
        self.mpv
            .set_property("sub-ass-override", mode)
            .map_err(backend_error)?;
        self.mpv
            .set_property("sub-font", font)
            .map_err(backend_error)?;
        self.mpv
            .set_property("sub-font-size", size)
            .map_err(backend_error)?;
        self.mpv
            .set_property("sub-color", color)
            .map_err(backend_error)?;
        Ok(())
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
                // Duration, chapters and tracks usually only become known after the header is
                // parsed, so refresh anything still missing from the open() snapshot.
                if media.duration_secs.is_none() {
                    media.duration_secs = self.property_f64("duration");
                }
                if media.chapters.is_empty() {
                    media.chapters = self.chapters();
                }
                // Tracks are cached in `self.media` (set on open, refreshed by add_sub_file), so
                // rebuild the full list — several property reads per track — only when the count
                // changed, catching a track that appears once the header is fully parsed, rather
                // than on every ~4×/second tick.
                let live_count = self.property_i64("track-list/count").unwrap_or(0).max(0) as usize;
                if live_count != media.tracks.len() {
                    media.tracks = MpvEngine::tracks(self);
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
            // `aid`/`sid` read back as "no"/"auto" (not an i64) when not a concrete track, so a
            // disabled or not-yet-chosen track reads as None — exactly right for the menu.
            audio_id: self.property_i64("aid"),
            subtitle: SubtitleState {
                id: self.property_i64("sid"),
                secondary_id: self.property_i64("secondary-sid"),
                visible: self.property_bool("sub-visibility").unwrap_or(true),
                delay_secs: self.property_f64("sub-delay").unwrap_or(0.0),
                pos: self
                    .property_i64("sub-pos")
                    .unwrap_or(crate::SUB_POS_DEFAULT),
                scale: self.property_f64("sub-scale").unwrap_or(1.0),
            },
        }
    }
}

fn backend_error(err: libmpv2::Error) -> EngineError {
    EngineError::Backend(err.to_string())
}

/// mpv's default subtitle look, restored when the style override is turned off so a forced
/// font/size/colour never lingers.
const DEFAULT_SUB_FONT: &str = "sans-serif";
const DEFAULT_SUB_FONT_SIZE: f64 = 55.0;
const DEFAULT_SUB_COLOR: &str = "#FFFFFFFF";

/// Is this subtitle codec image-based (a bitmap) rather than text? Style, font and colour
/// overrides only apply to text subtitles, so the UI uses this to say so honestly rather than
/// offering controls that would do nothing.
fn is_image_subtitle(codec: Option<&str>) -> bool {
    matches!(
        codec,
        Some(
            "hdmv_pgs_subtitle"
                | "pgssub"
                | "dvd_subtitle"
                | "dvdsub"
                | "dvb_subtitle"
                | "dvbsub"
                | "dvb_teletext"
                | "xsub"
        )
    )
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
        // The Phase 2 track/subtitle commands refuse the same way with nothing open.
        assert_eq!(
            engine.set_audio_track(Some(1)),
            Err(EngineError::NothingOpen)
        );
        assert_eq!(engine.set_sub_track(Some(1)), Err(EngineError::NothingOpen));
        assert_eq!(
            engine.set_secondary_sub_track(Some(2)),
            Err(EngineError::NothingOpen)
        );
        assert_eq!(
            engine.add_sub_file("subs.srt", true),
            Err(EngineError::NothingOpen)
        );
        assert_eq!(engine.set_sub_visible(false), Err(EngineError::NothingOpen));
        assert_eq!(engine.set_sub_delay(1.0), Err(EngineError::NothingOpen));
        assert_eq!(engine.set_sub_pos(90), Err(EngineError::NothingOpen));
        assert_eq!(engine.set_sub_scale(1.5), Err(EngineError::NothingOpen));
        assert_eq!(
            engine.set_sub_style_override(&SubStyleOverride::default()),
            Err(EngineError::NothingOpen)
        );
        // With nothing open there are no tracks to report.
        assert!(Engine::tracks(&engine).is_empty());
    }

    #[test]
    fn image_based_subtitle_codecs_are_recognised() {
        // Bitmap subtitle formats — style/font/colour overrides do not apply to these.
        assert!(is_image_subtitle(Some("hdmv_pgs_subtitle")));
        assert!(is_image_subtitle(Some("dvd_subtitle")));
        assert!(is_image_subtitle(Some("dvb_subtitle")));
        // Text subtitles — overridable.
        assert!(!is_image_subtitle(Some("subrip")));
        assert!(!is_image_subtitle(Some("ass")));
        assert!(!is_image_subtitle(Some("webvtt")));
        assert!(!is_image_subtitle(None));
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
