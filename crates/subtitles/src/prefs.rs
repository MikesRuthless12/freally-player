//! Per-file subtitle preferences: timing, placement and scale, remembered per media file.
//!
//! When a viewer nudges the subtitle delay to match a slightly-off SRT, or moves the line up
//! off a hardcoded caption, that adjustment belongs to *that file* — it should be there next
//! time without being reapplied to everything else. This is a small JSON map keyed by the
//! file's path, with the same size+mtime identity check and atomic-write discipline as the
//! watch-state store, so a changed file does not inherit stale placement and a crash never
//! truncates the store.
//!
//! Track *selection* (which `sid`/`aid`) is already remembered by the watch-state store; this
//! only covers the appearance/timing knobs that store does not.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// The most files we keep subtitle preferences for; the least-recently-updated fall off first.
const MAX_ENTRIES: usize = 4096;

const SCHEMA_VERSION: u32 = 1;

/// The subtitle knobs remembered for one file.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitlePrefs {
    /// Timing offset in seconds (mpv `sub-delay`).
    pub delay_secs: f64,
    /// Vertical position on mpv's 0–150 scale (`sub-pos`).
    pub pos: i64,
    /// Size multiplier (mpv `sub-scale`).
    pub scale: f64,
    /// Whether the primary subtitle is shown.
    pub visible: bool,
    /// The secondary subtitle track that was showing (mpv `secondary-sid`), if any.
    #[serde(default)]
    pub secondary_id: Option<i64>,
}

impl Default for SubtitlePrefs {
    fn default() -> Self {
        Self {
            delay_secs: 0.0,
            pos: 100,
            scale: 1.0,
            visible: true,
            secondary_id: None,
        }
    }
}

impl SubtitlePrefs {
    /// Whether these are just the defaults — nothing worth persisting a row for.
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// One stored row: the preferences plus the file identity that validates them and a timestamp
/// for pruning. The payload is flattened, so the on-disk JSON keeps the `SubtitlePrefs` fields at
/// the top level beside the identity fields, and a new pref is added in one place.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    #[serde(flatten)]
    prefs: SubtitlePrefs,
    size: u64,
    modified_ms: Option<u64>,
    updated_ms: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Store {
    schema_version: u32,
    entries: HashMap<String, Entry>,
}

/// The live subtitle-preferences store, backed by an atomically-rewritten JSON file.
#[derive(Debug)]
pub struct SubtitlePrefsStore {
    path: PathBuf,
    store: Mutex<Store>,
}

impl SubtitlePrefsStore {
    /// Load from an explicit path, degrading to an empty store on a missing or malformed file.
    pub fn load_from(path: PathBuf) -> Self {
        let store = read_store(&path);
        Self {
            path,
            store: Mutex::new(store),
        }
    }

    /// The remembered preferences for `path`, or `None` when nothing is stored or the file has
    /// changed since (different size or modification time).
    pub fn get(&self, path: &Path) -> Option<SubtitlePrefs> {
        let key = key_for(path);
        let guard = self.store.lock().unwrap_or_else(|e| e.into_inner());
        let entry = guard.entries.get(&key)?;

        let identity = file_identity(path);
        if entry.size != identity.size || entry.modified_ms != identity.modified_ms {
            return None;
        }
        Some(entry.prefs)
    }

    /// Remember `prefs` for `path`. Storing the defaults instead *clears* any row, so a file the
    /// viewer reset never keeps an all-defaults entry around.
    pub fn set(&self, path: &Path, prefs: SubtitlePrefs) -> io::Result<()> {
        if prefs.is_default() {
            return self.clear(path);
        }
        let identity = file_identity(path);
        let entry = Entry {
            prefs,
            size: identity.size,
            modified_ms: identity.modified_ms,
            updated_ms: now_ms(),
        };
        let mut guard = self.store.lock().unwrap_or_else(|e| e.into_inner());
        guard.entries.insert(key_for(path), entry);
        prune(&mut guard.entries);
        self.persist(&mut guard)
    }

    /// Forget the preferences for `path`.
    pub fn clear(&self, path: &Path) -> io::Result<()> {
        let mut guard = self.store.lock().unwrap_or_else(|e| e.into_inner());
        if guard.entries.remove(&key_for(path)).is_none() {
            return Ok(());
        }
        self.persist(&mut guard)
    }

    fn persist(&self, store: &mut Store) -> io::Result<()> {
        store.schema_version = SCHEMA_VERSION;
        let json = serde_json::to_string_pretty(store)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        write_atomic(&self.path, &json)
    }
}

fn key_for(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

struct FileIdentity {
    size: u64,
    modified_ms: Option<u64>,
}

fn file_identity(path: &Path) -> FileIdentity {
    match fs::metadata(path) {
        Ok(meta) => FileIdentity {
            size: meta.len(),
            modified_ms: meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64),
        },
        Err(_) => FileIdentity {
            size: 0,
            modified_ms: None,
        },
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn prune(entries: &mut HashMap<String, Entry>) {
    if entries.len() <= MAX_ENTRIES {
        return;
    }
    let mut by_recency: Vec<(String, u64)> = entries
        .iter()
        .map(|(k, e)| (k.clone(), e.updated_ms))
        .collect();
    by_recency.sort_by_key(|(_, updated)| *updated);
    for (key, _) in by_recency.iter().take(entries.len() - MAX_ENTRIES) {
        entries.remove(key);
    }
}

fn read_store(path: &Path) -> Store {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) => {
            if e.kind() != io::ErrorKind::NotFound {
                log::warn!(
                    "could not read {}: {e} — starting with no subtitle preferences",
                    path.display()
                );
            }
            return Store::default();
        }
    };
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
    match serde_json::from_str(raw) {
        Ok(store) => store,
        Err(e) => {
            log::warn!(
                "{} is not valid subtitle-preferences JSON: {e} — starting fresh",
                path.display()
            );
            Store::default()
        }
    }
}

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

    fn scratch_dir(name: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "freally-subprefs-{}-{name}-{n}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    fn media_file(dir: &Path, bytes: &[u8]) -> PathBuf {
        let path = dir.join("clip.mkv");
        fs::write(&path, bytes).expect("write media fixture");
        path
    }

    fn prefs(delay: f64) -> SubtitlePrefs {
        SubtitlePrefs {
            delay_secs: delay,
            pos: 90,
            scale: 1.25,
            visible: true,
            secondary_id: Some(2),
        }
    }

    #[test]
    fn nothing_is_remembered_until_something_is_saved() {
        let dir = scratch_dir("empty");
        let store = SubtitlePrefsStore::load_from(dir.join("subtitle_prefs.json"));
        assert_eq!(store.get(&media_file(&dir, b"video")), None);
    }

    #[test]
    fn preferences_round_trip_through_the_file() {
        let dir = scratch_dir("round-trip");
        let media = media_file(&dir, b"video-bytes");
        let path = dir.join("subtitle_prefs.json");

        SubtitlePrefsStore::load_from(path.clone())
            .set(&media, prefs(2.5))
            .expect("persist");

        let restored = SubtitlePrefsStore::load_from(path)
            .get(&media)
            .expect("preferences");
        assert_eq!(restored.delay_secs, 2.5);
        assert_eq!(restored.pos, 90);
        assert_eq!(restored.scale, 1.25);
        assert_eq!(restored.secondary_id, Some(2));
    }

    #[test]
    fn a_changed_file_ignores_its_stale_preferences() {
        let dir = scratch_dir("changed");
        let media = media_file(&dir, b"original");
        let store = SubtitlePrefsStore::load_from(dir.join("subtitle_prefs.json"));
        store.set(&media, prefs(3.0)).expect("persist");
        assert!(store.get(&media).is_some());

        fs::write(&media, b"a completely different and much longer file").expect("overwrite");
        assert_eq!(store.get(&media), None);
    }

    #[test]
    fn saving_defaults_clears_the_row() {
        let dir = scratch_dir("reset");
        let media = media_file(&dir, b"video");
        let store = SubtitlePrefsStore::load_from(dir.join("subtitle_prefs.json"));
        store.set(&media, prefs(1.0)).expect("persist");
        assert!(store.get(&media).is_some());

        // Resetting to defaults should not leave an all-defaults entry lying around.
        store
            .set(&media, SubtitlePrefs::default())
            .expect("reset to defaults");
        assert_eq!(store.get(&media), None);
    }

    #[test]
    fn a_malformed_store_degrades_to_empty_instead_of_failing() {
        let dir = scratch_dir("malformed");
        let path = dir.join("subtitle_prefs.json");
        fs::write(&path, "{ not json").expect("write malformed");
        let media = media_file(&dir, b"video");
        let store = SubtitlePrefsStore::load_from(path);
        assert_eq!(store.get(&media), None);
        store
            .set(&media, prefs(1.0))
            .expect("persist over malformed");
        assert!(store.get(&media).is_some());
    }
}
