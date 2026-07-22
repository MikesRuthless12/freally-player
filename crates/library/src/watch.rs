//! Watch-state: resume position + last-used tracks, per file.
//!
//! A JSON map keyed by the file's path lives next to the settings file. Each entry also records
//! the file's **size and modification time**, so a file that has changed under a path — a
//! different episode saved over the old one, a re-encode — is treated as new and its stale
//! resume point is ignored rather than dropping the viewer into the wrong place.
//!
//! Writes are atomic (temp file + rename) so a crash never truncates the store, and a missing
//! or malformed file degrades to "nothing remembered" rather than failing — losing a resume
//! point is a small cost, blocking playback over it is not.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// The most files we keep resume points for. A viewer who opens thousands of files should not
/// grow an unbounded store; the oldest points fall off first.
const MAX_ENTRIES: usize = 4096;

/// What is worth restoring when a file is reopened.
#[derive(Debug, Clone, PartialEq)]
pub struct WatchState {
    /// Where the file was left, in seconds.
    pub position_secs: f64,
    /// The duration known when it was saved, if any — lets the caller ignore a resume point
    /// that sits right at the end.
    pub duration_secs: Option<f64>,
    /// The audio track that was playing (mpv `aid`), to restore it.
    pub audio_id: Option<i64>,
    /// The subtitle track that was showing (mpv `sid`).
    pub sub_id: Option<i64>,
}

/// A recently-watched item, for the idle screen's Continue-Watching row. Serialize-only: it
/// only ever travels from the store out to the UI.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentWatch {
    pub path: String,
    pub position_secs: f64,
    pub duration_secs: Option<f64>,
}

/// One stored row: a [`WatchState`] plus the file identity that validates it and a timestamp
/// for pruning.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    position_secs: f64,
    duration_secs: Option<f64>,
    audio_id: Option<i64>,
    sub_id: Option<i64>,
    /// File size in bytes when saved — half of the change check.
    size: u64,
    /// File modification time in milliseconds since the epoch, when readable — the other half.
    modified_ms: Option<u64>,
    /// When this row was written, for pruning the oldest first.
    updated_ms: u64,
}

/// The persisted map: `{ "<path>": Entry }`, versioned so a future shape can migrate.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Store {
    schema_version: u32,
    entries: HashMap<String, Entry>,
}

const SCHEMA_VERSION: u32 = 1;

/// The live watch-state store, backed by an atomically-rewritten JSON file.
#[derive(Debug)]
pub struct WatchStore {
    path: PathBuf,
    store: Mutex<Store>,
}

impl WatchStore {
    /// Load from an explicit path, degrading to an empty store on a missing or malformed file.
    pub fn load_from(path: PathBuf) -> Self {
        let store = read_store(&path);
        Self {
            path,
            store: Mutex::new(store),
        }
    }

    /// The resume point for `path`, or `None` when nothing is remembered or the file has
    /// changed since it was saved (different size or modification time).
    pub fn get(&self, path: &Path) -> Option<WatchState> {
        let key = key_for(path);
        let guard = self.store.lock().unwrap_or_else(|e| e.into_inner());
        let entry = guard.entries.get(&key)?;

        // A file that no longer matches what we saved is a different file under the same name;
        // its resume point does not belong to it.
        let identity = file_identity(path);
        if entry.size != identity.size || entry.modified_ms != identity.modified_ms {
            return None;
        }

        Some(WatchState {
            position_secs: entry.position_secs,
            duration_secs: entry.duration_secs,
            audio_id: entry.audio_id,
            sub_id: entry.sub_id,
        })
    }

    /// Remember where `path` was left. Stamps the current file identity so a later change
    /// invalidates it. Failures to persist are returned but never block the caller.
    pub fn set(&self, path: &Path, state: WatchState) -> io::Result<()> {
        let identity = file_identity(path);
        let entry = Entry {
            position_secs: state.position_secs,
            duration_secs: state.duration_secs,
            audio_id: state.audio_id,
            sub_id: state.sub_id,
            size: identity.size,
            modified_ms: identity.modified_ms,
            updated_ms: now_ms(),
        };

        let mut guard = self.store.lock().unwrap_or_else(|e| e.into_inner());
        guard.entries.insert(key_for(path), entry);
        prune(&mut guard.entries);
        self.persist(&mut guard)
    }

    /// The most recently updated resume points, newest first, up to `limit`. Feeds the idle
    /// screen's Continue-Watching row.
    pub fn recent(&self, limit: usize) -> Vec<RecentWatch> {
        let guard = self.store.lock().unwrap_or_else(|e| e.into_inner());
        let mut rows: Vec<(&String, &Entry)> = guard.entries.iter().collect();
        // Newest first.
        rows.sort_by_key(|(_, entry)| std::cmp::Reverse(entry.updated_ms));
        rows.into_iter()
            .take(limit)
            .map(|(path, entry)| RecentWatch {
                path: path.clone(),
                position_secs: entry.position_secs,
                duration_secs: entry.duration_secs,
            })
            .collect()
    }

    /// Forget the resume point for `path` — e.g. once it has played to the end.
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

/// The path as a stable string key. Paths are compared byte-for-byte; no normalisation is
/// attempted, so two spellings of the same path get two entries — harmless, and cheaper than
/// guessing at case-folding rules that differ per OS and filesystem.
fn key_for(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// A file's size and modification time — the pair that decides whether a stored point still
/// belongs to the file now under that path.
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
        // A URL or an unreadable path has no filesystem identity; size 0 / no mtime still
        // round-trips, so a network stream can carry a resume point too.
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

/// Keep the store bounded: once past the cap, drop the least-recently-updated entries.
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

/// Read the store, degrading to empty on a missing, unreadable, or malformed file.
fn read_store(path: &Path) -> Store {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) => {
            if e.kind() != io::ErrorKind::NotFound {
                log::warn!(
                    "could not read {}: {e} — starting with no watch state",
                    path.display()
                );
            }
            return Store::default();
        }
    };
    // Strip a UTF-8 BOM some Windows tools prepend; serde_json rejects one.
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
    match serde_json::from_str(raw) {
        Ok(store) => store,
        Err(e) => {
            log::warn!(
                "{} is not valid watch-state JSON: {e} — starting fresh",
                path.display()
            );
            Store::default()
        }
    }
}

/// Write via a sibling temp file + rename, so an interrupted write never truncates the store.
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
        let dir =
            std::env::temp_dir().join(format!("freally-watch-{}-{name}-{n}", std::process::id()));
        fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    /// A real media file to key against, so `file_identity` has something to read.
    fn media_file(dir: &Path, bytes: &[u8]) -> PathBuf {
        let path = dir.join("clip.mkv");
        fs::write(&path, bytes).expect("write media fixture");
        path
    }

    fn state(position: f64) -> WatchState {
        WatchState {
            position_secs: position,
            duration_secs: Some(3600.0),
            audio_id: Some(2),
            sub_id: Some(1),
        }
    }

    #[test]
    fn nothing_is_remembered_until_something_is_saved() {
        let dir = scratch_dir("empty");
        let store = WatchStore::load_from(dir.join("watch_state.json"));
        assert_eq!(store.get(&media_file(&dir, b"video")), None);
    }

    #[test]
    fn a_resume_point_round_trips_through_the_file() {
        let dir = scratch_dir("round-trip");
        let media = media_file(&dir, b"video-bytes");
        let path = dir.join("watch_state.json");

        WatchStore::load_from(path.clone())
            .set(&media, state(123.5))
            .expect("persist");

        let restored = WatchStore::load_from(path)
            .get(&media)
            .expect("a resume point");
        assert_eq!(restored.position_secs, 123.5);
        assert_eq!(restored.duration_secs, Some(3600.0));
        assert_eq!(restored.audio_id, Some(2));
        assert_eq!(restored.sub_id, Some(1));
    }

    #[test]
    fn a_changed_file_ignores_its_stale_resume_point() {
        let dir = scratch_dir("changed");
        let media = media_file(&dir, b"original");
        let store = WatchStore::load_from(dir.join("watch_state.json"));
        store.set(&media, state(200.0)).expect("persist");
        assert!(store.get(&media).is_some());

        // A different file saved over the same path: different size (and mtime).
        fs::write(&media, b"a completely different and longer file").expect("overwrite");
        assert_eq!(
            store.get(&media),
            None,
            "a changed file must not resume at the old position"
        );
    }

    #[test]
    fn recent_lists_newest_first() {
        let dir = scratch_dir("recent");
        let store = WatchStore::load_from(dir.join("watch_state.json"));
        let first = dir.join("first.mkv");
        let second = dir.join("second.mkv");
        fs::write(&first, b"one").expect("write");
        fs::write(&second, b"two longer").expect("write");

        store.set(&first, state(30.0)).expect("persist first");
        store.set(&second, state(40.0)).expect("persist second");

        let recent = store.recent(10);
        assert_eq!(recent.len(), 2);
        // `second` was saved last, so it comes first.
        assert!(recent[0].path.ends_with("second.mkv"));
        assert_eq!(recent[0].position_secs, 40.0);
        // The limit is honoured.
        assert_eq!(store.recent(1).len(), 1);
    }

    #[test]
    fn clearing_forgets_the_resume_point() {
        let dir = scratch_dir("clear");
        let media = media_file(&dir, b"video");
        let store = WatchStore::load_from(dir.join("watch_state.json"));
        store.set(&media, state(50.0)).expect("persist");
        store.clear(&media).expect("clear");
        assert_eq!(store.get(&media), None);
    }

    #[test]
    fn a_malformed_store_degrades_to_empty_instead_of_failing() {
        let dir = scratch_dir("malformed");
        let path = dir.join("watch_state.json");
        fs::write(&path, "{ not json").expect("write malformed");
        let media = media_file(&dir, b"video");
        // Loads without panicking, and can still record fresh state over the bad file.
        let store = WatchStore::load_from(path);
        assert_eq!(store.get(&media), None);
        store
            .set(&media, state(10.0))
            .expect("persist over malformed");
        assert!(store.get(&media).is_some());
    }
}
