# Security Policy

Freally Player is proprietary, source-available software (© 2026 Mike Weaver — All Rights
Reserved; see [`LICENSE`](LICENSE)). Protecting your data is a core design goal: the app is
**local-first and offline by default** — playback, the media library, and editing all run **on
your machine**, with **no accounts, no cloud, no ads, and no telemetry**.

## Supported versions

Freally Player is pre-1.0 and under active development. Security fixes target the **latest** commit
on the default branch; older snapshots are not maintained.

| Version | Supported |
|---------|-----------|
| latest (`main`) | ✅ |
| older | ❌ |

## Reporting a vulnerability

Please report security issues **privately — do not open a public issue or PR**.

- **Email:** mythodikalone@gmail.com (subject: `Freally Player security`), **or**
- **GitHub:** use **Security ▸ Report a vulnerability** (private vulnerability reporting) on this repo.

Include the affected version/commit, your OS, reproduction steps, impact, and any proof-of-concept.
You'll get an acknowledgement and status updates through to the fix. Please allow reasonable time to
remediate before any public disclosure.

## Scope & notes

- **Local-first:** the core never transmits your data. The **only** network actions are *optional and
  explicit* — the update check and **user-initiated** fetches (online subtitles,
  library metadata/artwork scraping, network/URL streaming, casting on your LAN). Nothing is sent
  anywhere otherwise, and there is **no telemetry, ever**.
- **Decode surface (untrusted media is the primary attack surface).** A media file, subtitle file,
  playlist, or network stream can be hostile. Freally Player decodes via **libmpv (mpv) and ffmpeg
  (libav\*)** — mature, widely-audited engines — driven from the Rust core. The owned Rust code that
  parses container/playlist/subtitle metadata **bounds every allocation derived from a file field**
  (dimensions, track counts, durations, subtitle/cue counts, chapter counts) so a malformed or
  hostile input fails cleanly instead of exhausting memory. The owned crates are
  `#![forbid(unsafe_code)]` except at the thin, explicitly-reviewed FFI boundary to libmpv/ffmpeg,
  which is isolated in a single module and audited.
- **Native video surface (Tauri).** Decoded video is rendered into a **native video surface**
  composited **under** the Tauri webview UI (the central architecture decision — see `prd.md`). The
  webview draws only the control overlay; it does **not** receive raw decoded frames, and the web UI
  has **no network access of its own** — all I/O goes through audited Rust Tauri commands. Tauri's
  allowlist/capabilities are scoped to the minimum commands the UI needs.
- **Subtitle rendering.** ASS/SSA subtitles are a scripting-capable format; rendering is handled by
  the engine's subtitle renderer (libass via mpv) with embedded-font and drawing handling kept to the
  engine's hardened path. Online subtitle fetch (OpenSubtitles) is **opt-in**, over **TLS** to a
  fixed host; downloaded subtitle files are treated as untrusted input and parsed through the same
  bounded path.
- **yt-dlp sidecar (URL playback).** YouTube/site URL playback runs **yt-dlp as a separate
  subprocess** (not linked into the app), invoked with an **argv vector (no shell)**; the app passes
  only the URL you entered. yt-dlp is **downloaded on demand** to a per-user cache and then executed.
  **Honest trust note:** that download fetches an **executable** from a third-party host — a
  compromised host or MITM is a code-execution risk. The feature is optional and clearly labeled.
  **Tracked hardening:** pin/verify the yt-dlp download (signature/hash), and offer a "use my own
  yt-dlp on PATH" option so no download is required.
- **Library scraping (metadata/artwork).** Folder scanning is local. Metadata/artwork scraping
  (TMDB/TVDB/MusicBrainz) is **opt-in**, performed by the Rust core over **TLS** to fixed hosts;
  only the title/filename needed to identify a work is sent (never your file contents or full paths),
  and fetched artwork is streamed to a per-user cache via a temp file plus an atomic rename. **Tracked
  hardening:** allow a "no online metadata" library mode and per-source toggles.
- **Casting / DLNA (LAN only).** Casting to (or acting as) a Chromecast/DLNA/UPnP renderer is **LAN
  only** and **off until you start it**. Discovery (mDNS/SSDP) and control bind to local interfaces;
  when Freally Player acts as a renderer/server it serves **only** the media you explicitly share, on
  a port you can see, and stops when you stop casting. No content leaves your LAN.
- **Optical media (DVD/Blu-ray/ISO).** Disc menus use **libdvdnav/libbluray**. **AACS/CSS decryption
  keys are NOT bundled**; encrypted discs require keys/configuration you provide, and you are
  responsible for compliance with your jurisdiction's laws. See [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md)
  and [`EULA.md`](EULA.md).
- **No AI/ML components.** Freally Player ships **no models, bundled or downloaded**, and has no
  speech-to-text or machine-translation features. Every feature is deterministic, classic
  engineering. (An earlier plan for on-device auto-subtitles was cut before implementation.)
- **Updates / downloads integrity.** All optional downloads (yt-dlp, artwork) are over
  **TLS** from fixed, hardcoded hosts; target filenames are hardcoded literals (no path-traversal
  input); each file is streamed to a temp path and atomically renamed. **Tracked hardening:** per-file
  SHA-256/signature pinning — TLS authenticates the host, not the bytes.
- **Third-party components** (see [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md)) carry their own
  advisories; we track and update them, and intend to run `cargo audit` / `cargo deny` in CI as the
  project matures.
- **No secrets** are bundled or logged; `.env` and config files are treated as sensitive.

Thank you for helping keep Freally Player and its users safe.
