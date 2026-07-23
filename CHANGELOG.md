# Changelog

All notable changes to Freally Player are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Freally Player is © 2026 Mike Weaver — All Rights Reserved (proprietary, source-available; see
[`LICENSE`](LICENSE)). **Project started: June 30th, 2026.** **v1.0.0 released: ______** (fill on
release).

> **Status: in development.** `0.10.0` and `0.10.1` were tagged but **never published** — the
> first had no playback engine and the second was held back, so neither draft shipped. **`0.20.0`
> is the first published release**: it bundles libmpv, plays, and carries the full playback UI and
> the 18-language interface. The release ladder runs
> **0.10.0 → 0.20.0 → 0.30.0 → 0.40.0 → 0.50.0 → 0.60.0 → 0.70.0 → 0.80.0 → 0.85.0 (library milestone — first public) → 0.95.0 → 1.0.0**,
> one tag per phase (see `product-roadmap.md`).

## [Unreleased]

## [0.30.0] — 2026-07-23

### Added — Phase 2: subtitles
- **Subtitle & audio track switching** — a subtitle menu and an audio menu in the control bar,
  each listing the media's tracks (a track's own title, else its language spelled out in the UI
  language, else a numbered fallback) with the current one marked. Switching is instant; the
  engine changes `sid`/`aid` and the transport snapshot reflects it. Tracks are read from the
  engine's track list, which grows when an external subtitle is added
  ([`crates/player`](crates/player), [`commands/subtitles.rs`](src-tauri/src/commands/subtitles.rs)).
- **External subtitle load with encoding auto-detect** — open an SRT/ASS/SSA/WebVTT/MicroDVD/PGS/
  VobSub file. Text subtitles are treated as **untrusted input**: the read is bounded, the charset
  is auto-detected (`chardetng`) and transcoded to UTF-8 (`encoding_rs`) before libass ever sees
  it, so a legacy Windows-1251 or Shift-JIS file renders as its real letters rather than mojibake.
  Image-based tracks (PGS/VobSub) are bounded and passed through by path. The UI names what
  happened honestly ("Loaded, converted from windows-1251") ([`crates/subtitles`](crates/subtitles)).
- **Sync, position & scale, remembered per file** — nudge the subtitle delay to match an off SRT,
  move the line up off a hardcoded caption, or resize it; the adjustment is remembered for *that
  file* (a JSON map with the same size+mtime identity check as the resume store) and reapplied
  next time. Rendering stays the engine's job — libass draws into the video surface; the web layer
  never paints a subtitle over the picture.
- **Secondary subtitles** — show two subtitle tracks at once (e.g. for language learning), the
  second independent of the first.
- **A subtitle style override (accessibility)** — force a font, size, and colour over the file's
  own ASS styling, for readability. Off by default, and honest that it overrides the author's
  intent; applies to text subtitles only.
- **Opt-in OpenSubtitles fetch** — search by title and language and download a subtitle over TLS,
  **off unless you turn it on** and supply your own free API key. Only the title and languages you
  search leave the machine; your account password is never stored (it is exchanged for a
  short-lived session token). Downloaded files go through the same bounded, untrusted loader as
  local ones.

## [0.20.0] — 2026-07-23

### Added — Phase 1: core playback UI
- **A glassy control bar under the picture** — play/pause, frame-step, ±10s skip, a live clock,
  volume with a mute toggle, a playback-speed menu (0.25×–4.0×), A–B repeat, a chapters menu, a
  snapshot button, and fullscreen. It **auto-hides during playback** so the picture fills the
  window, and reappears on the first mouse move. It sits in its own strip *below* the frame rather
  than floating over it: on Windows the native video surface composites *above* the webview, so
  the web layer can only paint chrome where the picture does not — measured, not assumed
  ([`crates/player/src/surface/windows.rs`](crates/player/src/surface/windows.rs)).
- **A scrubber** with click/drag seek, a buffered-range bar, chapter tick marks, the A–B loop
  region shaded in, and a **timecode that follows the cursor** on hover. Pinned left-to-right in
  every locale, because a timeline is a temporal axis — mirroring it in Arabic would put 0:00 on
  the right and invert every drag.
- **Resume-from-position + last-used tracks.** A file reopens where it was left, with the audio
  and subtitle tracks that were playing — saved on open, close, and periodically while playing.
  A file that has **changed under its path** (size or modification time) ignores its stale point
  rather than dropping you into the wrong place; a file watched to the end forgets its point.
  Stored in [`crates/library`](crates/library) as a JSON map next to the settings file.
- **A keyboard-first shortcut layer** — Space/K play-pause, ←/→ and J/L seek, ↑/↓ volume, M mute,
  F fullscreen, `,`/`.` frame-step, `[`/`]` speed, 0–9 jump to a percentage. Inert while a text
  field, a menu, or a modal has the keyboard.
- **OS media keys + a now-playing panel** via `souvlaki` (Windows SMTC, macOS MediaRemote, Linux
  MPRIS): the media keys drive the same engine, and the panel shows the current title and play
  state.
- **Snapshot export** — save the current frame (with the subtitle overlay) to a PNG through a
  native save dialog.
- **An idle screen** — a drop zone (drop a file anywhere to open it), an Open button, and a
  **Continue-Watching row** of the files you last left partway through, each with a progress bar.

### Added — the 18-language UI (landed after Phase 0, first shipping in 0.20.0)
- **The UI is translated into all 18 shipped languages** (`ar de en es fr hi id it ja ko nl pl
  pt-BR ru tr uk vi zh-CN`), with a **Language pane in Settings** that switches instantly — no
  restart, no reload, no request. Every user-visible string now goes through a Fluent catalog in
  [`ui/src/i18n/`](ui/src/i18n); all 18 catalogs are bundled at build time, which is what makes
  the switch instant and offline. Pulled forward from Phase 10 (`P10.3`) on purpose: the Phase 0
  UI has few strings, so translating now is cheap, and DoD 6c keeps every later phase current
  instead of letting ten phases of translation debt pile up.
- **First-run locale detection.** With nothing stored, the app takes the OS's preferred language;
  `pt-PT` lands on `pt-BR` and `zh-TW` on `zh-CN` rather than dropping to English, because a
  language the user reads beats one they may not.
- **Right-to-left for Arabic.** Selecting it mirrors the whole shell — sidebar, title-bar
  controls, transport, footer — via CSS logical properties and `dir` on the document root.
- **`npm run i18n:lint`, and a CI job step that runs it** (DoD 6c(a)). It fails on a key present
  in one catalog and missing from another, a `t()` key the source asks for and no catalog has, a
  translation that drops or invents a `{ $placeholder }`, a locale with no catalog file, and a
  catalog Fluent cannot parse.
- **A language smoke suite** (`ui/e2e/languages.spec.ts`) that drives the real Settings modal,
  switches into each of the 18 languages in turn, asserts the chrome actually changed into it —
  against strings read from the catalogs, not copied into the test — and screenshots each one, on
  all three OSes.
- A **`language` field in `settings.json`**, absent until the user picks one. A settings file
  written before this existed still loads, with every other preference intact.

### Changed
- The **language picker lists English first, then the other 17 alphabetically by their own name**,
  in that same order in every language. The order is a baked-in literal, not a runtime sort:
  collation is locale-sensitive, so sorting live would rearrange the picker under the user each
  time they switched — Arabic lifts العربية to the top, Russian lifts Cyrillic, Chinese lifts the
  CJK names. Across the 18 locales that produces five different orders.
- The Appearance pane's theme buttons now read **Dark**/**Light**. They were lowercase strings
  capitalised by CSS, so the visible label and the screen-reader label disagreed.

### Notes
- **The agreement text and the bug report stay in English, deliberately.** The EULA is the
  document that legally binds, so a translated copy would not be the one that applies. The bug
  report preview is headed "exactly what will be sent" and mirrors the English body Rust builds
  for whoever reads it — localising the preview would make that heading untrue. Both are marked
  `dir="ltr"` so the Arabic shell does not lay English text out right-to-left.

## [0.10.1] — 2026-07-22

### Added
- **The installers bundle the playback engine, so a downloaded build plays.** On Windows
  `libmpv-2.dll` is installed next to `freally-player.exe`; the executable imports it at **load
  time**, which is why it must be a sibling of the binary and cannot be resolved at runtime from a
  resource path. Both the MSI and the NSIS installer carry it.
- A **written offer for the LGPL source** in [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md),
  naming the exact pinned libmpv build that ships and how to replace it. Bundling the engine makes
  that offer a live obligation rather than a statement of intent.
- **macOS bundles libmpv's whole dependency chain into the `.app`.** Unlike Windows, where the
  library is self-contained, the Homebrew libmpv records absolute `/opt/homebrew` paths that do not
  exist on a user's Mac; `scripts/bundle-macos-dylibs.sh` rewrites the graph into the bundle and
  fails the build if any absolute path survives.
- **Linux `.deb`/`.rpm` declare a libmpv dependency** rather than vendoring one, so apt/dnf install
  it alongside the app — the convention on those platforms.
- **Noto is bundled, so text renders in every shipped language on every OS** — Latin, Greek,
  Cyrillic, Vietnamese, Arabic, Devanagari and all four CJK families (Simplified and Traditional
  Chinese, Japanese, Korean). Windows and macOS carry CJK system fonts but many Linux installs do
  not, where CJK was previously tofu boxes. The variable builds keep the whole set to ~19 MB, and
  `unicode-range` slicing means a Latin-only UI never loads a CJK byte. Traditional Chinese ships
  even though it is not one of the 18 UI locales, because filenames and subtitle tracks are in
  whatever script the *media* uses.
- **A font smoke suite that checks what was actually rasterised** (`ui/e2e/fonts.spec.ts`), via the
  DevTools Protocol rather than trusting `font-family`, and requiring the face to be the bundled
  one rather than a system lookalike. It runs on all three OSes and uploads one screenshot per
  language per platform. It immediately caught Traditional Chinese rendering in *Simplified*
  letterforms — `:lang(zh)` matches `zh-TW` by BCP-47 prefix, so the cascade order had silently
  inverted.

### Changed (packaging)
- **macOS ships two DMGs — Apple Silicon and Intel — instead of one universal binary.** Homebrew
  provides only host-architecture libmpv, so a universal build's x86_64 slice cannot link against
  an arm64 dylib. (`macos-15-intel` is the last x86_64 image GitHub Actions offers; the
  architecture goes away around Aug 2027.)

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
