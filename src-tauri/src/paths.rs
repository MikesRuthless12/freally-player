//! The app's path-resolution chokepoint.
//!
//! Every persistent path resolves through here, so there is exactly one place that decides
//! where Freally Player keeps user data. Both directories are per-user OS locations via
//! `directories`; on a platform that exposes neither, callers get `None` and degrade to
//! not persisting rather than writing somewhere unexpected.

use std::path::PathBuf;

use directories::ProjectDirs;

fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("com", "Freally", "Freally Player")
}

/// User configuration — `settings.json` lives here.
pub fn config_dir() -> Option<PathBuf> {
    project_dirs().map(|dirs| dirs.config_dir().to_path_buf())
}

/// App data — crash reports live here.
pub fn data_dir() -> Option<PathBuf> {
    project_dirs().map(|dirs| dirs.data_dir().to_path_buf())
}
