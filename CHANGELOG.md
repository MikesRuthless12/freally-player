# Changelog

All notable changes to Freally Player are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Freally Player is © 2026 Mike Weaver — All Rights Reserved (proprietary, source-available; see
[`LICENSE`](LICENSE)). **Project started: June 30th, 2026.** **v1.0.0 released: ______** (fill on
release).

> **Status: in development.** Phase 0 is built and green on Windows/macOS/Linux CI; no releases exist yet;
> **downloads will be available in future releases.** The release ladder runs
> **0.10.0 → 0.20.0 → 0.30.0 → 0.40.0 → 0.50.0 → 0.60.0 → 0.70.0 → 0.80.0 → 0.85.0 (library milestone — first public) → 0.95.0 → 1.0.0**,
> one tag per phase (see `product-roadmap.md`).

## [Unreleased]

### Added (Phase 0 — Foundation & repo → 0.10.0, unreleased)
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

---

_No public releases yet. Each shipped phase will move its highlights from the roadmap into a dated
release section here and on the [project site](https://mikesruthless12.github.io/freally-player/)._
