# Changelog

All notable changes to Freally Player are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Freally Player is © 2026 Mike Weaver — All Rights Reserved (proprietary, source-available; see
[`LICENSE`](LICENSE)). **Project started: June 30th, 2026.** **v1.0.0 released: ______** (fill on
release).

> **Status: pre-development (planning).** Nothing is built yet — this is the plan. No releases exist;
> **downloads will be available in future releases.** The release ladder runs
> **0.10.0 → 0.20.0 → 0.30.0 → 0.40.0 → 0.50.0 → 0.60.0 → 0.70.0 → 0.80.0 → 0.85.0 (library milestone — first public) → 0.95.0 → 1.0.0**,
> one tag per phase (see `product-roadmap.md`).

## [Unreleased]

### Planned (Phase 0 — Foundation & repo → 0.10.0)
- Tauri v2 app shell (Rust core + React + TypeScript + Vite UI), 1200×800 dark Havoc window, no
  console window on Windows release builds.
- The **native video surface under the Tauri webview** wired up as an empty stage (the key
  architecture decision), with the player-core `Engine` interface and a libmpv render-API binding
  stub.
- Cargo workspace (`src-tauri` app crate + `crates/` player, library, subtitles, streaming, convert),
  proprietary `LICENSE` + governance docs, `.gitignore` / `.gitattributes`, `rust-toolchain.toml`.
- CI matrix on `windows-latest` / `macos-latest` / `ubuntu-latest`; tag-triggered release workflow →
  draft GitHub Release.
- `docs/` GitHub Pages site seed (Havoc-branded landing + changelog + "Roadmap to 1.0.0"); download
  section reads **"Downloads available in future releases."**

---

_No public releases yet. Each shipped phase will move its highlights from the roadmap into a dated
release section here and on the [project site](https://mikesruthless12.github.io/freally-player/)._
