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
| [`tauri-plugin-process`](https://crates.io/crates/tauri-plugin-process) | lets the UI exit the app (the first-run EULA gate's "Decline & Quit") | MIT OR Apache-2.0 |
| [Tailwind CSS](https://tailwindcss.com) (`tailwindcss`, `@tailwindcss/vite`) | UI styling — the Havoc dark/light theme tokens | MIT |
| [`rfd`](https://crates.io/crates/rfd) | native "stopped unexpectedly" message box after a crash (MessageBoxW / NSAlert / GTK3) | MIT |
| [`sysinfo`](https://crates.io/crates/sysinfo) | process-table lookup so the crash helper waits for the crashed process to be reaped | MIT |
| [`chrono`](https://crates.io/crates/chrono) | crash-report timestamps (local time with offset, plus UTC) | MIT OR Apache-2.0 |

Transitive Rust dependencies are MIT / Apache-2.0 / BSD / Zlib / MPL. Verify the full set with
`cargo about` / `cargo deny` before any release.

## Fonts — bundled

| Component | Role | License |
|-----------|------|---------|
| [Noto Sans](https://fonts.google.com/noto) (Latin / Greek / Cyrillic / Devanagari / Vietnamese) | the UI typeface | **OFL-1.1** |
| Noto Sans Arabic · Noto Sans Devanagari | Arabic and Hindi UI text | **OFL-1.1** |
| Noto Sans SC · TC · JP · KR | Simplified/Traditional Chinese, Japanese, Korean UI **and content** text | **OFL-1.1** |
| [Fontsource](https://fontsource.org) (`@fontsource-variable/*`) | the packaging that delivers the above as self-hosted WOFF2 | MIT (packaging; the fonts stay OFL-1.1) |

> **Noto is OFL-1.1, not Apache-2.0.** Early Noto releases were Apache-2.0 and the assumption
> outlives them; Google relicensed the family to the SIL Open Font License years ago, and OFL is
> what governs what we ship. OFL explicitly permits bundling inside a proprietary application:
> the fonts may be redistributed with software, at no cost or sold as part of it, provided they
> are not sold *on their own* and this notice travels with them.
>
> The one clause with teeth is the **Reserved Font Name**: a modified font may not keep the Noto
> name. We therefore ship the published WOFF2 files **byte-for-byte unmodified** — no subsetting,
> no re-hinting, no renaming — which keeps us clear of it entirely. If a future build ever
> subsets these fonts to save space, it must be renamed, and this notice updated to match.
>
> Bundled rather than relied upon: Windows and macOS ship CJK system fonts, but many Linux
> installs do not, so unbundled CJK renders as tofu there. The variable builds keep all four CJK
> families to ~19 MB in total. `ui/e2e/fonts.spec.ts` asserts every shipped language actually
> rasterises in a bundled face.

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
> separate shared library, with this notice and the written offer below.
> We do **not** enable ffmpeg's GPL or nonfree paths. This is the same posture VLC and other
> LGPL-based players ship under; it is reviewed before each release.

### Written offer for the LGPL source — exactly what we ship

The installers **bundle** the LGPL engine as a shared library, so this offer is a live obligation,
not a formality. What ships, per platform:

| Platform | Bundled file | Upstream build |
|----------|--------------|----------------|
| Windows | `libmpv-2.dll`, installed **next to `freally-player.exe`** | [`shinchiro/mpv-winbuild-cmake`](https://github.com/shinchiro/mpv-winbuild-cmake) release **`20260610`**, asset `mpv-dev-x86_64-20260610-git-304426c.7z` (mpv `git-304426c`) — SHA-256 pinned and verified by `scripts/vendor-libmpv.mjs` |

This Windows build links ffmpeg and libass **statically into `libmpv-2.dll`**; that whole library is
LGPL and is shipped as a replaceable file, which is what LGPL §4 requires. **You may replace it:**
drop your own build of `libmpv-2.dll` (same filename, compatible ABI) into the install directory and
the app will load yours instead — the executable imports it by name at load time.

**The offer.** For **three years** from the date you received this software, we will provide the
complete corresponding source for the LGPL components in the build you received, plus the scripts
used to configure and build them, for no more than the cost of distribution. Request it from
**mythodikalone@gmail.com** (subject: *"LGPL source request — Freally Player"*), stating the version
and platform shown in the app's **About** pane (the ⓘ icon in the title bar). The upstream sources
are also public at the URLs above; the pinned tag and commit identify the exact revision.

> **Maintenance note:** when `scripts/vendor-libmpv.mjs` bumps its pinned `RELEASE`, update this
> table in the same commit. An offer that names a build we no longer ship is not a valid offer.

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

## Models

**None.** Freally Player ships no AI/ML models, bundled or downloaded on demand. An earlier plan
for an on-device speech-to-text model (auto-subtitles) was cut before implementation, so there is
no model component to license, audit, or fetch. Subtitles come from the file, from a sidecar file,
or from an opt-in online lookup.

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
