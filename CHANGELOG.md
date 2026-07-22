# Changelog

All notable changes to Freally Player are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Freally Player is © 2026 Mike Weaver — All Rights Reserved (proprietary, source-available; see
[`LICENSE`](LICENSE)). **Project started: June 30th, 2026.** **v1.0.0 released: ______** (fill on
release).

> **Status: in development.** `0.10.0` was tagged but **never published** — its installers had no
> playback engine, so the draft release was discarded rather than shipped. `0.10.1` is the first
> build that actually plays: it bundles libmpv. The release ladder runs
> **0.10.0 → 0.20.0 → 0.30.0 → 0.40.0 → 0.50.0 → 0.60.0 → 0.70.0 → 0.80.0 → 0.85.0 (library milestone — first public) → 0.95.0 → 1.0.0**,
> one tag per phase (see `product-roadmap.md`).

## [Unreleased] — 0.10.1

### Added
- **The installers bundle the playback engine, so a downloaded build plays.** On Windows
  `libmpv-2.dll` is installed next to `freally-player.exe`; the executable imports it at **load
  time**, which is why it must be a sibling of the binary and cannot be resolved at runtime from a
  resource path. Both the MSI and the NSIS installer carry it.
- A **written offer for the LGPL source** in [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md),
  naming the exact pinned libmpv build that ships and how to replace it. Bundling the engine makes
  that offer a live obligation rather than a statement of intent.

### Changed
- **`engine-libmpv` is now a default feature of the app crate** — a plain `cargo build` produces
  what ships, instead of an engine-less binary that looked fine until you opened a file. Building
  the app now requires libmpv (`node scripts/vendor-libmpv.mjs`, `brew install mpv`, or
  `apt install libmpv-dev`); the owned library crates keep their engines optional, so
  `cargo build -p freally-player-core` still needs no media libraries.

## [0.10.0] — 2026-07-21 — tagged, never published

### Added (Phase 0 — Foundation & repo)
- Tauri v2 app shell (Rust core + React + TypeScript + Vite UI), 1200×800 dark Havoc window, no
  console window on Windows release builds.
- **The native video surface — the key architecture decision, working.** libmpv decodes into a
  GPU context Freally Player creates and drives through mpv's **render API**, composited with the
  web UI, so decoded pixels never cross the IPC boundary. Behind the owned `Engine` interface in
  `freally-player-core`. **Windows only for now** — the macOS and Linux hosts are not implemented
  and report that plainly instead of showing a black stage.
- Playback command/event bus: `open_media` / `play` / `pause` / `seek` / `get_state` plus
  `player://state` and `player://media-opened`. The UI mirrors the transport from events and never
  polls; a ticker emits on change so the position keeps moving during playback.
- Cargo workspace (`src-tauri` app crate + `crates/` player, library, subtitles, streaming, convert),
  proprietary `LICENSE` + governance docs, `.gitignore` / `.gitattributes`, `rust-toolchain.toml`.
- `scripts/vendor-libmpv.mjs` — fetches a SHA-256-pinned libmpv and, on Windows, generates the MSVC
  import library the upstream package does not ship.
- **Havoc dark *and* light themes** — one set of CSS custom properties, switched at runtime and
  persisted in settings; dark stays the default.
- **First-run EULA acceptance gate** — `EULA.md` embedded at build time, scroll-to-read modal with
  **I Agree** / **Decline & Quit**, acceptance persisted and re-prompted only when `EULA_VERSION`
  changes.
- **Opt-in anonymous bug reporting** (per `HAVOC-STANDARD-bug-report-and-updater.md`) — a panic hook
  writes a scrubbed local crash report, a native error window offers a restart, and the relaunched
  app auto-surfaces the report with GitHub / Gmail / mail-client submission. Nothing auto-sends; no
  server, no shipped credentials.
- CI matrix on `windows-latest` / `macos-latest` / `ubuntu-latest`: UI lint/format/typecheck/test,
  Rust fmt/clippy/test/build, and an **engine job that proves libmpv links and initialises on all
  three OSes** and that the video-surface boundary behaves correctly per platform. Tag-triggered
  release workflow → draft GitHub Release.
- `docs/` GitHub Pages site seed (Havoc-branded landing + changelog + "Roadmap to 1.0.0"); download
  section reads **"Downloads available in future releases."**

### Changed
- `EULA.md`, `PRIVACY.md`, `SECURITY.md` and `THIRD-PARTY-NOTICES.md` no longer describe on-device
  speech-to-text auto-subtitles: the No-AI charter cut that feature, and the EULA is binding once
  accepted, so it must not promise capabilities the product does not ship. `EULA_VERSION` bumped to
  `2026-07-21`, which re-prompts anyone who accepted the earlier text.

### Known limitations
- **The installers ship no playback engine.** `engine-libmpv` was off by default and libmpv was not
  bundled as a packaged resource, so `0.10.0` opens, themes, and reports honestly that it has no
  engine — it does not play media. This is why the release was never published; fixed in `0.10.1`.
- **Video output is Windows-only.** The macOS and Linux surface hosts are not implemented; both
  report that plainly rather than showing a black stage.
- Video is composited *over* the web UI in the stage rect rather than under a transparent webview.

---

_Each shipped phase moves its highlights from the roadmap into a dated release section here and on
the [project site](https://mikesruthless12.github.io/freally-player/)._
