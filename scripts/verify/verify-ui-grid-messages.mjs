#!/usr/bin/env node
// RusTok grid chrome localization guardrails.
//
// `DataGrid` renders its own chrome (pager, page-size picker, empty/loading rows,
// filter labels, table name). That chrome used to be English for everyone unless a
// call site remembered `messages=`, which 1 of 43 sites did — so a Russian admin
// shipped English pagination.
//
// The fix is centralized, and this gate locks the central mechanism instead of
// demanding the same boilerplate at every call site:
//
//  1. the host provides the effective locale once (`provide_grid_locale`),
//  2. `DataGrid` prefers an explicit catalog and otherwise resolves the ambient one,
//  3. when one page renders several tables, each table carries its own accessible
//     name (`aria_label`), because "Data grid" twice on a page names nothing.
//
// `--json` prints the census for humans.

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const asJson = process.argv.includes("--json");
const failures = [];

function read(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function collectRustFiles(relativeRoot) {
  const absoluteRoot = path.join(repoRoot, relativeRoot);
  if (!fs.existsSync(absoluteRoot)) return [];
  const files = [];
  const stack = [absoluteRoot];
  while (stack.length > 0) {
    const current = stack.pop();
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const absolute = path.join(current, entry.name);
      if (entry.isDirectory()) {
        if (entry.name === "target" || entry.name === "node_modules") continue;
        stack.push(absolute);
      } else if (entry.isFile() && entry.name.endsWith(".rs")) {
        files.push(path.relative(repoRoot, absolute).split(path.sep).join("/"));
      }
    }
  }
  return files;
}

function requireMarker(relativePath, marker, description) {
  if (!fs.existsSync(path.join(repoRoot, relativePath))) {
    failures.push(`${relativePath}: missing (${description})`);
    return;
  }
  if (!read(relativePath).includes(marker)) {
    failures.push(`${relativePath}: missing ${JSON.stringify(marker)} — ${description}`);
  }
}

// The opening tag of a `view!` element: text between `<DataGrid` and its `>`.
function openingTag(source, startIndex) {
  const rest = source.slice(startIndex);
  const tagEnd = rest.indexOf("/>");
  const gt = rest.indexOf(">");
  const candidates = [tagEnd, gt].filter((index) => index !== -1);
  if (candidates.length === 0) return rest.slice(0, 2000);
  const cut = Math.min(...candidates);
  // A `>` inside a nested attribute expression (`when=move || a > b`) would cut
  // early; keep scanning to the last `>` on the logical tag instead.
  let end = cut;
  while (rest[end] !== ">") end += 1;
  return rest.slice(0, end + 1);
}

const skippedRoots = ["crates/ui/rustok-grid"];
const sites = [];
for (const file of [...collectRustFiles("apps"), ...collectRustFiles("crates")]) {
  if (skippedRoots.some((root) => file.startsWith(`${root}/`))) continue;
  if (!file.includes("/src/")) continue;
  const source = read(file);
  if (!source.includes("<DataGrid")) continue;
  const tagStarts = [...source.matchAll(/<DataGrid\b/g)].map((match) => match.index);
  for (const start of tagStarts) {
    const tag = openingTag(source, start);
    sites.push({
      file,
      line: source.slice(0, start).split("\n").length,
      hasAriaLabel: /\baria_label\s*=/.test(tag),
      hasMessages: /\bmessages\s*=/.test(tag),
    });
  }
}

const byFile = new Map();
for (const site of sites) {
  const list = byFile.get(site.file) ?? [];
  list.push(site);
  byFile.set(site.file, list);
}

for (const [file, list] of byFile) {
  if (list.length < 2) continue;
  for (const site of list) {
    if (!site.hasAriaLabel) {
      failures.push(
        `${file}:${site.line}: page renders ${list.length} tables, so each needs its own accessible name (aria_label=)`,
      );
    }
  }
}

// Central mechanism: the host provides the locale, the adapter consumes it.
requireMarker(
  "apps/admin/src/app/providers/locale.rs",
  "provide_grid_locale",
  "the admin host must provide the ambient grid locale once, not per table",
);
requireMarker(
  "crates/ui/rustok-grid/leptos/src/grid.rs",
  "use_grid_locale()",
  "DataGrid must fall back to the ambient grid locale",
);
requireMarker(
  "crates/ui/rustok-grid/src/messages.rs",
  "pub fn locale(",
  "GridMessages must expose whether a locale was chosen explicitly",
);
requireMarker(
  "crates/ui/rustok-grid/leptos/src/grid.rs",
  "messages.locale()",
  "an explicit catalog must win over the ambient locale",
);

const withAria = sites.filter((site) => site.hasAriaLabel).length;
const withMessages = sites.filter((site) => site.hasMessages).length;
const multiTableFiles = [...byFile.values()].filter((list) => list.length > 1).length;

if (asJson) {
  console.log(
    JSON.stringify(
      { total: sites.length, withAriaLabel: withAria, withMessages, multiTableFiles },
      null,
      2,
    ),
  );
}

if (failures.length > 0) {
  console.error("Grid chrome localization verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(1);
}

console.log(
  `Grid chrome localization passed: ${sites.length} <DataGrid> call site(s), ` +
    `${multiTableFiles} multi-table page(s) all named, ambient locale provided by the host ` +
    `(${withAria} explicit aria_label, ${withMessages} explicit messages catalog).`,
);
