//! Windows video surface: a sibling child `HWND` of the WebView2, carrying a WGL OpenGL
//! context that mpv's render API draws into.
//!
//! # Why a child window, and the clipping flags it needs
//!
//! WebView2 lives in its own child `HWND` covering the parent's client area. Ours is a
//! sibling of it, sized to the stage rect the UI reports, so decoded pixels reach the screen
//! without ever crossing IPC.
//!
//! **Overlapping sibling child windows only layer correctly when the parent sets
//! `WS_CLIPCHILDREN` and every child sets `WS_CLIPSIBLINGS`.** Tauri's window has neither by
//! default, and without them WebView2 repaints straight over the video window *regardless of
//! Z-order* — the video renders perfectly and is simply painted away. [`enable_sibling_clipping`]
//! sets both. This was measured, not guessed: with the flags missing, `glReadPixels` returned
//! opaque video while `GetPixel` on the screen at the same point returned the chrome's
//! background colour.
//!
//! # Threading
//!
//! An OpenGL context belongs to whichever thread made it current, and mpv may only render on
//! that thread. So the window is created by the caller's thread (it just needs to belong to a
//! thread with a message pump — the parent's), while a dedicated render thread owns the DC,
//! the GL context and the mpv render context for their whole lifetime.
//!
//! mpv's update callback fires on an arbitrary mpv thread; it only bumps a flag and wakes the
//! render thread, which does the actual work. The render thread also wakes on a timeout so a
//! resize repaints even when no new frame is due.

#![allow(unsafe_code)]

use std::ffi::CString;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};

use libmpv2::render::{OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType};
use libmpv2::Mpv;
use windows_sys::Win32::Foundation::{HMODULE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{GetDC, GetPixel, ReleaseDC, HDC};
use windows_sys::Win32::Graphics::OpenGL::{
    wglCreateContext, wglDeleteContext, wglGetProcAddress, wglMakeCurrent, ChoosePixelFormat,
    SetPixelFormat, SwapBuffers, HGLRC, PFD_DOUBLEBUFFER, PFD_DRAW_TO_WINDOW, PFD_MAIN_PLANE,
    PFD_SUPPORT_OPENGL, PFD_TYPE_RGBA, PIXELFORMATDESCRIPTOR,
};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryA};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClassNameW, GetParent, GetWindow,
    GetWindowLongW, GetWindowRect, IsWindowVisible, RegisterClassW, SetWindowLongW, SetWindowPos,
    CS_OWNDC, GWL_EXSTYLE, GWL_STYLE, GW_CHILD, GW_HWNDNEXT, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, WNDCLASSW, WS_CHILD, WS_CLIPSIBLINGS, WS_VISIBLE,
};

use crate::EngineError;

/// Window class name for the video child window (UTF-16, NUL-terminated).
const CLASS_NAME: &[u16] = &[
    b'F' as u16,
    b'r' as u16,
    b'e' as u16,
    b'a' as u16,
    b'l' as u16,
    b'l' as u16,
    b'y' as u16,
    b'V' as u16,
    b'i' as u16,
    b'd' as u16,
    b'e' as u16,
    b'o' as u16,
    0,
];

/// How long the render thread sleeps before re-checking when mpv has no new frame. Short
/// enough that a resize repaints promptly, long enough to stay idle.
const IDLE_TICK: std::time::Duration = std::time::Duration::from_millis(50);

/// Shared between the render thread and its owner.
struct Shared {
    /// Set by mpv's update callback (any thread) and by resizes.
    redraw: Mutex<bool>,
    wake: Condvar,
    /// Client size in physical pixels, packed as `(width << 32) | height` so both halves are
    /// read atomically together — a torn read would render one frame at a mismatched size.
    size: AtomicU64,
    running: AtomicBool,
}

impl Shared {
    fn request_redraw(&self) {
        *self.redraw.lock().unwrap_or_else(|e| e.into_inner()) = true;
        self.wake.notify_one();
    }

    fn set_size(&self, width: u32, height: u32) {
        self.size.store(
            (u64::from(width) << 32) | u64::from(height),
            Ordering::Relaxed,
        );
        self.request_redraw();
    }

    fn size(&self) -> (i32, i32) {
        let packed = self.size.load(Ordering::Relaxed);
        // Clamp to at least 1×1: mpv rejects a zero-sized framebuffer, and a minimised
        // window legitimately reports 0.
        (
            ((packed >> 32) as u32).max(1) as i32,
            ((packed & 0xFFFF_FFFF) as u32).max(1) as i32,
        )
    }
}

/// A native video surface backed by a child window and a WGL context.
pub struct WglSurface {
    hwnd: isize,
    shared: Arc<Shared>,
    render_thread: Option<std::thread::JoinHandle<()>>,
}

impl WglSurface {
    pub fn attach(
        parent: isize,
        mpv: Arc<Mpv>,
        width: u32,
        height: u32,
    ) -> Result<Self, EngineError> {
        let hwnd = create_child_window(parent, width, height)?;

        let shared = Arc::new(Shared {
            redraw: Mutex::new(true),
            wake: Condvar::new(),
            size: AtomicU64::new((u64::from(width) << 32) | u64::from(height)),
            running: AtomicBool::new(true),
        });

        // The render thread reports its setup result back so `attach` can fail loudly rather
        // than leave a window that never draws.
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();
        let thread_shared = Arc::clone(&shared);
        let thread_hwnd = SendHwnd(hwnd);

        let render_thread = std::thread::Builder::new()
            .name("freally-video-surface".to_owned())
            .spawn(move || render_loop(thread_hwnd, mpv, thread_shared, ready_tx))
            .map_err(|err| {
                EngineError::Backend(format!("could not start the render thread: {err}"))
            })?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                hwnd,
                shared,
                render_thread: Some(render_thread),
            }),
            Ok(Err(reason)) => {
                unsafe { DestroyWindow(hwnd as HWND) };
                Err(EngineError::Backend(reason))
            }
            Err(_) => {
                unsafe { DestroyWindow(hwnd as HWND) };
                Err(EngineError::Backend(
                    "the video render thread stopped before it finished starting".to_owned(),
                ))
            }
        }
    }

    /// Place the surface at `(x, y)` with the given size, in physical pixels relative to the
    /// host window's client area.
    ///
    /// The video window sits **above** the webview (see the module docs), so it must occupy
    /// exactly the stage rect the UI reports — anything larger would cover the chrome.
    pub fn set_rect(&self, x: i32, y: i32, width: u32, height: u32) {
        // SAFETY: `hwnd` is a window we created and have not destroyed (we only destroy it in
        // `Drop`, which takes `&mut self`).
        unsafe {
            SetWindowPos(
                self.hwnd as HWND,
                HWND_TOP,
                x,
                y,
                width.max(1) as i32,
                height.max(1) as i32,
                SWP_NOACTIVATE,
            );
        }
        self.shared.set_size(width, height);
    }
}

impl Drop for WglSurface {
    fn drop(&mut self) {
        self.shared.running.store(false, Ordering::SeqCst);
        self.shared.request_redraw();
        if let Some(handle) = self.render_thread.take() {
            // The render thread owns the GL and mpv render contexts; it must finish tearing
            // them down before the window goes away.
            let _ = handle.join();
        }
        // SAFETY: created by us, destroyed exactly once here.
        unsafe { DestroyWindow(self.hwnd as HWND) };
    }
}

/// An `HWND` moved to the render thread.
///
/// `HWND` is a raw pointer and therefore not `Send`, but a window handle is just an opaque
/// process-wide identifier — using one from another thread is explicitly supported by Win32.
/// The window's *messages* stay on its owning thread, and we never pump messages here.
struct SendHwnd(isize);
// SAFETY: see above — an HWND is a process-wide identifier, not a thread-affine pointer.
unsafe impl Send for SendHwnd {}

/// Append a line to the `FREALLY_VIDEO_DEBUG` log next to the OS temp dir. Diagnostic only —
/// nothing in normal operation writes here.
fn write_debug(line: &str) {
    use std::io::Write;
    let path = std::env::temp_dir().join("freally-video-debug.log");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{line}");
    }
}

/// Make overlapping sibling child windows layer correctly.
///
/// Two flags are required for this and neither is set by default here:
/// * the parent needs `WS_CLIPCHILDREN`, or it paints over its children's areas;
/// * every child needs `WS_CLIPSIBLINGS`, or it paints over siblings regardless of Z-order.
///
/// Without them the WebView2 sibling repaints straight over the video window even though the
/// video window is above it — which is exactly the "chrome background instead of picture"
/// symptom this fixes.
fn enable_sibling_clipping(parent: HWND, video: HWND) {
    const WS_CLIPCHILDREN: u32 = 0x0200_0000;
    // SWP flags that force the frame to be recalculated after a style change.
    const SWP_FRAMECHANGED: u32 = 0x0020;
    const SWP_NOZORDER: u32 = 0x0004;

    // SAFETY: both handles are live windows; these are plain style reads/writes followed by
    // the documented SetWindowPos call that applies a style change.
    unsafe {
        if !parent.is_null() {
            let style = GetWindowLongW(parent, GWL_STYLE) as u32;
            if style & WS_CLIPCHILDREN == 0 {
                SetWindowLongW(parent, GWL_STYLE, (style | WS_CLIPCHILDREN) as i32);
                SetWindowPos(
                    parent,
                    std::ptr::null_mut(),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                );
            }

            // Every sibling, not just ours — the WebView2 window is the one that overpaints.
            let mut sibling = GetWindow(parent, GW_CHILD);
            while !sibling.is_null() {
                if sibling != video {
                    let style = GetWindowLongW(sibling, GWL_STYLE) as u32;
                    if style & WS_CLIPSIBLINGS == 0 {
                        SetWindowLongW(sibling, GWL_STYLE, (style | WS_CLIPSIBLINGS) as i32);
                    }
                }
                sibling = GetWindow(sibling, GW_HWNDNEXT);
            }
        }
    }
}

/// Read a window's class name.
fn class_name_of(hwnd: HWND) -> String {
    let mut buffer = [0u16; 128];
    // SAFETY: `buffer` is a valid writable array of the length passed.
    let len = unsafe { GetClassNameW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
    if len <= 0 {
        return "<unknown>".to_owned();
    }
    String::from_utf16_lossy(&buffer[..len as usize])
}

fn rect_of(hwnd: HWND) -> RECT {
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: `hwnd` is live and `rect` is a valid out-parameter.
    unsafe { GetWindowRect(hwnd, &mut rect) };
    rect
}

/// Dump everything that decides whether our rendered pixels reach the screen.
///
/// The decisive line is `SCREEN vs FRAMEBUFFER`: `glReadPixels` says what we drew, and
/// `GetPixel` on the screen DC says what the compositor actually shows at that same physical
/// point. If those disagree, we are being composited away rather than failing to draw — and
/// the sibling z-order and style dump below says by whom.
fn dump_composition_state(hwnd: HWND, framebuffer_sample: Option<(usize, [u8; 4])>) {
    // SAFETY: every call takes a live window handle or a DC we release; all are plain reads.
    unsafe {
        let our_rect = rect_of(hwnd);
        write_debug(&format!(
            "-- video window: hwnd={hwnd:?} visible={} rect=({},{})-({},{}) style=0x{:X} exstyle=0x{:X}",
            IsWindowVisible(hwnd) != 0,
            our_rect.left,
            our_rect.top,
            our_rect.right,
            our_rect.bottom,
            GetWindowLongW(hwnd, GWL_STYLE) as u32,
            GetWindowLongW(hwnd, GWL_EXSTYLE) as u32,
        ));

        let parent = GetParent(hwnd);
        if !parent.is_null() {
            let parent_rect = rect_of(parent);
            const WS_CLIPCHILDREN: u32 = 0x0200_0000;
            let parent_style = GetWindowLongW(parent, GWL_STYLE) as u32;
            write_debug(&format!(
                "-- parent: rect=({},{})-({},{}) style=0x{parent_style:X} exstyle=0x{:X} WS_CLIPCHILDREN={}",
                parent_rect.left,
                parent_rect.top,
                parent_rect.right,
                parent_rect.bottom,
                GetWindowLongW(parent, GWL_EXSTYLE) as u32,
                parent_style & WS_CLIPCHILDREN != 0,
            ));

            // Child z-order, topmost first. Whoever sits above us is what covers the video.
            // GW_CHILD gives the parent's FIRST CHILD; GW_HWNDFIRST would give the first
            // window in the parent's own z-order band (i.e. top-level windows) instead.
            let mut sibling = GetWindow(parent, GW_CHILD);
            let mut index = 0;
            while !sibling.is_null() && index < 16 {
                let sibling_style = GetWindowLongW(sibling, GWL_STYLE) as u32;
                write_debug(&format!(
                    "   z[{index}] {:<28} visible={} style=0x{sibling_style:X} CLIPSIBLINGS={} exstyle=0x{:X}{}",
                    class_name_of(sibling),
                    IsWindowVisible(sibling) != 0,
                    sibling_style & WS_CLIPSIBLINGS != 0,
                    GetWindowLongW(sibling, GWL_EXSTYLE) as u32,
                    if sibling == hwnd { "   <-- OUR VIDEO WINDOW" } else { "" },
                ));
                sibling = GetWindow(sibling, GW_HWNDNEXT);
                index += 1;
            }
        }

        // What is actually on screen where we just drew?
        let centre_x = (our_rect.left + our_rect.right) / 2;
        let centre_y = (our_rect.top + our_rect.bottom) / 2;
        let screen_dc = GetDC(std::ptr::null_mut());
        if !screen_dc.is_null() {
            let colour = GetPixel(screen_dc, centre_x, centre_y);
            ReleaseDC(std::ptr::null_mut(), screen_dc);
            // COLORREF is 0x00BBGGRR.
            let (r, g, b) = (
                (colour & 0xFF) as u8,
                ((colour >> 8) & 0xFF) as u8,
                ((colour >> 16) & 0xFF) as u8,
            );
            match framebuffer_sample {
                Some((distinct, first)) => write_debug(&format!(
                    "== SCREEN at ({centre_x},{centre_y}) = RGB({r},{g},{b})  vs  FRAMEBUFFER \
                     first_rgba={first:?} distinct={distinct}"
                )),
                None => write_debug(&format!(
                    "== SCREEN at ({centre_x},{centre_y}) = RGB({r},{g},{b})"
                )),
            }
        }
    }
}

/// Register the window class once per process.
fn window_class() -> Result<(), EngineError> {
    static REGISTERED: OnceLock<Result<(), String>> = OnceLock::new();
    REGISTERED
        .get_or_init(|| {
            // SAFETY: a zeroed WNDCLASSW with the fields below set is the documented way to
            // register a class; all pointers are either null or valid for the call.
            unsafe {
                let instance: HMODULE = GetModuleHandleW(std::ptr::null());
                let class = WNDCLASSW {
                    // CS_OWNDC: the DC we hand OpenGL stays valid for the window's lifetime.
                    style: CS_OWNDC,
                    lpfnWndProc: Some(video_wnd_proc),
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: instance,
                    hIcon: std::ptr::null_mut(),
                    hCursor: std::ptr::null_mut(),
                    hbrBackground: std::ptr::null_mut(),
                    lpszMenuName: std::ptr::null(),
                    lpszClassName: CLASS_NAME.as_ptr(),
                };
                if RegisterClassW(&class) == 0 {
                    let code = windows_sys::Win32::Foundation::GetLastError();
                    // 1410 = ERROR_CLASS_ALREADY_EXISTS, which is success for our purposes.
                    if code != 1410 {
                        return Err(format!(
                            "could not register the video window class ({code})"
                        ));
                    }
                }
                Ok(())
            }
        })
        .clone()
        .map_err(EngineError::Backend)
}

/// The child window draws nothing itself — OpenGL owns every pixel — so it does no background
/// erase and defers everything else.
unsafe extern "system" fn video_wnd_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    const WM_ERASEBKGND: u32 = 0x0014;
    if message == WM_ERASEBKGND {
        // Claim the erase so Windows never flashes a background over the video.
        return 1;
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn create_child_window(parent: isize, width: u32, height: u32) -> Result<isize, EngineError> {
    window_class()?;

    // SAFETY: the class is registered, `parent` is the caller's window, and every pointer is
    // null or points at our NUL-terminated class name.
    let hwnd = unsafe {
        let instance: HMODULE = GetModuleHandleW(std::ptr::null());
        CreateWindowExW(
            0,
            CLASS_NAME.as_ptr(),
            std::ptr::null(),
            // WS_CLIPSIBLINGS keeps us from painting over the webview sibling.
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
            0,
            0,
            width.max(1) as i32,
            height.max(1) as i32,
            parent as HWND,
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        )
    };

    if hwnd.is_null() {
        // SAFETY: plain FFI read of the calling thread's last error.
        let code = unsafe { windows_sys::Win32::Foundation::GetLastError() };
        return Err(EngineError::Backend(format!(
            "could not create the video window ({code})"
        )));
    }

    enable_sibling_clipping(parent as HWND, hwnd);

    // Sit ABOVE the WebView2 sibling.
    //
    // The obvious arrangement — video underneath a transparent webview — does not work on
    // Windows: WebView2 composites through DirectComposition, so its transparent pixels
    // reveal the desktop rather than a sibling window below it. Measured directly: with the
    // video window at HWND_BOTTOM, `glReadPixels` returned opaque video while `GetPixel` on
    // the screen at the same point returned the desktop behind the app. Placing it on top and
    // sizing it to the stage rect is what actually composites.
    //
    // SAFETY: `hwnd` was just created successfully.
    unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOP,
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
        );
    }

    Ok(hwnd as isize)
}

/// Resolves GL entry points for mpv.
struct GlLoader {
    opengl32: HMODULE,
}

// SAFETY: an HMODULE is a process-wide module handle, valid on any thread.
unsafe impl Send for GlLoader {}
unsafe impl Sync for GlLoader {}

/// `wglGetProcAddress` only resolves extension functions, and returns these sentinel values
/// (not just null) on failure for the core GL 1.1 entry points — which must be looked up in
/// `opengl32.dll` instead. Missing this is the classic cause of a black video window.
fn is_invalid_proc(address: isize) -> bool {
    matches!(address, -1..=3)
}

fn get_proc_address(loader: &GlLoader, name: &str) -> *mut std::ffi::c_void {
    let Ok(symbol) = CString::new(name) else {
        return std::ptr::null_mut();
    };
    // SAFETY: `symbol` is a valid NUL-terminated C string that outlives both calls, and
    // `loader.opengl32` is a live module handle.
    unsafe {
        if let Some(proc) = wglGetProcAddress(symbol.as_ptr() as *const u8) {
            let address = proc as usize as isize;
            if !is_invalid_proc(address) {
                return address as *mut std::ffi::c_void;
            }
        }
        match GetProcAddress(loader.opengl32, symbol.as_ptr() as *const u8) {
            Some(proc) => proc as *mut std::ffi::c_void,
            None => std::ptr::null_mut(),
        }
    }
}

/// GL 1.1 entry points we call ourselves, resolved from `opengl32.dll`.
///
/// Only the handful needed to force the framebuffer opaque — mpv resolves everything else it
/// needs through its own `get_proc_address`.
struct GlFns {
    clear_color: unsafe extern "system" fn(f32, f32, f32, f32),
    clear: unsafe extern "system" fn(u32),
    color_mask: unsafe extern "system" fn(u8, u8, u8, u8),
    read_pixels: unsafe extern "system" fn(i32, i32, i32, i32, u32, u32, *mut u8),
}

const GL_COLOR_BUFFER_BIT: u32 = 0x0000_4000;
const GL_RGBA: u32 = 0x1908;
const GL_UNSIGNED_BYTE: u32 = 0x1401;

impl GlFns {
    fn load(loader: &GlLoader) -> Option<Self> {
        // SAFETY: transmuting a resolved GL entry point to its documented signature. Each is
        // core GL 1.1 and therefore exported directly from opengl32.dll.
        unsafe {
            let clear_color = get_proc_address(loader, "glClearColor");
            let clear = get_proc_address(loader, "glClear");
            let color_mask = get_proc_address(loader, "glColorMask");
            let read_pixels = get_proc_address(loader, "glReadPixels");
            if clear_color.is_null()
                || clear.is_null()
                || color_mask.is_null()
                || read_pixels.is_null()
            {
                return None;
            }
            Some(Self {
                clear_color: std::mem::transmute::<
                    *mut std::ffi::c_void,
                    unsafe extern "system" fn(f32, f32, f32, f32),
                >(clear_color),
                clear: std::mem::transmute::<*mut std::ffi::c_void, unsafe extern "system" fn(u32)>(
                    clear,
                ),
                color_mask: std::mem::transmute::<
                    *mut std::ffi::c_void,
                    unsafe extern "system" fn(u8, u8, u8, u8),
                >(color_mask),
                read_pixels: std::mem::transmute::<
                    *mut std::ffi::c_void,
                    unsafe extern "system" fn(i32, i32, i32, i32, u32, u32, *mut u8),
                >(read_pixels),
            })
        }
    }

    /// Read a small block back out of the framebuffer and report how many distinct pixel
    /// values it contains. Used only by the `FREALLY_VIDEO_DEBUG` diagnostic: it answers
    /// "did mpv actually draw into OUR GL surface?" independently of how the window manager
    /// composites that surface afterwards.
    fn sample_framebuffer(&self, width: i32, height: i32) -> (usize, [u8; 4]) {
        const BLOCK: i32 = 32;
        let x = (width / 2 - BLOCK / 2).max(0);
        let y = (height / 2 - BLOCK / 2).max(0);
        let mut buffer = vec![0u8; (BLOCK * BLOCK * 4) as usize];
        // SAFETY: the context is current and the buffer is exactly BLOCK*BLOCK*4 bytes, the
        // size glReadPixels writes for a BLOCK×BLOCK RGBA/UNSIGNED_BYTE read.
        unsafe {
            (self.read_pixels)(
                x,
                y,
                BLOCK,
                BLOCK,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                buffer.as_mut_ptr(),
            );
        }
        let mut seen = std::collections::HashSet::new();
        for pixel in buffer.chunks_exact(4) {
            seen.insert([pixel[0], pixel[1], pixel[2], pixel[3]]);
        }
        let first = [buffer[0], buffer[1], buffer[2], buffer[3]];
        (seen.len(), first)
    }

    /// Force every pixel's alpha to 1 without touching colour.
    ///
    /// The window is DWM-composited with **per-pixel alpha** (Tauri's `transparent: true`
    /// enables blur-behind rather than `WS_EX_LAYERED`). mpv renders opaque video but leaves
    /// the framebuffer's alpha channel at zero, so DWM composites the picture away entirely
    /// and the user sees straight through to the desktop. Masking colour off and clearing
    /// only alpha fixes that without disturbing the frame mpv just drew.
    fn force_opaque_alpha(&self) {
        // SAFETY: the GL context is current on this thread and these are the real entry
        // points; the colour mask is restored before returning.
        unsafe {
            (self.color_mask)(0, 0, 0, 1);
            (self.clear_color)(0.0, 0.0, 0.0, 1.0);
            (self.clear)(GL_COLOR_BUFFER_BIT);
            (self.color_mask)(1, 1, 1, 1);
        }
    }
}

/// Owns the DC + GL context for the render thread and releases them in the right order.
struct GlContext {
    hwnd: HWND,
    hdc: HDC,
    hglrc: HGLRC,
}

impl GlContext {
    fn create(hwnd: HWND) -> Result<Self, String> {
        // SAFETY: `hwnd` is our live child window; the descriptor is fully initialised below
        // and every call is checked before the next one uses its result.
        unsafe {
            let hdc = GetDC(hwnd);
            if hdc.is_null() {
                return Err("could not get a device context for the video window".to_owned());
            }

            let mut descriptor: PIXELFORMATDESCRIPTOR = std::mem::zeroed();
            descriptor.nSize = std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16;
            descriptor.nVersion = 1;
            descriptor.dwFlags = PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER;
            descriptor.iPixelType = PFD_TYPE_RGBA;
            // 24-bit colour with NO alpha plane. The window is DWM-composited with per-pixel
            // alpha (Tauri's `transparent: true`), so an alpha channel here would let the
            // video be composited away — see `force_opaque_alpha`.
            descriptor.cColorBits = 24;
            descriptor.cAlphaBits = 0;
            descriptor.cDepthBits = 24;
            descriptor.cStencilBits = 8;
            descriptor.iLayerType = PFD_MAIN_PLANE as u8;

            let format = ChoosePixelFormat(hdc, &descriptor);
            if format == 0 {
                ReleaseDC(hwnd, hdc);
                return Err("no suitable OpenGL pixel format is available".to_owned());
            }
            if SetPixelFormat(hdc, format, &descriptor) == 0 {
                ReleaseDC(hwnd, hdc);
                return Err("could not set the OpenGL pixel format".to_owned());
            }

            let hglrc = wglCreateContext(hdc);
            if hglrc.is_null() {
                ReleaseDC(hwnd, hdc);
                return Err("could not create an OpenGL context".to_owned());
            }
            if wglMakeCurrent(hdc, hglrc) == 0 {
                wglDeleteContext(hglrc);
                ReleaseDC(hwnd, hdc);
                return Err("could not make the OpenGL context current".to_owned());
            }

            Ok(Self { hwnd, hdc, hglrc })
        }
    }

    /// Present the back buffer. Returns whether the swap succeeded — a failing `SwapBuffers`
    /// is silent otherwise, and would look exactly like "the video never appeared".
    fn swap(&self) -> bool {
        // SAFETY: `hdc` is live and its context is current on this thread.
        unsafe { SwapBuffers(self.hdc) != 0 }
    }
}

impl Drop for GlContext {
    fn drop(&mut self) {
        // SAFETY: unwind in the reverse order of creation, exactly once.
        unsafe {
            wglMakeCurrent(std::ptr::null_mut(), std::ptr::null_mut());
            wglDeleteContext(self.hglrc);
            ReleaseDC(self.hwnd, self.hdc);
        }
    }
}

/// The render thread: owns the GL context and mpv's render context, and draws frames.
fn render_loop(
    hwnd: SendHwnd,
    mpv: Arc<Mpv>,
    shared: Arc<Shared>,
    ready: std::sync::mpsc::Sender<Result<(), String>>,
) {
    let hwnd = hwnd.0 as HWND;

    let gl = match GlContext::create(hwnd) {
        Ok(gl) => gl,
        Err(reason) => {
            let _ = ready.send(Err(reason));
            return;
        }
    };

    // SAFETY: `opengl32.dll` is already loaded (we just made a WGL context); this bumps its
    // refcount so the handle stays valid for the loader's lifetime.
    let opengl32 = unsafe { LoadLibraryA(c"opengl32.dll".as_ptr() as *const u8) };
    if opengl32.is_null() {
        let _ = ready.send(Err("could not load opengl32.dll".to_owned()));
        return;
    }

    // SAFETY: `Mpv::ctx` is a live handle kept alive by the `Arc` we hold for the whole loop.
    // The binding asks for `&mut mpv_handle`, but `mpv_handle` is an opaque C type whose own
    // locking makes it thread-safe (libmpv2 marks `Mpv` both `Send` and `Sync` for exactly
    // this reason), so the reference is never used to mutate Rust-visible state and no Rust
    // aliasing guarantee is relied upon. mpv also permits creating the render context off the
    // core's thread provided rendering then stays on one thread — which it does, this one.
    let render_context = unsafe {
        RenderContext::new(
            &mut *mpv.ctx.as_ptr(),
            vec![
                RenderParam::ApiType(RenderParamApiType::OpenGl),
                RenderParam::InitParams(OpenGLInitParams {
                    get_proc_address,
                    ctx: GlLoader { opengl32 },
                }),
            ],
        )
    };

    let mut render_context = match render_context {
        Ok(context) => context,
        Err(err) => {
            let _ = ready.send(Err(format!("mpv could not create a render context: {err}")));
            return;
        }
    };

    let gl_fns = GlFns::load(&GlLoader { opengl32 });
    if gl_fns.is_none() {
        log::warn!("could not resolve glClear/glColorMask — video may composite as transparent");
    }

    // Wake this thread whenever mpv has a new frame ready.
    let callback_shared = Arc::clone(&shared);
    render_context.set_update_callback(move || {
        callback_shared.request_redraw();
    });

    let _ = ready.send(Ok(()));

    let debug_diagnostics = std::env::var_os("FREALLY_VIDEO_DEBUG").is_some();
    let mut frame: u64 = 0;
    if debug_diagnostics {
        write_debug("render thread started; GL context and mpv render context created OK");
    }

    while shared.running.load(Ordering::SeqCst) {
        {
            let mut redraw = shared.redraw.lock().unwrap_or_else(|e| e.into_inner());
            // Wait at most one tick for mpv to signal a new frame. Deliberately not a
            // `while` loop: on timeout we fall through and repaint anyway, so a resize is
            // picked up even while paused, when mpv sends no updates at all.
            if !*redraw && shared.running.load(Ordering::SeqCst) {
                let (guard, _timeout) = shared
                    .wake
                    .wait_timeout(redraw, IDLE_TICK)
                    .unwrap_or_else(|e| e.into_inner());
                redraw = guard;
            }
            *redraw = false;
        }

        if !shared.running.load(Ordering::SeqCst) {
            break;
        }

        let (width, height) = shared.size();
        // FBO 0 is the window's back buffer. `flip = true` because OpenGL's origin is bottom
        // left while video rows run top-down.
        if let Err(err) = render_context.render::<GlLoader>(0, width, height, true) {
            log::warn!("mpv render failed: {err}");
            continue;
        }
        if let Some(fns) = &gl_fns {
            fns.force_opaque_alpha();
        }

        // FREALLY_VIDEO_DEBUG: prove whether mpv drew into our surface, independently of how
        // the compositor treats it afterwards. Sampled once, a second or so in.
        frame += 1;
        let diagnose = debug_diagnostics && (frame == 40 || frame % 200 == 0);
        let sample = if diagnose {
            gl_fns
                .as_ref()
                .map(|fns| fns.sample_framebuffer(width, height))
        } else {
            None
        };

        let swapped = gl.swap();

        if diagnose {
            if let Some((distinct, first)) = sample {
                write_debug(&format!(
                    "frames={frame} fbo={width}x{height} swap_ok={swapped} \
                     distinct_pixels_in_32x32={distinct} first_rgba={first:?}"
                ));
            }
            dump_composition_state(hwnd, sample);
        }
    }

    // `render_context` and `gl` drop here, on the thread that created them — required, and
    // the reason teardown is not done by the owner.
}

/// The number of GL entry points mpv resolves is large and driver-specific; these tests cover
/// the pure logic that surrounds the FFI rather than the FFI itself, which the app-level
/// smoke test exercises for real.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wgl_sentinel_return_values_are_treated_as_failures() {
        // The whole point: 1/2/3/-1 are "not found", not addresses. Treating them as
        // addresses is the classic cause of a crash or a black window.
        for sentinel in [0, 1, 2, 3, -1] {
            assert!(is_invalid_proc(sentinel), "{sentinel} must be rejected");
        }
        assert!(!is_invalid_proc(0x7FF6_1234_5678));
    }

    #[test]
    fn size_packing_round_trips_and_never_yields_zero() {
        let shared = Shared {
            redraw: Mutex::new(false),
            wake: Condvar::new(),
            size: AtomicU64::new(0),
            running: AtomicBool::new(true),
        };

        shared.set_size(1920, 1080);
        assert_eq!(shared.size(), (1920, 1080));

        // A minimised window reports 0×0; mpv rejects a zero-sized framebuffer.
        shared.set_size(0, 0);
        assert_eq!(shared.size(), (1, 1));
    }

    #[test]
    fn a_redraw_request_is_observable() {
        let shared = Shared {
            redraw: Mutex::new(false),
            wake: Condvar::new(),
            size: AtomicU64::new(0),
            running: AtomicBool::new(true),
        };
        shared.request_redraw();
        assert!(*shared.redraw.lock().unwrap());
    }
}
