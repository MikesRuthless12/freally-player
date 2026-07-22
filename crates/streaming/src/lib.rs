//! `freally-streaming` — owned streaming/casting layer: network streams, cast/DLNA, and the
//! yt-dlp driver.
//!
//! Stub crate: streaming lands in Phase 5 and casting in Phase 9 (see `product-roadmap.md`).
//! yt-dlp is never linked — it is driven as a separate subprocess. Casting is LAN-only and
//! stays off until the user starts it.

#![forbid(unsafe_code)]
