//! The audited Tauri command surface. The web UI has no direct network or filesystem
//! access — every I/O path the UI can reach is a command in here.

pub mod playback;
