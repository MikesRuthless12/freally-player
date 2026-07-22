#!/usr/bin/env node
// Vendor libmpv — the non-owned decode/render engine — into third_party/libmpv.
//
// Freally Player links libmpv (LGPL) behind the owned `Engine` trait. The library is NOT
// committed: this script fetches a pinned build, verifies its hash, and prepares it for the
// local toolchain. Run it once before building with `--features engine-libmpv`:
//
//     node scripts/vendor-libmpv.mjs
//
// WINDOWS/MSVC: the upstream package ships a MinGW import library (`libmpv.dll.a`) that the
// MSVC linker cannot read, so this script generates a real `mpv.lib` from the DLL's export
// table. Two details are easy to get wrong and both fail at *runtime*, not link time:
//   1. `libmpv2-sys` emits `cargo:rustc-link-lib=mpv`, so the file MUST be named `mpv.lib`.
//   2. `lib.exe /def:` records the DLL name from the OUTPUT name unless the .def carries an
//      explicit `LIBRARY` line — without it the app imports a nonexistent `mpv.dll` and dies
//      with STATUS_DLL_NOT_FOUND.
//
// macOS/Linux: install libmpv from the system package manager (`brew install mpv`,
// `apt install libmpv-dev`); the loader and pkg-config find it without vendoring.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const vendorDir = join(repoRoot, "third_party");
const libDir = join(vendorDir, "libmpv");

// Pinned build — bump deliberately, never floating. The hash is the supply-chain check:
// a mismatch aborts rather than building against something unexpected.
const RELEASE = {
  tag: "20260610",
  asset: "mpv-dev-x86_64-20260610-git-304426c.7z",
  sha256: "8cbb25ea784f01afbb3f904217cab1317430a8bcfd5680fd827a866367f71cc9",
  url: "https://github.com/shinchiro/mpv-winbuild-cmake/releases/download/20260610/mpv-dev-x86_64-20260610-git-304426c.7z",
};

const DLL = "libmpv-2.dll";

function log(message) {
  console.log(`• ${message}`);
}

function fail(message) {
  console.error(`\n✗ ${message}\n`);
  process.exit(1);
}

/** The first existing path, or null. */
function firstExisting(candidates) {
  return candidates.find((candidate) => existsSync(candidate)) ?? null;
}

function find7z() {
  const found = firstExisting([
    "C:\\Program Files\\7-Zip\\7z.exe",
    "C:\\Program Files (x86)\\7-Zip\\7z.exe",
  ]);
  if (!found) {
    fail(
      "7-Zip is required to extract the libmpv package but was not found.\n" +
        "Install it from https://www.7-zip.org/ and re-run this script.",
    );
  }
  return found;
}

/** Locate the MSVC build tools (dumpbin + lib) via vswhere. */
function findMsvcTools() {
  const vswhere = "C:\\Program Files (x86)\\Microsoft Visual Studio\\Installer\\vswhere.exe";
  if (!existsSync(vswhere)) {
    fail("vswhere.exe not found — install the Visual Studio C++ build tools (MSVC).");
  }
  const installPath = execFileSync(
    vswhere,
    [
      "-latest",
      "-products",
      "*",
      "-requires",
      "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
      "-property",
      "installationPath",
    ],
    { encoding: "utf8" },
  ).trim();
  if (!installPath) {
    fail("No Visual Studio installation with the MSVC x64 toolset was found.");
  }

  const toolsRoot = join(installPath, "VC", "Tools", "MSVC");
  // Highest version wins; MSVC installs side-by-side.
  const version = readdirSync(toolsRoot).sort().reverse()[0];
  if (!version) fail(`No MSVC toolset under ${toolsRoot}`);

  const binDir = join(toolsRoot, version, "bin", "Hostx64", "x64");
  const tools = { dumpbin: join(binDir, "dumpbin.exe"), lib: join(binDir, "lib.exe") };
  for (const [name, path] of Object.entries(tools)) {
    if (!existsSync(path)) fail(`${name}.exe not found at ${path}`);
  }
  return tools;
}

async function download(url, destination) {
  log(`downloading ${RELEASE.asset} …`);
  const response = await fetch(url, { redirect: "follow" });
  if (!response.ok) {
    fail(`download failed: HTTP ${response.status} ${response.statusText}\n  ${url}`);
  }
  writeFileSync(destination, Buffer.from(await response.arrayBuffer()));
}

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

/**
 * Build an MSVC import library from the DLL's export table.
 *
 * The `LIBRARY` line is load-bearing — see the header comment.
 */
function generateImportLibrary(tools) {
  log("generating mpv.lib from the DLL export table …");
  const exports = execFileSync(tools.dumpbin, ["/exports", join(libDir, DLL)], {
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });

  const names = [];
  for (const line of exports.split(/\r?\n/)) {
    // "    1    0 00001000 mpv_create"
    const match = /^\s+\d+\s+[0-9A-Fa-f]+\s+[0-9A-Fa-f]{8}\s+(\S+)/.exec(line);
    if (match) names.push(match[1]);
  }
  if (!names.some((name) => name.startsWith("mpv_"))) {
    fail(`no mpv_* exports found in ${DLL} — is this really a libmpv build?`);
  }
  log(`  ${names.length} exports (${names.filter((n) => n.startsWith("mpv_")).length} mpv_*)`);

  const defPath = join(libDir, "mpv.def");
  writeFileSync(defPath, [`LIBRARY ${DLL}`, "EXPORTS", ...names, ""].join("\n"), "ascii");

  execFileSync(
    tools.lib,
    ["/nologo", `/def:${defPath}`, `/out:${join(libDir, "mpv.lib")}`, "/machine:x64"],
    { stdio: "pipe" },
  );
}

async function main() {
  if (process.platform !== "win32") {
    console.log(
      "This script vendors libmpv for Windows/MSVC only.\n" +
        "On macOS: brew install mpv    On Linux: apt install libmpv-dev\n" +
        "Then build with --features engine-libmpv.",
    );
    return;
  }

  if (existsSync(join(libDir, "mpv.lib")) && existsSync(join(libDir, DLL))) {
    log(`libmpv already vendored at ${libDir} — nothing to do.`);
    log("Delete third_party/ to force a refresh.");
    return;
  }

  const sevenZip = find7z();
  const tools = findMsvcTools();

  mkdirSync(vendorDir, { recursive: true });
  const archive = join(vendorDir, RELEASE.asset);

  if (!existsSync(archive)) {
    await download(RELEASE.url, archive);
  } else {
    log("archive already downloaded.");
  }

  const digest = sha256(archive);
  if (digest !== RELEASE.sha256) {
    rmSync(archive, { force: true });
    fail(
      `checksum mismatch for ${RELEASE.asset}\n` +
        `  expected ${RELEASE.sha256}\n  actual   ${digest}\n` +
        "The downloaded file was deleted. Re-run to try again.",
    );
  }
  log("checksum verified.");

  log(`extracting to ${libDir} …`);
  rmSync(libDir, { recursive: true, force: true });
  execFileSync(sevenZip, ["x", "-y", `-o${libDir}`, archive], { stdio: "pipe" });
  if (!existsSync(join(libDir, DLL))) {
    fail(`${DLL} missing after extraction — the archive layout changed.`);
  }

  generateImportLibrary(tools);

  console.log(`\n✓ libmpv vendored at ${libDir}`);
  console.log("  Build the engine with:  cargo build --features engine-libmpv");
}

await main();
