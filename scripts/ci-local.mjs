#!/usr/bin/env node
// Local CI — run the SAME checks you'd want green before pushing.
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
import { existsSync } from "node:fs";
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
  step("rust: fmt", "cargo fmt --all --check", repoRoot);
  step("rust: clippy", "cargo clippy --workspace --all-targets --all-features -- -D warnings", repoRoot);
  step("rust: test", "cargo test --workspace --all-features", repoRoot);
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
  step("ui: typecheck", "npm run typecheck", uiDir);
  step("ui: lint", "npm run lint", uiDir);
  step("ui: test", "npm run test", uiDir);
  step("ui: i18n:lint", "npm run i18n:lint", uiDir);
  if (!noE2e) {
    step("ui: e2e", "npm run test:e2e", uiDir);
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
