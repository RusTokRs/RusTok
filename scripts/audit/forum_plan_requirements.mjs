#!/usr/bin/env node

/**
 * Which canonical-plan markers do the forum source verifiers demand?
 *
 * Many `scripts/verify/verify-forum-*.mjs` guards pin rows of
 * `crates/modules/rustok-forum/docs/implementation-plan.md`, and each guard spells its variable
 * differently (`plan`, `forumPlan`, `paths.forumPlan`, a `requireText` loop, ...). Running them one at a
 * time to discover what the family expects is slow and hides the union, so this audit resolves the plan
 * variable inside every guard, collects the markers that guard requires from it, and reports which of
 * those markers the plan does not currently contain.
 *
 * Read-only: it edits nothing. `node scripts/audit/forum_plan_requirements.mjs
 * [--plan crates/modules/rustok-search/docs/implementation-plan.md]`.


 */

import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { containsMarker } from "../verify/lib/forum-source-verification.mjs";

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const planArgument = process.argv.indexOf("--plan");
const planRelative =
  planArgument >= 0 && process.argv[planArgument + 1]
    ? process.argv[planArgument + 1]
    : "crates/modules/rustok-forum/docs/implementation-plan.md";
const verifierDir = "scripts/verify";

const read = (relative) => {
  const absolute = path.join(repoRoot, relative);
  return existsSync(absolute) ? readFileSync(absolute, "utf8") : "";
};

const escapeRegExp = (text) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** The paths-map keys of one guard that resolve to the forum canonical plan. */
function planPathKeys(source) {
  const keys = new Set();
  for (const match of source.matchAll(/(\w+):\s*"([^"]+)"/g)) {
    if (match[2] === planRelative) keys.add(match[1]);
  }
  return [...keys];
}

/** Identifiers in one guard that hold the forum canonical plan text. */
function planVariables(source) {
  const names = new Set();
  const literal = escapeRegExp(`"${planRelative}"`);
  // `const planPath = "crates/.../implementation-plan.md"; const plan = read(planPath);`
  for (const match of source.matchAll(new RegExp(`const\\s+(\\w+)\\s*=\\s*${literal}\\s*[;,\\n]`, "g"))) {
    names.add(match[1]);
  }
  // `forumPlan: "crates/.../implementation-plan.md"` in a paths map, read as `read(paths.forumPlan)`.
  const keys = planPathKeys(source);
  for (const match of source.matchAll(/const\s+(\w+)\s*=\s*read\w*\(\s*(?:[A-Za-z]+,\s*)?paths\.(\w+)\s*\)/g)) {
    if (keys.includes(match[2])) names.add(match[1]);
  }
  const pathVars = [...names].filter((name) => name.endsWith("Path"));
  const alternatives = pathVars.map(escapeRegExp).join("|");
  if (alternatives) {
    for (const match of source.matchAll(
      new RegExp(`const\\s+(\\w+)\\s*=\\s*read\\w*\\(\\s*(?:[A-Za-z]+,\\s*)?(${alternatives})\\s*\\)`, "g"),
    )) {
      names.add(match[1]);
    }
  }
  return [...names].filter((name) => !name.endsWith("Path"));
}

/** String or regular-expression literals inside a marker array. */
function markerLiterals(block) {
  const literals = [];
  for (const match of block.matchAll(/"((?:[^"\\]|\\.)*)"|(\/(?:[^/\n\\]|\\.)*\/[a-z]*)/g)) {
    if (match[2]) {
      const flags = match[2].match(/[a-z]+$/)?.[0] ?? "";
      literals.push(new RegExp(match[2].slice(1, match[2].length - 1 - flags.length), flags));
    } else {
      literals.push(match[1]);
    }
  }
  return literals;
}

/** Markers the guard requires from any of the resolved plan variables. */
function requiredMarkers(source, names) {
  const alternatives = names.map(escapeRegExp).join("|");
  const markers = [];
  const patterns = [
    // requireAll(plan, [ ... ], label)
    new RegExp(`requireAll\\(\\s*(?:${alternatives})(?:\\.text)?\\s*,\\s*\\[([^\\[\\]]*?)\\]\\s*,[\\s\\S]{0,120}?\\);`, "g"),
    // for (const marker of [ ... ]) { requireText(plan, marker, ...) }
    // The loop body has to be exactly the `requireText` call for this variable, otherwise a marker list
    // that belongs to another file in the same guard would be attributed to the plan.
    new RegExp(
      `for\\s*\\(\\s*const marker of \\[([^\\[\\]]*?)\\]\\s*\\)\\s*\\{\\s*(?://[^\\n]*\\n\\s*)*requireText\\(\\s*(?:${alternatives})\\b`,
      "g",
    ),
    // requireText(plan, "marker", ...)
    new RegExp(
      `requireText\\(\\s*(?:${alternatives})\\s*,\\s*("(?:[^"\\\\]|\\\\.)*"|\\/(?:[^/\\\\\\n]|\\\\.)*\\/[a-z]*)\\s*,`,
      "g",
    ),
  ];
  for (const pattern of patterns) {
    for (const match of source.matchAll(pattern)) markers.push(...markerLiterals(match[1]));
  }
  return [...new Set(markers)];
}

const plan = read(planRelative);
const missing = [];
const inspected = [];
for (const entry of readdirSync(path.join(repoRoot, verifierDir)).sort()) {
  if (!entry.startsWith("verify-forum-") || !entry.endsWith(".mjs")) continue;
  const source = read(path.join(verifierDir, entry));
  if (!source.includes(planRelative)) continue;
  const names = planVariables(source);
  if (names.length === 0) continue;
  const markers = requiredMarkers(source, names);
  inspected.push(`${entry} (${names.join(", ")}): ${markers.length} marker(s)`);
  for (const marker of markers) {
    if (!containsMarker(plan, marker)) missing.push({ verifier: entry, marker });
  }
}

console.log(`Guards inspected whose plan variable was resolved: ${inspected.length}`);
for (const line of inspected) console.log(`  ${line}`);
if (missing.length === 0) {
  console.log(`\nEvery forum verifier marker pinned to ${planRelative} is present.`);
} else {
  console.log(`\nMissing markers in ${planRelative}:`);
  for (const { verifier, marker } of missing) console.log(`- ${verifier}: ${String(marker)}`);
  console.log(`\n${missing.length} unresolved marker(s).`);
  process.exitCode = 1;
}
