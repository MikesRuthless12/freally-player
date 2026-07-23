//! The JSON settings store — `settings.json` in the OS config dir.
//!
//! User configuration lives as plain JSON in the per-user config directory (via
//! `directories`), e.g. `%APPDATA%\Freally\Freally Player\config\` on Windows,
//! `~/Library/Application Support/` on macOS, `~/.config/` on Linux. Writes are atomic
//! (temp file + rename) so a crash never truncates the file.
//!
//! Phase 0 stores window geometry only — the shell's own state. Feature settings arrive
//! with their phases (see `product-roadmap.md`); a malformed or future-schema file always
//! degrades to defaults rather than failing the launch.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use freally_player_core::SubStyleOverride;
use serde::{Deserialize, Serialize};

use crate::paths;

/// Bumped whenever the on-disk shape changes in a way that needs migration.
const SCHEMA_VERSION: u32 = 1;

/// Window geometry bounds. The lower bounds mirror `tauri.conf.json`'s `minWidth`/
/// `minHeight`; the upper bound is a sanity ceiling so a corrupt file can never ask for a
/// window the user cannot reach.
const MIN_WIDTH: u32 = 900;
const MIN_HEIGHT: u32 = 560;
const MAX_DIMENSION: u32 = 16_384;

/// The UI colour scheme. Dark is the Havoc default; light is a full token override.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

/// The main window's restored geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WindowSettings {
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
}

impl Default for WindowSettings {
    fn default() -> Self {
        Self {
            width: 1200,
            height: 800,
            maximized: false,
        }
    }
}

impl WindowSettings {
    /// Clamp geometry into the reachable range, falling back to the default on nonsense.
    fn clamp(&mut self) {
        let default = Self::default();
        if self.width < MIN_WIDTH || self.width > MAX_DIMENSION {
            self.width = default.width;
        }
        if self.height < MIN_HEIGHT || self.height > MAX_DIMENSION {
            self.height = default.height;
        }
    }
}

/// The opt-in OpenSubtitles configuration. Off unless the user enables it and supplies their
/// own API key (a free account). The account **password is never stored** — it is exchanged
/// for a short-lived session token at fetch time; only the username identifier is kept.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OpenSubtitlesSettings {
    /// Whether online subtitle fetch is turned on. Off by default; nothing reaches the network
    /// until this is true.
    pub enabled: bool,
    /// The user's own OpenSubtitles API key.
    pub api_key: Option<String>,
    /// The account username, kept to pre-fill the login. The password is never persisted.
    pub username: Option<String>,
}

/// Everything persisted between runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub schema_version: u32,
    pub window: WindowSettings,
    /// The UI colour scheme.
    pub theme: Theme,
    /// Minimising hides to the system tray instead of the taskbar.
    pub minimize_to_tray: bool,
    /// A user override of subtitle styling (font/size/colour) for readability. Off by default;
    /// when on it overrides the file's own ASS styling. Applies to text subtitles only.
    pub subtitle_style: SubStyleOverride,
    /// Opt-in online subtitle fetch (OpenSubtitles).
    pub opensubtitles: OpenSubtitlesSettings,
    /// The chosen UI language as a BCP-47 tag, or `None` until the user picks one — in which
    /// case the first run detects it from the OS.
    ///
    /// Deliberately an unvalidated `String`: which locales exist is decided by which catalogs
    /// ship, and those live in the UI (`ui/src/i18n/`). Duplicating that list here would mean
    /// two sources of truth that drift, so the UI owns the question and falls back to English
    /// for a tag it does not ship — including a hand-edited nonsense one.
    pub language: Option<String>,
    /// The EULA version the user accepted, if any. `None` until first acceptance — the app
    /// does not render its main UI until this matches the shipped `EULA_VERSION`.
    pub accepted_eula_version: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            window: WindowSettings::default(),
            theme: Theme::default(),
            minimize_to_tray: false,
            subtitle_style: SubStyleOverride::default(),
            opensubtitles: OpenSubtitlesSettings::default(),
            language: None,
            accepted_eula_version: None,
        }
    }
}

/// The subset of [`Settings`] the Settings modal owns.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct UserSettings {
    pub theme: Theme,
    /// Minimising hides to the system tray instead of the taskbar. Off by default, so a user
    /// who never opens Settings gets ordinary minimise behaviour.
    pub minimize_to_tray: bool,
    /// The subtitle-styling override for readability (off by default).
    pub subtitle_style: SubStyleOverride,
    /// Opt-in online subtitle fetch configuration.
    pub opensubtitles: OpenSubtitlesSettings,
    /// The chosen UI language, or `None` while the first run is still following the OS.
    pub language: Option<String>,
}

/// The live settings, backed by a JSON file that is rewritten atomically on every change.
#[derive(Debug)]
pub struct SettingsStore {
    path: PathBuf,
    current: Mutex<Settings>,
}

impl SettingsStore {
    /// Load from the OS config dir, or fall back to an in-memory default when the platform
    /// gives us no config directory (a headless/sandboxed environment).
    pub fn load_default() -> Self {
        match settings_path() {
            Some(path) => Self::load_from(path),
            None => {
                log::warn!("no OS config directory available — settings will not persist");
                Self {
                    path: PathBuf::from("settings.json"),
                    current: Mutex::new(Settings::default()),
                }
            }
        }
    }

    /// Load from an explicit path (used by tests).
    pub fn load_from(path: PathBuf) -> Self {
        let current = read_settings(&path);
        Self {
            path,
            current: Mutex::new(current),
        }
    }

    /// A snapshot of the current settings.
    pub fn get(&self) -> Settings {
        self.current
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Replace the settings and persist them atomically.
    pub fn set(&self, mut next: Settings) -> io::Result<()> {
        next.schema_version = SCHEMA_VERSION;
        next.window.clamp();

        let json = serde_json::to_string_pretty(&next)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        write_atomic(&self.path, &json)?;

        *self
            .current
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = next;
        Ok(())
    }

    /// Persist the main window's geometry.
    pub fn set_window(&self, window: WindowSettings) -> io::Result<()> {
        let mut next = self.get();
        next.window = window;
        self.set(next)
    }

    /// The settings the user can actually change from the Settings modal.
    pub fn user_settings(&self) -> UserSettings {
        let current = self.get();
        UserSettings {
            theme: current.theme,
            minimize_to_tray: current.minimize_to_tray,
            subtitle_style: current.subtitle_style,
            opensubtitles: current.opensubtitles,
            language: current.language,
        }
    }

    /// Apply the user-facing settings.
    ///
    /// Takes only the fields the modal owns rather than a whole [`Settings`], so writing
    /// preferences can never clobber EULA acceptance or window geometry — a whole-struct
    /// setter makes that mistake easy to introduce and hard to spot.
    pub fn set_user_settings(&self, next: UserSettings) -> io::Result<()> {
        let mut settings = self.get();
        settings.theme = next.theme;
        settings.minimize_to_tray = next.minimize_to_tray;
        settings.subtitle_style = next.subtitle_style;
        settings.opensubtitles = next.opensubtitles;
        settings.language = next.language;
        self.set(settings)
    }

    /// Record acceptance of a EULA version. Idempotent.
    pub fn accept_eula(&self, version: &str) -> io::Result<()> {
        let mut next = self.get();
        next.accepted_eula_version = Some(version.to_owned());
        self.set(next)
    }
}

/// `<config dir>/settings.json`, or `None` when the OS exposes no config directory.
fn settings_path() -> Option<PathBuf> {
    paths::config_dir().map(|dir| dir.join("settings.json"))
}

/// Read settings, degrading to defaults on a missing, unreadable, or malformed file — a bad
/// settings file must never stop the app from launching.
fn read_settings(path: &Path) -> Settings {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) => {
            if e.kind() != io::ErrorKind::NotFound {
                log::warn!("could not read {}: {e} — using defaults", path.display());
            }
            return Settings::default();
        }
    };

    // Strip a UTF-8 BOM. `serde_json` rejects one, and plenty of Windows tools write it —
    // Notepad, and PowerShell's `Set-Content -Encoding utf8`. Without this, a user who edits
    // their settings by hand silently loses every preference AND their EULA acceptance,
    // because the parse fails and we fall back to defaults.
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(&raw);

    let mut settings: Settings = match serde_json::from_str(raw) {
        Ok(settings) => settings,
        Err(e) => {
            log::warn!(
                "{} is not valid settings JSON: {e} — using defaults",
                path.display()
            );
            return Settings::default();
        }
    };
    settings.window.clamp();
    settings
}

/// Write via a sibling temp file + rename, so an interrupted write never truncates the real
/// file. The temp name is derived from the target so concurrent stores don't collide.
fn write_atomic(path: &Path, content: &str) -> io::Result<()> {
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp");
    let temp = PathBuf::from(temp);

    let mut file = fs::File::create(&temp)?;
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    drop(file);

    fs::rename(&temp, path).inspect_err(|_| {
        let _ = fs::remove_file(&temp);
    })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    /// A unique scratch dir per test — no global temp-file collisions between test threads.
    fn scratch(name: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "freally-player-settings-{}-{name}-{n}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create scratch dir");
        dir.join("settings.json")
    }

    #[test]
    fn defaults_when_the_file_does_not_exist() {
        let store = SettingsStore::load_from(scratch("missing"));
        assert_eq!(store.get(), Settings::default());
    }

    #[test]
    fn settings_round_trip_through_the_file() {
        let path = scratch("round-trip");
        let store = SettingsStore::load_from(path.clone());
        store
            .set_window(WindowSettings {
                width: 1600,
                height: 900,
                maximized: true,
            })
            .expect("persist settings");

        let reloaded = SettingsStore::load_from(path).get();
        assert_eq!(reloaded.window.width, 1600);
        assert_eq!(reloaded.window.height, 900);
        assert!(reloaded.window.maximized);
        assert_eq!(reloaded.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn malformed_json_degrades_to_defaults_instead_of_failing() {
        let path = scratch("malformed");
        fs::write(&path, "{ not json").expect("write malformed file");
        assert_eq!(SettingsStore::load_from(path).get(), Settings::default());
    }

    #[test]
    fn unreachable_geometry_is_clamped_back_to_the_default() {
        let path = scratch("clamp");
        fs::write(&path, r#"{"window":{"width":1,"height":9999999}}"#).expect("write file");
        let window = SettingsStore::load_from(path).get().window;
        assert_eq!(window.width, WindowSettings::default().width);
        assert_eq!(window.height, WindowSettings::default().height);
    }

    #[test]
    fn eula_acceptance_persists_and_is_idempotent() {
        let path = scratch("eula");
        let store = SettingsStore::load_from(path.clone());
        assert_eq!(store.get().accepted_eula_version, None);

        store.accept_eula("2026-07-01").expect("accept once");
        store.accept_eula("2026-07-01").expect("accept twice");

        let reloaded = SettingsStore::load_from(path).get();
        assert_eq!(
            reloaded.accepted_eula_version.as_deref(),
            Some("2026-07-01")
        );
    }

    #[test]
    fn eula_acceptance_survives_an_unrelated_settings_write() {
        let path = scratch("eula-survives");
        let store = SettingsStore::load_from(path);
        store.accept_eula("2026-07-01").expect("accept");
        store
            .set_window(WindowSettings {
                width: 1024,
                height: 640,
                maximized: false,
            })
            .expect("persist geometry");

        assert_eq!(
            store.get().accepted_eula_version.as_deref(),
            Some("2026-07-01")
        );
    }

    /// The whole reason `set_user_settings` takes a narrow DTO: changing a preference must
    /// not silently un-accept the EULA or forget the window size.
    #[test]
    fn writing_user_settings_preserves_acceptance_and_geometry() {
        let path = scratch("user-settings");
        let store = SettingsStore::load_from(path);
        store.accept_eula("2026-07-21").expect("accept");
        store
            .set_window(WindowSettings {
                width: 1600,
                height: 900,
                maximized: true,
            })
            .expect("persist geometry");

        store
            .set_user_settings(UserSettings {
                theme: Theme::Light,
                minimize_to_tray: true,
                language: Some("ja".to_owned()),
                ..UserSettings::default()
            })
            .expect("persist preferences");

        let after = store.get();
        assert_eq!(after.theme, Theme::Light);
        assert!(after.minimize_to_tray);
        assert_eq!(after.language.as_deref(), Some("ja"));
        assert_eq!(after.accepted_eula_version.as_deref(), Some("2026-07-21"));
        assert_eq!(after.window.width, 1600);
        assert!(after.window.maximized);
    }

    #[test]
    fn user_settings_round_trip_through_the_file() {
        let path = scratch("user-settings-reload");
        SettingsStore::load_from(path.clone())
            .set_user_settings(UserSettings {
                theme: Theme::Light,
                minimize_to_tray: true,
                language: Some("pt-BR".to_owned()),
                ..UserSettings::default()
            })
            .expect("persist");

        let reloaded = SettingsStore::load_from(path).user_settings();
        assert_eq!(reloaded.theme, Theme::Light);
        assert!(reloaded.minimize_to_tray);
        assert_eq!(reloaded.language.as_deref(), Some("pt-BR"));
    }

    /// No stored language means "follow the OS", which the UI resolves at runtime. It has to
    /// stay `None` rather than defaulting to English, or a first run would pin every user to
    /// English and locale detection would never happen.
    #[test]
    fn no_language_is_stored_until_the_user_picks_one() {
        let path = scratch("language-unset");
        let store = SettingsStore::load_from(path.clone());
        assert_eq!(store.get().language, None);

        store
            .set_user_settings(UserSettings {
                theme: Theme::Dark,
                minimize_to_tray: false,
                language: None,
                ..UserSettings::default()
            })
            .expect("persist");

        assert_eq!(
            SettingsStore::load_from(path).user_settings().language,
            None
        );
    }

    /// A settings file written before the Language pane existed has no `language` key at all.
    /// It must load as "not chosen yet", not fail the parse and wipe every other preference.
    #[test]
    fn a_file_from_before_the_language_setting_still_loads() {
        let path = scratch("language-absent");
        fs::write(
            &path,
            r#"{"theme":"light","minimizeToTray":true,"acceptedEulaVersion":"2026-07-21"}"#,
        )
        .expect("write file");

        let settings = SettingsStore::load_from(path).get();
        assert_eq!(settings.language, None);
        assert_eq!(settings.theme, Theme::Light);
        assert_eq!(
            settings.accepted_eula_version.as_deref(),
            Some("2026-07-21")
        );
    }

    /// A hand-edited settings file must survive the BOM that Windows tools add. Losing this
    /// silently discards every preference *and* the recorded EULA acceptance.
    #[test]
    fn a_utf8_bom_does_not_wipe_the_settings() {
        let path = scratch("bom");
        fs::write(
            &path,
            "\u{feff}{\"theme\":\"light\",\"acceptedEulaVersion\":\"2026-07-21\"}",
        )
        .expect("write file with a BOM");

        let settings = SettingsStore::load_from(path).get();
        assert_eq!(settings.theme, Theme::Light);
        assert_eq!(
            settings.accepted_eula_version.as_deref(),
            Some("2026-07-21")
        );
    }

    #[test]
    fn subtitle_style_and_opensubtitles_round_trip() {
        let path = scratch("subs");
        SettingsStore::load_from(path.clone())
            .set_user_settings(UserSettings {
                subtitle_style: SubStyleOverride {
                    enabled: true,
                    font: Some("Atkinson Hyperlegible".to_owned()),
                    font_size: Some(64.0),
                    color: Some("#FFEE00".to_owned()),
                },
                opensubtitles: OpenSubtitlesSettings {
                    enabled: true,
                    api_key: Some("secret-key".to_owned()),
                    username: Some("cinephile".to_owned()),
                },
                ..UserSettings::default()
            })
            .expect("persist");

        let reloaded = SettingsStore::load_from(path).user_settings();
        assert!(reloaded.subtitle_style.enabled);
        assert_eq!(
            reloaded.subtitle_style.font.as_deref(),
            Some("Atkinson Hyperlegible")
        );
        assert!(reloaded.opensubtitles.enabled);
        assert_eq!(
            reloaded.opensubtitles.api_key.as_deref(),
            Some("secret-key")
        );
        assert_eq!(
            reloaded.opensubtitles.username.as_deref(),
            Some("cinephile")
        );
    }

    /// A settings file written before the subtitle fields existed must still load, with those
    /// fields at their defaults — not fail the parse and wipe everything else.
    #[test]
    fn a_file_from_before_the_subtitle_settings_still_loads() {
        let path = scratch("pre-subs");
        fs::write(
            &path,
            r#"{"theme":"light","language":"fr","acceptedEulaVersion":"2026-07-21"}"#,
        )
        .expect("write file");
        let settings = SettingsStore::load_from(path).get();
        assert_eq!(settings.theme, Theme::Light);
        assert!(!settings.subtitle_style.enabled);
        assert!(!settings.opensubtitles.enabled);
        assert_eq!(settings.language.as_deref(), Some("fr"));
    }

    #[test]
    fn a_partial_file_keeps_the_defaults_for_absent_fields() {
        let path = scratch("partial");
        fs::write(&path, r#"{"window":{"maximized":true}}"#).expect("write file");
        let settings = SettingsStore::load_from(path).get();
        assert!(settings.window.maximized);
        assert_eq!(settings.window.width, WindowSettings::default().width);
    }
}
