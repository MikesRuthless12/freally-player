//! The native video surface — the key architecture decision.
//!
//! Decoded frames are drawn by a **native GPU surface composited underneath the transparent
//! webview**. The web layer paints chrome on top and never receives a pixel; the only thing
//! that crosses IPC is the transport snapshot in [`crate::PlaybackState`].
//!
//! mpv is driven through its **render API**: we create and own the GPU context, and mpv
//! renders into it on our schedule (`mpv_render_context_render`). That is deliberately more
//! work than handing mpv a window id and letting it drive itself — owning the context is what
//! makes Wayland, frame-level presentation control (display refresh-rate matching, PLR-M04)
//! and compositing overlays possible later.
//!
//! **This is the only place in the app that uses `unsafe`.** The platform modules talk to
//! Win32/WGL, and everything above this boundary is safe Rust.
//!
//! Platform status — stated plainly rather than failing silently:
//! * **Windows** — implemented (child HWND + WGL context under the WebView2).
//! * **macOS / Linux** — not implemented yet; [`attach`] returns an error saying so, and the
//!   UI shows it. Audio still plays; video has nowhere to go.

use crate::{EngineError, HostWindow};

#[cfg(all(windows, feature = "engine-libmpv"))]
mod windows;

/// A live native video surface. Dropping it tears the surface down.
pub struct VideoSurface {
    #[cfg(all(windows, feature = "engine-libmpv"))]
    inner: windows::WglSurface,
    /// Keeps the type inhabited (and the field used) on platforms without an implementation.
    #[cfg(not(all(windows, feature = "engine-libmpv")))]
    _unsupported: std::convert::Infallible,
}

impl VideoSurface {
    /// Place the surface at the stage rect, in physical pixels relative to the host window's
    /// client area.
    pub fn set_rect(&self, x: i32, y: i32, width: u32, height: u32) {
        #[cfg(all(windows, feature = "engine-libmpv"))]
        self.inner.set_rect(x, y, width, height);

        #[cfg(not(all(windows, feature = "engine-libmpv")))]
        {
            let _ = (x, y, width, height);
        }
    }
}

/// Attach a video surface for `mpv` inside `host`.
///
/// # Errors
/// Returns [`EngineError::Backend`] when the platform has no implementation yet, or when the
/// GPU context or mpv render context cannot be created.
#[cfg(all(windows, feature = "engine-libmpv"))]
pub fn attach(
    host: HostWindow,
    mpv: std::sync::Arc<libmpv2::Mpv>,
    width: u32,
    height: u32,
) -> Result<VideoSurface, EngineError> {
    let HostWindow::Win32(hwnd) = host;
    Ok(VideoSurface {
        inner: windows::WglSurface::attach(hwnd, mpv, width, height)?,
    })
}

#[cfg(all(not(windows), feature = "engine-libmpv"))]
pub fn attach(
    _host: HostWindow,
    _mpv: std::sync::Arc<libmpv2::Mpv>,
    _width: u32,
    _height: u32,
) -> Result<VideoSurface, EngineError> {
    Err(EngineError::Backend(
        "video output is not implemented on this platform yet — audio plays, but the picture \
         has nowhere to go. Windows is supported today; macOS and Linux are in progress."
            .to_owned(),
    ))
}

/// The platform boundary on hosts without a surface implementation.
///
/// This is the honesty invariant under test: a platform with no video output must say so in
/// words the user can act on, never fail silently or show a black stage. CI runs this on
/// macOS and Linux, so the day a real host lands there, this test is what has to change.
#[cfg(all(not(windows), feature = "engine-libmpv", test))]
mod tests {
    use super::*;

    #[test]
    fn attaching_reports_that_this_platform_has_no_surface_yet() {
        let mpv = std::sync::Arc::new(libmpv2::Mpv::new().expect("libmpv should initialise"));
        // Deliberately not `expect_err`: that needs `VideoSurface: Debug`, and a type holding
        // OS window handles has no business deriving Debug just to satisfy a test.
        let message = match attach(HostWindow::Win32(0), mpv, 1280, 720) {
            Ok(_) => panic!("this platform has no surface implementation, but attach succeeded"),
            Err(error) => error.to_string(),
        };
        assert!(
            message.contains("not implemented on this platform yet"),
            "must say plainly that video output is missing, got: {message}"
        );
        assert!(
            message.contains("audio plays"),
            "must say what still works, got: {message}"
        );
    }
}
