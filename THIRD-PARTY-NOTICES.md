# Third-Party Notices

Freally Player is proprietary, source-available software (© 2026 Mike Weaver — All Rights Reserved;
see [`LICENSE`](LICENSE)). It incorporates and/or drives the third-party components listed below, each
of which remains licensed under its own terms. This file provides the attribution those licenses
require. Trademarks belong to their respective owners; listing here does not imply endorsement.

> Third-party components are kept **behind interfaces** so an owned implementation can replace them
> later. This list grows as later phases add dependencies. The **decode/render engine (libmpv/ffmpeg)
> is the one piece Freally Player will not realistically own** — replacing a mature media engine is a
> multi-year effort and out of scope; the rest of the app (orchestration, library, subtitle pipeline,
> streaming/casting glue, conversion presets, UI) is authored here and owned.

## Owned components (not third-party)

The **player-core orchestration**, the **media library** (scan/scrape glue, watch-state, schema),
the **subtitle pipeline** (parsing/sync/management glue around the renderer), the **streaming/casting
layer**, the **conversion/clip/record presets**, the **Tauri/web UI**, and the **`.fvlib` library
format** are original works © Mike Weaver, covered by [`LICENSE`](LICENSE) — they are not third-party
components.

## Desktop shell & UI (bundled / linked)

| Component | Role | License |
|-----------|------|---------|
| [Tauri v2](https://tauri.app) (`tauri`, `tauri-build`) | cross-platform desktop shell (Rust core + web UI) | MIT OR Apache-2.0 |
| [React](https://react.dev) + [TypeScript](https://www.typescriptlang.org) + [Vite](https://vitejs.dev) | web UI framework + build tool (control overlay, library, menus) | MIT |
| [`wry`](https://github.com/tauri-apps/wry) / [`tao`](https://github.com/tauri-apps/tao) | Tauri's webview + windowing | MIT OR Apache-2.0 |
| [`serde`](https://serde.rs) / [`serde_json`](https://crates.io/crates/serde_json) | settings + IPC (de)serialization | MIT OR Apache-2.0 |
| [`directories`](https://crates.io/crates/directories) | OS config/data/cache paths | MIT OR Apache-2.0 |
| [`rusqlite`](https://crates.io/crates/rusqlite) (bundled SQLite) | local media library / watch-state / scrape cache | MIT |
| [`sha2`](https://crates.io/crates/sha2) | integrity hashing | MIT OR Apache-2.0 |
| [`fluent`](https://crates.io/crates/fluent) | Fluent catalogs for the 18-language UI/engine strings | Apache-2.0 OR MIT |
| [`log`](https://crates.io/crates/log) | logging facade | MIT OR Apache-2.0 |

Transitive Rust dependencies are MIT / Apache-2.0 / BSD / Zlib / MPL. Verify the full set with
`cargo about` / `cargo deny` before any release.

## Media engine — decode / render / hardware decode (the core dependency)

**Linked / bundled (the playback engine):**

| Component | Role | License |
|-----------|------|---------|
| [mpv / **libmpv**](https://mpv.io) (via [`libmpv2`](https://crates.io/crates/libmpv2) bindings, **render API**) | **primary decode + render engine**; the native video surface is driven through libmpv's render API | **LGPL-2.1+** (libmpv) |
| [**ffmpeg / libav\***](https://ffmpeg.org) (via [`ffmpeg-next`](https://crates.io/crates/ffmpeg-next) bindings) | codec/container coverage + the conversion / clip / record engine | **LGPL-2.1+** (use LGPL builds; do **not** `--enable-gpl`/`--enable-nonfree`) |
| [`symphonia`](https://crates.io/crates/symphonia) | pure-Rust audio decoding where used (gapless/metadata paths) | MPL-2.0 |
| [libass](https://github.com/libass/libass) (via mpv) | ASS/SSA subtitle rendering | ISC |

**Hardware decode (driven via the engine; the user's own OS/GPU stack — not vendored):**

| Component | Role | License / Source |
|-----------|------|------------------|
| D3D11VA / DXVA2 (Windows) | GPU video decode | OS / GPU vendor |
| VideoToolbox (macOS) | GPU video decode | OS (Apple) |
| VA-API / VDPAU (Linux) | GPU video decode | Mesa / GPU vendor |

> **libmpv/ffmpeg licensing posture.** Freally Player links libmpv and ffmpeg built under **LGPL**
> (no GPL-only or nonfree components enabled). LGPL permits use in a proprietary application provided
> the LGPL components remain replaceable/relinkable and their licenses + source offers are honored —
> which is why the engine sits **behind the player-core `Engine` interface** and is shipped as a
> separate shared library, with this notice and a written offer for the corresponding LGPL source.
> We do **not** enable ffmpeg's GPL or nonfree paths. This is the same posture VLC and other
> LGPL-based players ship under; it is reviewed before each release.

## Subtitles, streaming, casting & media keys

**Bundled / linked:**

| Component | Role | License |
|-----------|------|---------|
| [`rupnp`](https://crates.io/crates/rupnp) | UPnP/DLNA control — cast to / discover renderers, act as a renderer | MIT OR Apache-2.0 |
| [`mdns-sd`](https://crates.io/crates/mdns-sd) | mDNS service discovery (Chromecast/DLNA on the LAN) | MIT OR Apache-2.0 |
| [`souvlaki`](https://crates.io/crates/souvlaki) | OS media-key + now-playing integration (Windows SMTC / Linux MPRIS / macOS MediaRemote) | MIT |

**Run as a separate process, downloaded on demand (NOT bundled, NOT linked):**

| Component | Role | License |
|-----------|------|---------|
| [yt-dlp](https://github.com/yt-dlp/yt-dlp) | optional **YouTube / site URL** playback (resolves a stream URL the engine then plays) | Unlicense (public domain) |

`yt-dlp` is **not linked** into Freally Player — it is **invoked as a standalone subprocess** (fetched
on first use to a per-user cache, or supplied by the user on PATH), so its license stays with that
separate binary. The app passes only the URL you entered, via an argv vector (no shell). See
[`SECURITY.md`](SECURITY.md) for the yt-dlp-download trust note.

## Optical media (DVD / Blu-ray / ISO menus)

| Component | Role | License |
|-----------|------|---------|
| [libdvdnav](https://www.videolan.org/developers/libdvdnav.html) / [libdvdread](https://www.videolan.org/developers/libdvdnav.html) | DVD navigation + menus | GPL-2.0+ (driven via the engine / separate library) |
| [libbluray](https://www.videolan.org/developers/libbluray.html) | Blu-ray navigation + menus (BD-J best-effort) | LGPL-2.1+ |
| libaacs / libbdplus | Blu-ray decryption **support** (keys NOT included) | LGPL-2.1+ |

> **Optical-disc decryption is legally constrained.** Freally Player can navigate DVD/Blu-ray/ISO
> **menus**, but **AACS/CSS/BD+ decryption keys are NOT bundled and never will be.** Playing an
> encrypted disc requires keys/configuration **you** provide where lawful in your jurisdiction.
> Region handling and the legality of decryption vary by country and are **the user's
> responsibility** — see [`EULA.md`](EULA.md). Where libdvdnav (GPL) is involved, that path is driven
> via the separate engine library, not statically linked into the proprietary app.

## Optional models (on-demand; not bundled)

| Component | Role | License | Notes |
|-----------|------|---------|-------|
| On-device speech-to-text model (e.g. a Whisper-class GGML model) | optional **auto-subtitles** (STT) + translate, fully local | model's own permissive terms (audited before shipping) | downloaded on demand to a per-user cache; audio never leaves the machine |

Only **permissive, audited** models are ever auto-downloaded; the app shows the size and source
before fetching, and the feature degrades gracefully (manual subtitles still work) if no model is
present.

## Codec / patent note

Several media formats Freally Player can play or export are **patent-encumbered**: **H.264/AVC**,
**HEVC/H.265**, and **AAC** are covered by patent pools (e.g. Via LA, Access Advance); **VVC/H.266**
and some others likewise. The royalty-free formats — **VP8/VP9/AV1** video and **Opus/Vorbis/FLAC**
audio, plus container formats like Matroska/WebM — are not subject to those pools. Freally Player
takes the same posture as VLC and other ffmpeg/libmpv-based players: it **decodes/encodes these
formats via libmpv/ffmpeg (LGPL builds, no GPL/nonfree)** and ships **guidance** rather than codec
licenses. Decoding patent-pooled formats for personal playback is generally understood to be covered
by the licenses already paid into the hardware/OS, but **patent licensing for distribution of an
encoder/decoder is a real, jurisdiction-specific responsibility** that is documented here and reviewed
before release. A from-scratch H.264/HEVC implementation would **not** avoid these patents — they
cover the format's techniques, not the code.

## Rules of thumb for staying clean

- Build libmpv/ffmpeg under **LGPL** (no `--enable-gpl`, no `--enable-nonfree`); keep them behind the
  `Engine` interface and shipped as replaceable shared libraries with a source offer.
- Drive GPL-only components (libdvdnav, GPL ffmpeg) **as separate processes/libraries**, never
  statically linked into the proprietary binary.
- **Never bundle AACS/CSS/BD+ keys**; document the user's legal responsibility.
- Keep `THIRD-PARTY-NOTICES.md` current with every bundled/driven component + license + attribution;
  run `cargo about` / `cargo deny` before each release.
- Only ever auto-download **permissive, audited** models/tools, and show the source/size first.
