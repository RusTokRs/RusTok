#!/usr/bin/env node
// RusTok admin child-page contract guardrails.
//
// A module declares `[[provides.admin_ui.child_pages]]` in its `rustok-module.toml`.
// The host turns every entry into a secondary-navigation link and routes it to the
// same module component — so the *module* owns the subpath dispatch.
//
// Four modules declared child pages and ignored the subpath: every link rendered
// the identical page. A declared surface that does not exist is worse than a
// missing one: it is a control that lies. This gate enforces the contract in both
// directions:
//
//  1. every module that declares child pages has a `*_for_subpath` dispatch in its
//     admin crate (the module, not the host, maps the subpath),
//  2. every declared subpath appears in that dispatch (a declaration cannot rot
//     into a comment),
//  3. the root component reads the routed subpath instead of ignoring it.
//
// `--json` prints the census.

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

function exists(relativePath) {
  return fs.existsSync(path.join(repoRoot, relativePath));
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
        files.push({
          relative: path.relative(repoRoot, absolute).split(path.sep).join("/"),
          source: fs.readFileSync(absolute, "utf8"),
        });
      }
    }
  }
  return files;
}

// `[[provides.admin_ui.child_pages]]` blocks are small; a line scan is enough and
// keeps this gate dependency-free.
function declaredChildPages(manifestPath) {
  const lines = read(manifestPath).split(/\r?\n/);
  const subpaths = [];
  let inBlock = false;
  for (const line of lines) {
    const trimmed = line.trim();
    if (trimmed.startsWith("[") && trimmed.endsWith("]")) {
      inBlock = trimmed === "[[provides.admin_ui.child_pages]]";
      continue;
    }
    if (!inBlock) continue;
    const match = /^subpath\s*=\s*"([^"]+)"\s*$/.exec(trimmed);
    if (match) subpaths.push(match[1]);
  }
  return subpaths;
}

const moduleManifests = fs
  .readdirSync(path.join(repoRoot, "crates/modules"), { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map((entry) => `crates/modules/${entry.name}/rustok-module.toml`)
  .filter((manifest) => exists(manifest));

const report = [];
for (const manifest of moduleManifests) {
  const subpaths = declaredChildPages(manifest);
  if (subpaths.length === 0) continue;

  const moduleDir = path.dirname(manifest);
  const adminRoot = `${moduleDir}/admin/src`;
  if (!exists(adminRoot)) {
    failures.push(`${manifest}: declares ${subpaths.length} child page(s) but has no admin crate`);
    continue;
  }

  const files = collectRustFiles(adminRoot);
  const allSource = files.map((file) => file.source).join("\n");

  // Rule 1: the module — not the host — reads the routed subpath. Any of the
  // idioms the module crates already use counts: the context field, the
  // `subpath()`/`subpath_matches()` helpers, or a `*_for_subpath` dispatch.
  const readsSubpath =
    /route_context\.subpath/.test(allSource) ||
    /\.subpath\s*\(\s*\)/.test(allSource) ||
    /subpath_matches\s*\(/.test(allSource) ||
    /\bfn\s+\w*_for_subpath\s*\(/.test(allSource);
  if (!readsSubpath) {
    failures.push(
      `${adminRoot}: declares child pages (${subpaths.join(", ")}) but never reads the routed ` +
        "subpath — the host sends every child page to this component, so ignoring it renders " +
        "the same markup for every link",
    );
  }

  // Rule 2: every declared subpath is named in the module's own code, so a
  // declaration cannot rot into an unreferenced manifest entry.
  const missing = subpaths.filter((subpath) => !allSource.includes(`"${subpath}"`));
  if (missing.length > 0) {
    failures.push(
      `${adminRoot}: declared child page(s) that the module never mentions: ${missing.join(", ")}`,
    );
  }

  report.push({
    module: path.basename(moduleDir),
    declared: subpaths,
    dispatch: files
      .filter((file) => /\bfn\s+\w*_for_subpath\s*\(/.test(file.source))
      .map((file) => file.relative),
  });
}

if (asJson) {
  console.log(JSON.stringify(report, null, 2));
}

if (failures.length > 0) {
  console.error("Admin child-page verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(1);
}

const declaredTotal = report.reduce((sum, entry) => sum + entry.declared.length, 0);
console.log(
  `Admin child pages passed: ${declaredTotal} declared surface(s) across ${report.length} module(s), ` +
    "each dispatched by its own module and readable from the routed subpath.",
);
