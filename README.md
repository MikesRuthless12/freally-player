# Freally Player

A **local-first**, **cross-platform** media player for **Windows, macOS, and Linux** — by
[Havoc Software](https://github.com/MikesRuthless12). It **plays anything, beautifully**: virtually
any format/codec/container, **hardware-accelerated** (HDR/10-bit, 4K/8K, high-fps), wrapped in a
**modern UI** that VLC lacks — plus a real **media library**, **streaming + casting**, and great
**subtitles**. **No ads, no spyware** — the whole player runs **on your machine** and never phones
home.

> **Plays anything. Beautifully. No ads, no spyware.**

> **Status: pre-development (planning).** This public repository holds the **public site** (`docs/`)
> and project info. The detailed planning + design set (product vision, PRD, roadmap, build-prompts
> guide, competition guide, and go-to-market plan) is **maintained privately** and is not published
> here. The application itself is not built yet — **downloads will be available in future releases.**

> **🔒 Local-first, private by design — and no ads, ever.** All playback, the media library, and
> editing run **100% offline** and **on your machine**. There is **no telemetry, no analytics, and no
> advertising**. The **only** network actions are **explicit and opt-in**: an update check and
> **user-initiated** fetches (online subtitles, library metadata/artwork, network/URL
> streaming, and LAN casting). See [`PRIVACY.md`](PRIVACY.md) and [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md).

## What it does

1. **Plays everything** — virtually any codec/container via a proven **libmpv (mpv) + ffmpeg**
   backend, **hardware-decoded** on your GPU (D3D11VA/DXVA2 on Windows, VideoToolbox on macOS,
   VA-API/VDPAU on Linux): HDR/10-bit, 4K/8K, high-fps, with deinterlace, video filters, frame-step,
   playback speed, A–B repeat, snapshots, and **resume-from-position**.
2. **Sounds right** — all audio formats, **gapless**, ReplayGain, a **10-band EQ** with presets, audio
   device selection + **exclusive mode** + **passthrough/bitstream** (AC3/DTS/TrueHD/Atmos) to a
   receiver, channel mapping/downmix, audio sync/delay, and multiple audio tracks.
3. **Nails subtitles** — SRT/ASS/SSA/PGS/VobSub/WebVTT with full **ASS styling**, position/scale/
   sync/delay, encoding auto-detect, **opt-in** online subtitle fetch (OpenSubtitles), **secondary
   subtitles** (two tracks at once), and accessibility style overrides.
4. **Organizes your media** — scan folders into a real **library**, **opt-in** poster/metadata
   scraping (TMDB/TVDB/MusicBrainz), smart + manual **playlists**, **continue-watching** across files,
   a music library, search/sort/filter, and m3u/pls import/export.
5. **Streams & casts** — network streams (HTTP/HLS/DASH/RTSP/RTP/RTMP/UDP/SMB/FTP), **DVD/Blu-ray/ISO**
   menus, webcams/capture devices, internet radio + podcasts, **YouTube & site URLs** via a yt-dlp
   sidecar, and **cast to / act as** a Chromecast/DLNA/UPnP renderer.
6. **Does the extras** — convert/transcode presets, record streams, **GIF/short-clip** export, a
   media-info panel, sleep timer, **mini-player / Picture-in-Picture**, **media keys + global
   hotkeys**, fully customizable keybindings, skins/themes (Havoc dark default), and portable mode.

It's the **modern VLC** — plays everything, hardware-accelerated, with a library, streaming, casting,
and great subtitles — **without the ads, the spyware, or the dated UI**. Things it can't do
(encrypted-disc decryption, Dolby Vision everywhere) are **stated honestly**, never faked.

## License (important)

Freally Player is **proprietary, source-available** software — **© 2026 Mike Weaver. All Rights
Reserved.** The source is **public so you can read it and build/run it for your own personal
evaluation**, but it is **not open source**: you may not copy, modify, redistribute, or reuse it. See
[`LICENSE`](LICENSE). Bundled/driven third-party components keep their own licenses — see
[`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md).

## Security & privacy

**Local-first and offline by default** — no accounts, no cloud, no ads, no telemetry; your media and
watch history never leave your machine. To report a vulnerability, see [`SECURITY.md`](SECURITY.md)
(please report **privately**, not via a public issue).

## Requirements

- [Rust](https://rustup.rs) (stable; pinned via `rust-toolchain.toml`) and
  [Node.js](https://nodejs.org) (for the Tauri web UI build).
- The **media engine — libmpv (mpv) and ffmpeg (libav\*) development libraries** — is the core
  dependency on every OS (Freally Player links them under their LGPL terms; see
  [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md)):
  - **Windows:** the libmpv + ffmpeg DLLs (vendored at build/package time) plus the Microsoft WebView2
    runtime (Tauri). Hardware decode uses D3D11VA/DXVA2.
  - **macOS:** libmpv + ffmpeg (e.g. via Homebrew: `brew install mpv ffmpeg`) for development; the
    `.app` bundles them. Hardware decode uses VideoToolbox.
  - **Linux:** install the dev libraries and the Tauri/WebKitGTK stack, e.g. on Debian/Ubuntu:
    ```sh
    sudo apt-get install -y \
      libmpv-dev mpv \
      ffmpeg libavcodec-dev libavformat-dev libavutil-dev libswscale-dev \
      libwebkit2gtk-4.1-dev libgtk-3-dev \
      libva-dev libvdpau-dev \
      libdvdnav-dev libbluray-dev \
      pkg-config
    ```
    (libva/libvdpau are for hardware decode; libdvdnav/libbluray are for DVD/Blu-ray menus —
    **AACS keys are NOT bundled**, see [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md).)
  - Optional, downloaded on demand (not bundled): **yt-dlp** (URL playback). No models — the app
    ships no AI/ML features.

## Build & run

```sh
# Web UI dev + Rust shell together (Tauri v2):
npm install                 # install the React+TypeScript+Vite UI deps (in src-tauri/ui)
cargo tauri dev             # builds the UI + Rust core, opens the player window

cargo tauri build           # optimized, packaged build per OS (installer artifacts)
```

The downloadable **Windows** build is a **GUI app with no console window**.

## Develop

```sh
cargo fmt --all -- --check                   # CI format check
cargo clippy --all-targets -- -D warnings    # lint (warnings = errors)
cargo test                                   # Rust tests (incl. bounded-decode fixtures)
npm run lint && npm run typecheck            # UI lint + tsc --noEmit
```

These mirror what CI runs (`.github/workflows/ci.yml`) on Windows, macOS, and Linux.

## Packaging (per-OS installable artifact)

Packaging uses Tauri's bundler:

| OS | Produces | Notes |
|----|----------|-------|
| Windows | `.msi` / NSIS `.exe` | bundles libmpv/ffmpeg DLLs + WebView2 bootstrapper; **GUI app, no console window**; code-signing in the distribution phase |
| macOS | `.app` / `.dmg` | bundles libmpv/ffmpeg; **notarization** in the distribution phase |
| Linux | `.AppImage` / `.deb` / `.rpm` / Flatpak | bundles or depends on libmpv/ffmpeg per format |

### Releases

Pushing a version tag triggers `.github/workflows/release.yml`, which builds the app on all three
OSes, packages each installer, and opens a **draft GitHub Release** with the downloadable artifacts
(reviewed, then published):

```sh
git tag v0.10.0 && git push origin v0.10.0
```

Signed/notarized installers and auto-update arrive in the **distribution** phase. Until releases
exist, the site's download section reads **"Downloads available in future releases."** A
**Releases & Updates** web page lives in [`docs/`](docs/); publish it via
**Settings → Pages → Deploy from a branch → `main` / `docs`** to serve it at
`https://mikesruthless12.github.io/freally-player/`.

## Planned stack

**Rust + Tauri v2** (Rust core + **React + TypeScript + Vite** UI, Havoc dark) · decoded video rendered
into a **native video surface composited under the Tauri webview** (the key architecture decision) ·
**libmpv (mpv) render API** as the primary decode/render engine + **ffmpeg (libav\*)** for coverage and
conversion · **hardware decode** per-OS (D3D11VA/DXVA2 · VideoToolbox · VA-API/VDPAU) · optional
**yt-dlp** sidecar for URL playback · **SQLite** media library. Cargo workspace: `src-tauri` app
crate + owned `crates/` (player, library, subtitles, streaming, convert).

## Editions

Freally Player is **completely free — every feature, for everyone.** No Pro tier, no payments, no
license keys, no account.

- The complete **VLC-grade** core: plays virtually everything hardware-accelerated, full subtitle
  support (incl. ASS + online fetch), audio (EQ, passthrough, tracks), and basic playlists.
  **Basic playback is never crippled.**
- And every power feature: the **media library** + scraping, **casting/DLNA serving**,
  **conversion/transcode presets** with instant **remux**, and **advanced video filters** — all
  included, nothing gated.

## Supported inputs (launch target)

**Files:** virtually any codec/container the libmpv/ffmpeg backend supports (MKV/MP4/AVI/MOV/WebM/TS/…,
H.264/HEVC/AV1/VP9/MPEG-2/…, AAC/AC3/DTS/TrueHD/FLAC/Opus/…).
**Streams:** HTTP/HTTPS/HLS/DASH/RTSP/RTP/RTMP/UDP/SMB/FTP, **YouTube & site URLs** (yt-dlp).
**Discs:** DVD/Blu-ray/ISO **menus** (decryption keys NOT bundled).
**Devices:** webcams/capture devices, internet radio + podcasts; **cast to / be** a Chromecast/DLNA
renderer.

## Repository layout

```
.
├── README.md                # this file
├── CHANGELOG.md             # Keep a Changelog
├── SECURITY.md              # security policy
├── PRIVACY.md               # privacy policy (no telemetry, no ads, opt-in only)
├── EULA.md                  # end-user license agreement (draft)
├── THIRD-PARTY-NOTICES.md   # libmpv/ffmpeg/yt-dlp/libbluray + codec licensing posture
├── LICENSE                  # proprietary, source-available — All Rights Reserved
├── Cargo.toml               # workspace: src-tauri app + crates/ (player/library/subtitles/streaming/convert)
├── rust-toolchain.toml      # pinned stable toolchain
├── src-tauri/               # Tauri v2 app crate (Rust core + commands) + ui/ (React+TS+Vite)
├── crates/                  # owned library crates (player, library, subtitles, streaming, convert)
└── docs/                    # GitHub Pages site (Releases & Updates + Documentation hub)
```

> The planning, spec, roadmap, build-prompts, competition, and go-to-market documents are kept
> **private** and are deliberately **not** part of this public repository.

## Roadmap

The detailed build plan is maintained privately. Public release ladder:
**0.10.0** (foundation) → 0.20 → 0.30 → 0.40 → 0.50 → 0.60 → 0.70 → 0.80 →
**0.85 (library milestone — first public)** → 0.95 → **1.0.0**. Progress is published on the
[project site](https://mikesruthless12.github.io/freally-player/).

---

© 2026 Havoc Software · Mike Weaver &lt;mythodikalone@gmail.com&gt; · All Rights Reserved · _Project
started: June 30th, 2026 · v1.0.0 released: ______ · Downloads available in future releases._
