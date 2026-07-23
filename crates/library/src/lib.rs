//! `freally-library` — owned media library: folder scan, opt-in scraping, watch-state, schema.
//!
//! The library proper (folder scan, scraping, the SQLite schema) lands in Phase 7 (see
//! `product-roadmap.md`). Opt-in network (TMDB/TVDB/MusicBrainz) is the only outbound use here,
//! and only when the user enables it.
//!
//! What exists today is **watch-state** (Phase 1): where each file was left and which tracks
//! were playing, so a file reopens where it stopped. It is a plain JSON map keyed by path —
//! the same shape and durability discipline as the settings store — rather than the eventual
//! SQLite home, which the real library will bring.

#![forbid(unsafe_code)]

mod watch;

pub use watch::{RecentWatch, WatchState, WatchStore};
