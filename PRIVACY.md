# Freally Player — Privacy Policy

> **DRAFT — NOT YET LEGALLY REVIEWED.** Review by a qualified attorney is required
> before public distribution.

**Software:** Freally Player · **Contact:** Mike Weaver
&lt;mythodikalone@gmail.com&gt;

## The short version
Freally Player is **local-first and privacy-respecting**: **no accounts, no
telemetry, no analytics, no ads**. Your media, your library, your watch history,
and your settings **never leave your computer** unless *you* explicitly stream,
cast, share, or export them.

## What we collect
**Nothing.** The Licensor does not collect, receive, store, or transmit:

- your media files, audio, video, or playlists;
- your media library, watch history, "continue watching" positions, or bookmarks;
- your settings, file paths, or usage data;
- any personal information or identifiers.

All of your content and settings stay on your device, in the folders, the local
library database, and the configuration locations you control. There is **no
"watch history" sent anywhere** — it lives only in your local library.

## Network use
Freally Player works **fully offline** for playback, the media library, subtitles
you already have, and editing/conversion. It uses the network **only** when **you**
take an action that needs it — specifically:

- **Online subtitle fetch** (e.g. OpenSubtitles) — when you ask it to find subtitles;
- **Library metadata & artwork scraping** (e.g. TMDB / TVDB / MusicBrainz) — when
  you enable scraping for your library;
- **Network / URL streaming** — when you open a network stream (HLS/DASH/RTSP/SMB/…)
  or paste a YouTube/site URL (played via the **yt-dlp** sidecar);
- **Casting / DLNA on your local network** — when you cast to, or share with, a
  device on your LAN;
- **Optional component downloads** — the **yt-dlp** tool and the optional
  **speech-to-text** model, fetched on demand from their third-party distributors;
- **Update checks** — the only contact with a Havoc Software endpoint, and it is
  minimal.

These actions are initiated **by you**. Online subtitle and metadata lookups send
only the minimal identifier needed (e.g. a title, filename, or content hash used
for matching) — **never your file contents, full library, or full file paths**.
Streaming and casting transfer only the specific media you chose, to the
destination you chose (for casting, only on your own LAN). Optional-component
downloads transfer the component **to** your machine and send **no personal data
or media** beyond the standard network request needed to fetch the file. Those
third-party services and distributors have their own privacy practices.

## No ads, no tracking
Freally Player contains **no advertising, no ad SDKs, no trackers, and no
fingerprinting**. We do not build a profile of you, and there is nothing to opt
out of because there is nothing to collect.

## Children
Freally Player is a general-purpose tool, is not directed at children, and
collects no information from anyone.

## Changes
Any change to this policy will be reflected in this document, both in the
application's About panel and in the project repository.

© Mike Weaver — All Rights Reserved.
