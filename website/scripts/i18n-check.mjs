#!/usr/bin/env node
// i18n parity check for the Gaze landing page.
// Asserts structural equivalence between the English tree (root) and the Japanese tree (/ja/).
// Runs in CI; fails the build on drift.
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const WEBSITE = new URL("..", import.meta.url).pathname;
const EN_ROOT = WEBSITE;
const JA_ROOT = join(WEBSITE, "ja");

const REQUIRED_PAGES = [
  "index.html",
  "privacy.html",
  "terms.html",
  "changelog.html",
  "roadmap.html",
];

let failures = 0;
function fail(msg) {
  console.error(`FAIL: ${msg}`);
  failures += 1;
}

// 1. Every required page exists in both trees.
for (const page of REQUIRED_PAGES) {
  for (const [label, root] of [
    ["en", EN_ROOT],
    ["ja", JA_ROOT],
  ]) {
    const p = join(root, page);
    try {
      statSync(p);
    } catch {
      fail(`missing ${label} page: ${relative(WEBSITE, p)}`);
    }
  }
}

// 2. Per-page structural checks.
for (const page of REQUIRED_PAGES) {
  for (const [lang, root] of [
    ["en", EN_ROOT],
    ["ja", JA_ROOT],
  ]) {
    const path = join(root, page);
    let src;
    try {
      src = readFileSync(path, "utf8");
    } catch {
      continue; // already reported above
    }
    const rel = relative(WEBSITE, path);

    // html lang attribute matches expected
    const langMatch = src.match(/<html\s+lang="([^"]+)"/);
    if (!langMatch) {
      fail(`${rel}: <html lang="..."> missing`);
    } else if (langMatch[1] !== lang) {
      fail(`${rel}: <html lang="${langMatch[1]}">, expected "${lang}"`);
    }

    // canonical present
    if (!/rel="canonical"/.test(src)) {
      fail(`${rel}: missing <link rel="canonical">`);
    }

    // all three hreflang alternates present
    for (const hl of ["en", "ja", "x-default"]) {
      const re = new RegExp(`rel="alternate"[^>]*hreflang="${hl}"`);
      if (!re.test(src)) {
        fail(`${rel}: missing hreflang="${hl}" alternate`);
      }
    }

    // language switcher present
    if (!/class="nav-lang"/.test(src)) {
      fail(`${rel}: missing .nav-lang switcher`);
    }

    // og:locale matches the language
    const expectedLocale = lang === "ja" ? "ja_JP" : "en_US";
    const re = new RegExp(
      `<meta property="og:locale" content="${expectedLocale}"`,
    );
    if (!re.test(src)) {
      fail(`${rel}: missing or wrong og:locale (expected ${expectedLocale})`);
    }
  }
}

// 3. No Japanese characters in EN tree (outside /ja/). Catches translation drift.
const CJK = /[぀-ゟ゠-ヿ一-龯]/;
for (const page of REQUIRED_PAGES) {
  const path = join(EN_ROOT, page);
  let src;
  try {
    src = readFileSync(path, "utf8");
  } catch {
    continue;
  }
  const lines = src.split("\n");
  lines.forEach((line, i) => {
    if (CJK.test(line)) {
      fail(`${page}:${i + 1}: Japanese character in EN file: ${line.trim()}`);
    }
  });
}

if (failures > 0) {
  console.error(`\ni18n-check: ${failures} failure(s)`);
  process.exit(1);
}
console.log(`i18n-check: OK (${REQUIRED_PAGES.length} pages × 2 langs)`);
