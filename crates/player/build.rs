//! Link configuration for the vendored, non-owned libmpv.
//!
//! `libmpv2-sys` emits only `cargo:rustc-link-lib=mpv` — it never tells the linker *where* to
//! look, and on Windows/MSVC the shipped package carries a MinGW import library the MSVC
//! linker cannot read. `scripts/vendor-libmpv.mjs` fetches libmpv and generates a proper
//! `mpv.lib`; this script points the build at that directory and copies the runtime DLL next
//! to the build artifacts so `cargo run` / `cargo test` work without anyone touching PATH.
//!
//! Everything here is inert unless the `engine-libmpv` feature is on, so a plain checkout
//! with no media libraries still builds.

use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=MPV_LIB_DIR");

    // Cargo exposes features to build scripts as CARGO_FEATURE_<NAME>.
    if std::env::var_os("CARGO_FEATURE_ENGINE_LIBMPV").is_none() {
        return;
    }

    let dir = mpv_dir();
    if !dir.is_dir() {
        // On Windows there is no system-wide libmpv and MSVC needs the generated import
        // library, so a missing vendored tree is fatal and says how to fix it.
        if cfg!(target_os = "windows") {
            panic!(
                "the `engine-libmpv` feature needs libmpv, but {} does not exist.\n\
                 Run `node scripts/vendor-libmpv.mjs` to fetch it, or point MPV_LIB_DIR at an \
                 existing libmpv development directory.",
                dir.display()
            );
        }
        // On macOS/Linux libmpv normally comes from the package manager (`brew install mpv`,
        // `apt install libmpv-dev`) and the linker finds `-lmpv` without help, so vendoring
        // is optional. Set MPV_LIB_DIR when it lives somewhere non-standard (Homebrew on
        // Apple Silicon, for instance).
        println!(
            "cargo:warning=no vendored libmpv at {} — relying on a system libmpv",
            dir.display()
        );
        return;
    }

    println!("cargo:rustc-link-search=native={}", dir.display());
    copy_runtime_library(&dir);
}

/// Where libmpv lives: an explicit `MPV_LIB_DIR`, else the vendored `third_party/libmpv` at
/// the repo root.
fn mpv_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("MPV_LIB_DIR") {
        return PathBuf::from(dir);
    }
    let manifest =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"));
    // crates/player -> crates -> <repo root>
    manifest
        .parent()
        .and_then(Path::parent)
        .expect("the crate lives two levels below the repo root")
        .join("third_party")
        .join("libmpv")
}

/// Put the runtime shared library beside the built artifacts.
///
/// On Windows the loader only searches the executable's directory (and PATH), so without this
/// every `cargo run` would fail at startup with a missing-DLL box.
///
/// **Installed builds are handled separately, and cannot be handled at runtime.** `libmpv-2.dll`
/// is a *load-time* import of the executable (`dumpbin /dependents` lists it), so the loader
/// resolves it before `main` runs — `SetDllDirectory`, `LoadLibrary` from a resource path, or
/// any other runtime fixup happens too late to matter. The file simply has to be sitting next
/// to the exe. `src-tauri/tauri.windows.conf.json` maps it into `bundle.resources` with a bare
/// filename target, because on Windows Tauri's resource directory *is* the executable's
/// directory (`tauri_utils::platform::resource_dir`). The map form is required: the list form
/// would rewrite `../third_party/...` to a `_up_/third_party/...` subdirectory, where the
/// loader would never look.
fn copy_runtime_library(dir: &Path) {
    let name = if cfg!(target_os = "windows") {
        "libmpv-2.dll"
    } else {
        // On macOS/Linux the vendored tree is only used when it exists; a system libmpv is
        // found by the dynamic loader without help.
        return;
    };

    let source = dir.join(name);
    if !source.is_file() {
        println!("cargo:warning={} not found — the app will not start until it is present, run `node scripts/vendor-libmpv.mjs`", source.display());
        return;
    }
    println!("cargo:rerun-if-changed={}", source.display());

    // OUT_DIR is target/<profile>/build/<crate>-<hash>/out; three levels up is <profile>.
    let Some(out_dir) = std::env::var_os("OUT_DIR").map(PathBuf::from) else {
        return;
    };
    let Some(profile_dir) = out_dir.ancestors().nth(3) else {
        return;
    };

    // Both the app binary (target/<profile>/) and the test binaries (target/<profile>/deps/)
    // need it: on Windows the loader searches the *executable's own* directory, so one copy
    // in the profile root leaves `cargo test` failing with STATUS_DLL_NOT_FOUND.
    for target_dir in [profile_dir.to_path_buf(), profile_dir.join("deps")] {
        if !target_dir.is_dir() {
            continue;
        }
        let destination = target_dir.join(name);
        if let Err(err) = std::fs::copy(&source, &destination) {
            // Not fatal: a locked DLL usually means a previous build's copy is already there
            // and still correct.
            println!(
                "cargo:warning=could not copy {} to {}: {err}",
                source.display(),
                destination.display()
            );
        }
    }
}
