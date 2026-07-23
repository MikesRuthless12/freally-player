// A minimal Tauri v2 IPC mock so the REAL built UI renders in a plain browser (Playwright)
// for the visual-smoke gallery. It shims `window.__TAURI_INTERNALS__` with an `invoke` that
// returns canned, valid data per command, plus an event system the spec can drive.
//
// This is UI-render coverage ONLY. There is no media engine, no audio device, no GPU and no
// native video surface behind it — everything those provide is covered by the per-OS
// `cargo test` suite and, where only a human can judge it, by `Live-To-Do-List.md`.
//
// Runs via Playwright addInitScript (before the app bundle loads).
(() => {
  const params = new URLSearchParams(location.search);
  const eulaAccepted = params.get("eula") !== "0";
  const theme = params.get("theme") === "light" ? "light" : "dark";
  const pendingCrash = params.get("crash") === "1";

  const EULA_TEXT = [
    "# Freally Player — End User License Agreement (EULA)",
    "",
    "> **DRAFT — NOT YET LEGALLY REVIEWED.** (Mocked text for the UI gallery.)",
    "",
    "## 1. License grant",
    "The Software is **proprietary** and **All Rights Reserved**.",
    "",
    "## 3. Your content and your responsibility",
    "**You are solely responsible for your User Content and for how you use the Software,**",
    "including ensuring that you have all necessary rights and permissions.",
    "",
    "## 6. No warranty",
    'THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND.',
    "",
  ].join("\n");

  const CRASH_EXCERPT =
    "Crashed: 2026-07-21 20:11:03 -0500 (UTC 2026-07-22 01:11:03)\n" +
    "Panic at crates/player/src/mpv.rs:118\n" +
    "Message: the playback engine went away\n\n" +
    "Backtrace:\n   0: freally_player_core::mpv::MpvEngine::open\n   1: <home>/…\n";

  // The transport the gallery photographs. `?media=1` opens a file so the stage and
  // transport render their playing state, with a couple of chapters for the scrubber ticks.
  const media =
    params.get("media") === "1"
      ? {
          path: "C:/Videos/Big Buck Bunny.mkv",
          title: "Big Buck Bunny",
          durationSecs: 596,
          chapters: [
            { title: "Intro", startSecs: 0 },
            { title: "The meadow", startSecs: 120 },
            { title: "Finale", startSecs: 420 },
          ],
          // Two audio and two subtitle tracks, so the audio/subtitle menus have real content.
          tracks: [
            {
              id: 1,
              kind: "audio",
              lang: "en",
              title: "Stereo",
              default: true,
              external: false,
              imageBased: false,
            },
            {
              id: 2,
              kind: "audio",
              lang: "ja",
              title: null,
              default: false,
              external: false,
              imageBased: false,
            },
            {
              id: 1,
              kind: "sub",
              lang: "en",
              title: "English",
              default: true,
              external: false,
              imageBased: false,
            },
            {
              id: 2,
              kind: "sub",
              lang: "es",
              title: null,
              default: false,
              external: false,
              imageBased: false,
            },
          ],
        }
      : null;

  // The transport fields the control bar reads. `?media=1` plays; otherwise idle at rest.
  const transport = {
    volume: 100,
    muted: false,
    speed: 1,
    bufferedSecs: media ? 596 : 0,
    abLoop: { a: null, b: null },
    audioId: media ? 1 : null,
    subtitle: {
      id: media ? 1 : null,
      secondaryId: null,
      visible: true,
      delaySecs: 0,
      pos: 100,
      scale: 1,
    },
  };

  // `?os=1` turns on the opt-in OpenSubtitles config so the online panel renders; `?substyle=1`
  // turns on the subtitle style override so the Settings pane shows its font/size/colour fields.
  const online = params.get("os") === "1";
  const styleOn = params.get("substyle") === "1";

  // `?recent=1` seeds the idle screen's Continue-Watching row.
  const recent =
    params.get("recent") === "1"
      ? [
          { path: "C:/Videos/Arrival.2016.mkv", positionSecs: 1830, durationSecs: 7200 },
          { path: "C:/Videos/Big Buck Bunny.mkv", positionSecs: 140, durationSecs: 596 },
        ]
      : [];

  const RESP = {
    app_info: { name: "Freally Player", version: "0.30.0" },
    // `?lang=xx` starts in a stored locale; with none, the UI detects one from the browser
    // exactly as a first run detects it from the OS.
    settings_get: {
      theme,
      minimizeToTray: params.get("tray") === "1",
      subtitleStyle: {
        enabled: styleOn,
        font: styleOn ? "Atkinson Hyperlegible" : null,
        fontSize: styleOn ? 64 : null,
        color: styleOn ? "#ffee00" : null,
      },
      openSubtitles: {
        enabled: online,
        apiKey: online ? "demo-api-key" : null,
        username: online ? "cinephile" : null,
      },
      language: params.get("lang"),
    },
    settings_set: null,
    eula_status: { version: "2026-07-21", text: EULA_TEXT, accepted: eulaAccepted },
    eula_accept: null,
    // `?paused=1` opens the file paused (controls stay up, the toggle reads Play); otherwise
    // a playing file.
    get_state: media
      ? {
          status: params.get("paused") === "1" ? "paused" : "playing",
          positionSecs: 137,
          media,
          ...transport,
        }
      : { status: "idle", positionSecs: 0, media: null, ...transport },
    open_media: media,
    play: null,
    pause: null,
    toggle_play: null,
    seek: null,
    set_video_rect: null,
    set_volume: null,
    set_muted: null,
    set_speed: null,
    frame_step: null,
    set_ab_loop: null,
    set_chapter: null,
    capture_frame: null,
    recent_watch: recent,
    // Phase 2 — subtitles & audio tracks.
    set_audio_track: null,
    set_subtitle_track: null,
    set_secondary_subtitle_track: null,
    set_subtitle_visible: null,
    set_subtitle_delay: null,
    set_subtitle_pos: null,
    set_subtitle_scale: null,
    set_subtitle_style_override: null,
    add_subtitle_file: { trackId: 3, sourceEncoding: "windows-1251", imageBased: false },
    opensubtitles_search: online
      ? [
          {
            fileId: 101,
            fileName: "Big.Buck.Bunny.en.srt",
            language: "en",
            release: "BluRay",
            downloadCount: 900,
          },
          {
            fileId: 102,
            fileName: "Big.Buck.Bunny.es.srt",
            language: "es",
            release: "WEBRip",
            downloadCount: 120,
          },
        ]
      : [],
    opensubtitles_login: null,
    opensubtitles_download: { trackId: 4, sourceEncoding: null, imageBased: false },
    bug_report_context: {
      appVersion: "0.30.0",
      os: "windows",
      arch: "x86_64",
      diagnostics: "App: Freally Player 0.30.0\nOS: windows / x86_64\n",
      pendingCrash: pendingCrash ? CRASH_EXCERPT : null,
    },
    bug_report_submit: null,
    bug_report_clear_crash: null,
    open_external: null,
  };

  function respond(cmd) {
    if (cmd in RESP) return RESP[cmd];
    // Event plugin: let listen()/emit() resolve. Dialog: pretend the user picked a file.
    if (cmd.startsWith("plugin:event|")) return 0;
    if (cmd === "plugin:dialog|open") {
      return params.get("cancel") === "1" ? null : "C:/Videos/Big Buck Bunny.mkv";
    }
    if (cmd === "plugin:dialog|save") {
      return params.get("cancel") === "1" ? null : "C:/Videos/snapshot.png";
    }
    if (cmd.startsWith("plugin:")) return null;
    return null;
  }

  // Every invocation is recorded so tests can assert the UI actually drove the backend,
  // rather than only that a button rendered.
  window.__invokeLog = [];

  // `?engine=0` makes the engine refuse, so the gallery can photograph the honesty
  // invariant: a build with no decode backend must say so, not show a black stage.
  const ENGINE_REFUSAL =
    "this build has no playback engine — it was built without the libmpv backend";

  let cbId = 0;
  window.__TAURI_INTERNALS__ = {
    invoke: (cmd, args) => {
      window.__invokeLog.push({ cmd, args });
      if (params.get("engine") === "0" && ["open_media", "play", "pause", "seek"].includes(cmd)) {
        return Promise.reject(ENGINE_REFUSAL);
      }
      return Promise.resolve(respond(cmd));
    },
    transformCallback: (cb) => {
      const id = ++cbId;
      window[`_${id}`] = cb;
      return id;
    },
    convertFileSrc: (path, protocol) => `${protocol || "asset"}://localhost/${path}`,
    metadata: {
      currentWindow: { label: "main" },
      currentWebview: { windowLabel: "main", label: "main" },
    },
    plugins: {},
  };
})();
