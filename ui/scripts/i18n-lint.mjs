#!/usr/bin/env node
/**
 * Catalog gate — Definition of Done step 6c(a), `prd.md` FR-018.
 *
 * A phase does not close with English-only strings or `TODO` placeholders in a catalog: a
 * partially translated locale is how a shipped language silently degrades into half-English,
 * and nothing else in the build can see it happen. This is the check that can.
 *
 * It fails on:
 *   • a key in en.ftl that some other catalog is missing (the DoD's stated requirement)
 *   • a key in some catalog that en.ftl does not have  (a stale key nobody will ever update)
 *   • a `t("…")` in the source with no such key in en.ftl
 *   • a `t(`prefix-${…}`)` whose prefix matches no key at all
 *   • a translated string whose `{ $placeholders }` differ from the English ones — a dropped
 *     `{ $version }` renders as a missing version, not as an error
 *   • a locale in locales.ts with no catalog file, or a catalog file no locale declares
 *   • a catalog Fluent itself cannot parse
 *
 * What it does NOT do: report keys nothing references. Dynamic families like `status-${…}`
 * make that unreliable, and a wrong "unused" is worse than none.
 */
import { FluentBundle, FluentResource } from "@fluent/bundle";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const uiDir = join(dirname(fileURLToPath(import.meta.url)), "..");
const i18nDir = join(uiDir, "src", "i18n");
const SOURCE_LOCALE = "en";

const problems = [];
const fail = (message) => problems.push(message);

/** The locales the app claims to ship, read from the one list that decides it. */
function declaredLocales() {
  const source = readFileSync(join(i18nDir, "locales.ts"), "utf8");
  const codes = [...source.matchAll(/\{\s*code:\s*"([^"]+)"/g)].map((m) => m[1]);
  if (codes.length === 0) throw new Error("could not read LOCALES from src/i18n/locales.ts");
  return codes;
}

/**
 * The keys and per-key placeholders of one catalog.
 *
 * Keys are read with a line regex, then cross-checked against a real Fluent parse: if the two
 * ever disagree — an attribute, a multiline value, a selector — the regex has stopped being
 * good enough and this says so rather than quietly under-reporting.
 */
function readCatalog(locale) {
  const source = readFileSync(join(i18nDir, `${locale}.ftl`), "utf8");

  const bundle = new FluentBundle(locale, { useIsolating: false });
  for (const error of bundle.addResource(new FluentResource(source))) {
    fail(`${locale}.ftl: Fluent could not parse it — ${error}`);
  }

  const keys = new Map();
  const lines = source.split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    const match = /^([A-Za-z][A-Za-z0-9_-]*)\s*=(.*)$/.exec(lines[i]);
    if (!match) continue;
    const [, key, firstLine] = match;

    if (keys.has(key)) fail(`${locale}.ftl: "${key}" is defined twice`);
    if (!bundle.hasMessage(key)) {
      fail(`${locale}.ftl: "${key}" was read as a message but Fluent does not see one there`);
    }

    // Continuation lines are indented, so a value runs until the next unindented line.
    let value = firstLine;
    while (i + 1 < lines.length && /^\s+\S/.test(lines[i + 1])) value += "\n" + lines[++i];

    const placeholders = [...value.matchAll(/\{\s*\$([A-Za-z][\w-]*)\s*\}/g)].map((m) => m[1]);
    keys.set(key, new Set(placeholders));
  }
  return keys;
}

/** Every key the source actually asks for, split into literals and dynamic prefixes. */
function readSourceKeys() {
  const literals = new Map();
  const prefixes = new Map();

  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) {
        walk(path);
      } else if (/\.tsx?$/.test(entry.name) && !/\.test\.tsx?$/.test(entry.name)) {
        const source = readFileSync(path, "utf8");
        const where = path.slice(uiDir.length + 1).replace(/\\/g, "/");

        for (const m of source.matchAll(/\bt\(\s*["']([^"']+)["']/g)) {
          if (!literals.has(m[1])) literals.set(m[1], where);
        }
        for (const m of source.matchAll(/\bt\(\s*`([^`]*)`/g)) {
          const raw = m[1];
          const prefix = raw.includes("${") ? raw.slice(0, raw.indexOf("${")) : null;
          if (prefix === null) {
            if (!literals.has(raw)) literals.set(raw, where);
          } else if (!prefixes.has(prefix)) {
            prefixes.set(prefix, where);
          }
        }
      }
    }
  };
  walk(join(uiDir, "src"));

  return { literals, prefixes };
}

const locales = declaredLocales();

const onDisk = readdirSync(i18nDir)
  .filter((name) => name.endsWith(".ftl"))
  .map((name) => name.slice(0, -4));

for (const locale of locales) {
  if (!onDisk.includes(locale))
    fail(`locales.ts ships "${locale}" but src/i18n/${locale}.ftl does not exist`);
}
for (const locale of onDisk) {
  if (!locales.includes(locale))
    fail(`src/i18n/${locale}.ftl exists but locales.ts does not ship "${locale}"`);
}

const catalogs = new Map();
for (const locale of locales) {
  if (onDisk.includes(locale)) catalogs.set(locale, readCatalog(locale));
}

const english = catalogs.get(SOURCE_LOCALE);
if (!english) {
  console.error(
    `✗ i18n: the source catalog ${SOURCE_LOCALE}.ftl is missing — nothing to check against.`,
  );
  process.exit(1);
}

for (const [locale, keys] of catalogs) {
  if (locale === SOURCE_LOCALE) continue;

  for (const [key, placeholders] of english) {
    const translated = keys.get(key);
    if (!translated) {
      fail(`${locale}.ftl: missing "${key}" (it is in ${SOURCE_LOCALE}.ftl)`);
      continue;
    }
    for (const name of placeholders) {
      if (!translated.has(name)) fail(`${locale}.ftl: "${key}" drops the { $${name} } placeholder`);
    }
    for (const name of translated) {
      if (!placeholders.has(name)) {
        fail(
          `${locale}.ftl: "${key}" adds a { $${name} } placeholder ${SOURCE_LOCALE}.ftl does not have`,
        );
      }
    }
  }

  for (const key of keys.keys()) {
    if (!english.has(key)) fail(`${locale}.ftl: "${key}" is not in ${SOURCE_LOCALE}.ftl`);
  }
}

const { literals, prefixes } = readSourceKeys();
for (const [key, where] of literals) {
  if (!english.has(key)) fail(`${where}: t("${key}") has no such key in ${SOURCE_LOCALE}.ftl`);
}
for (const [prefix, where] of prefixes) {
  const reached = [...english.keys()].some((key) => key.startsWith(prefix));
  if (!reached) fail(`${where}: t(\`${prefix}\${…}\`) matches no key in ${SOURCE_LOCALE}.ftl`);
}

if (problems.length > 0) {
  console.error(`\n✗ i18n: ${problems.length} problem(s).\n`);
  for (const problem of problems) console.error(`  • ${problem}`);
  console.error(
    "\nEvery locale must carry every key. Translate in the same change, never later.\n",
  );
  process.exit(1);
}

console.log(
  `✓ i18n: ${locales.length} catalogs, ${english.size} keys each — complete and consistent.`,
);
