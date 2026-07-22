#!/usr/bin/env node
// Local CI — mirrors .github/workflows/ci.yml. Run this and get it GREEN BEFORE PUSHING.
//
// This is a Definition-of-Done step, not a convenience: pushing to find out what CI thinks
// costs ~6 minutes per attempt, and every failure it catches here is one you would otherwise
// wait on GitHub to discover.
//
// What it CANNOT tell you: this machine is one OS. Cross-platform breakage — a `#[cfg]` that
// only compiles on Windows, a dead-code warning that only fires elsewhere — still shows up
// first on the CI matrix. Green here means "worth pushing", not "CI will pass".
//
// NOTE: This repo has no .github/workflows yet (planning-stage skeleton). It is a
// Rust-only Cargo workspace with a pinned toolchain (rustfmt + clippy — see
// rust-toolchain.toml) and a rustfmt.toml, so this runner mirrors the standard Rust
// gate that CI will run once it lands:
//   Rust: cargo fmt --check · clippy -D warnings · test   (+ cargo-deny if configured)
//
// The React/TS/Vite UI is planned to live in ./ui (see src-tauri/Cargo.toml). There is
// no ui/package.json yet, so UI/e2e checks are auto-detected: they're skipped now and
// will run automatically once a ui/package.json with the expected scripts exists.
//
// Unlike a CI job (which stops at the first failing step), this runs EVERY check and
// prints one summary at the end, so a single pass surfaces all problems. It exits
// non-zero if anything failed, so it's safe to gate a push on it.
//
// Usage:  node scripts/ci-local.mjs [--rust-only] [--ui-only] [--no-e2e] [--install]
//   --rust-only  run only the Rust checks
//   --ui-only    run only the UI checks (once ui/package.json exists)
//   --no-e2e     skip the Playwright e2e step (only relevant once a UI exists)
//   --install    (re)install UI deps first: npm ci + playwright browsers
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const uiDir = join(repoRoot, "ui");

const args = new Set(process.argv.slice(2));
const rustOnly = args.has("--rust-only");
const uiOnly = args.has("--ui-only");
const noE2e = args.has("--no-e2e");
const doInstall = args.has("--install");

// Pass the whole probe as one shell string (not an args array) — with shell:true an
// args array triggers a Node deprecation warning and isn't escaped anyway.
function have(commandLine) {
  return spawnSync(commandLine, { stdio: "ignore", shell: true }).status === 0;
}

const steps = [];
function step(name, cmd, cwd) {
  steps.push({ name, cmd, cwd });
}

const hasRust =
  existsSync(join(repoRoot, "Cargo.toml")) ||
  existsSync(join(repoRoot, "src-tauri", "Cargo.toml"));
const hasUi = existsSync(join(uiDir, "package.json"));
const hasDeny = existsSync(join(repoRoot, "deny.toml"));

if (doInstall && hasUi) {
  step("ui: npm ci", "npm ci", uiDir);
  step("ui: playwright install", "npx playwright install --with-deps", uiDir);
}

if (!uiOnly && hasRust) {
  // NOT --all-features: `engine-ffmpeg` links against system media libraries a plain checkout
  // does not have. `engine-libmpv` IS in the default set for the app crate — it is what ships —
  // so `--workspace` already links libmpv and needs it present.
  step("rust: fmt", "cargo fmt --all --check", repoRoot);
  step("rust: clippy", "cargo clippy --workspace --all-targets -- -D warnings", repoRoot);
  step("rust: test", "cargo test --workspace", repoRoot);

  // libmpv is no longer optional for the app, so say so up front rather than letting the
  // workspace steps fail several minutes in with a linker error.
  const hasVendoredMpv = existsSync(join(repoRoot, "third_party", "libmpv"));
  const hasSystemMpv = process.platform !== "win32" && have("pkg-config --exists mpv");
  if (!hasVendoredMpv && !hasSystemMpv && !process.env.MPV_LIB_DIR) {
    console.error(
      "\n✗ no libmpv found, and the app crate links it by default — the Rust steps will fail.\n" +
        "  Windows: node scripts/vendor-libmpv.mjs\n" +
        "  macOS:   brew install mpv        Linux: apt install libmpv-dev\n",
    );
    process.exit(1);
  }

  // Mirrors the `engine` job in .github/workflows/ci.yml: the media engine and the native
  // video surface, exercised on the owned crate that still gates them behind a feature.
  step(
    "rust: clippy (engine)",
    "cargo clippy -p freally-player-core --all-targets --features engine-libmpv -- -D warnings",
    repoRoot,
  );
  step(
    "rust: test (engine)",
    "cargo test -p freally-player-core --features engine-libmpv",
    repoRoot,
  );
  // cargo-deny only runs when it's both configured (deny.toml) and installed.
  if (hasDeny && have("cargo deny --version")) {
    step("rust: cargo-deny", "cargo deny check", repoRoot);
  } else if (!hasDeny) {
    console.log("• note: no deny.toml — skipping cargo-deny.");
  } else {
    console.log("• note: cargo-deny not installed — skipping (install: cargo install cargo-deny).");
  }
}

if (!rustOnly && hasUi) {
  // Only run scripts ui/package.json actually defines. Several land in later phases
  // (`i18n:lint` in Phase 10, `test:e2e` with the Playwright visual-smoke gate before 1.0),
  // and `npm run <missing>` fails the whole gate rather than reporting "not yet".
  const uiScripts = JSON.parse(readFileSync(join(uiDir, "package.json"), "utf8")).scripts ?? {};
  const uiStep = (label, script) => {
    if (script in uiScripts) step(`ui: ${label}`, `npm run ${script}`, uiDir);
    else console.log(`• note: ui has no "${script}" script yet — skipping.`);
  };

  uiStep("typecheck", "typecheck");
  uiStep("lint", "lint");
  uiStep("format", "format:check");
  uiStep("test", "test");
  uiStep("i18n:lint", "i18n:lint");
  if (!noE2e) {
    uiStep("e2e", "test:e2e");
  } else {
    console.log("• note: --no-e2e — skipping Playwright e2e.");
  }
} else if (!rustOnly && !hasUi) {
  console.log("• note: no ui/package.json yet — skipping UI checks (they'll run once ./ui lands).");
}

if (steps.length === 0) {
  console.error("ci-local: nothing to run (no Rust/UI detected, or filtered out).");
  process.exit(1);
}

const results = [];
for (const s of steps) {
  const bar = "─".repeat(Math.max(0, 56 - s.name.length));
  console.log(`\n▶ ${s.name} ${bar}`);
  console.log(`  $ ${s.cmd}  (in ${s.cwd === repoRoot ? "." : "ui"})`);
  const started = process.hrtime.bigint();
  const r = spawnSync(s.cmd, { cwd: s.cwd, stdio: "inherit", shell: true });
  const secs = Number((process.hrtime.bigint() - started) / 1_000_000n) / 1000;
  results.push({ name: s.name, ok: r.status === 0, secs });
}

console.log("\n" + "═".repeat(64));
console.log("  Local CI summary");
console.log("═".repeat(64));
let failed = 0;
for (const r of results) {
  const mark = r.ok ? "✓ pass" : "✗ FAIL";
  console.log(`  ${mark}  ${r.name.padEnd(24)} ${r.secs.toFixed(1)}s`);
  if (!r.ok) failed++;
}
console.log("═".repeat(64));

if (failed > 0) {
  console.error(`\n✗ ${failed} check(s) failed — fix before pushing.`);
  process.exit(1);
}
console.log("\n✓ All checks passed. Safe to push.");
