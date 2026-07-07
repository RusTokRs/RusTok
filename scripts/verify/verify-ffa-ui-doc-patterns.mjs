#!/usr/bin/env node
// Cross-platform FFA UI doc/pattern checks.

import { readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const failures = [];

function fail(message) {
  failures.push(message);
}

function readText(relativePath) {
  return readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function listFiles(relativeDir, predicate = () => true) {
  const root = path.join(repoRoot, relativeDir);
  const result = [];
  const skippedDirectories = new Set([
    ".git",
    ".next",
    "dist",
    "node_modules",
    "target",
    "test-results",
  ]);

  function walk(directory) {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const fullPath = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        if (skippedDirectories.has(entry.name)) {
          continue;
        }
        walk(fullPath);
        continue;
      }
      if (entry.isFile() && predicate(fullPath)) {
        result.push(fullPath);
      }
    }
  }

  if (statSync(root).isDirectory()) {
    walk(root);
  }

  return result;
}

function relativePath(fullPath) {
  return path.relative(repoRoot, fullPath).replace(/\\/g, "/");
}

function findMatches(files, pattern) {
  const matches = [];
  for (const filePath of files) {
    const text = readFileSync(filePath, "utf8");
    text.split(/\r?\n/).forEach((line, index) => {
      if (pattern.test(line)) {
        matches.push(`${relativePath(filePath)}:${index + 1}:${line}`);
      }
    });
  }
  return matches;
}

console.log("[1/4] Checking conflicting transport wording...");
const textFiles = ["docs", "apps", "crates"].flatMap((dir) =>
  listFiles(dir, (filePath) => /\.(md|rs|toml|json|js|mjs|ts|tsx|jsx|sh|txt)$/i.test(filePath)),
);
const conflictPattern = /(only \/api\/fn|only \/api\/graphql)/i;
const conflicts = findMatches(textFiles, conflictPattern);
if (conflicts.length > 0) {
  fail(`Found potentially conflicting transport wording:\n${conflicts.join("\n")}`);
}

console.log("[2/4] Checking that the plan contains required execution sections...");
const planPath = "docs/research/dioxus-ffa-ui-migration-plan.md";
const plan = readText(planPath);
for (const marker of [
  "Backlog Execution Principle",
  "Reconciliation with Current Code",
  "Phase-Gate",
  "KPI Parity",
  "RACI",
]) {
  if (!plan.includes(marker)) {
    fail(`${planPath}: missing marker ${marker}`);
  }
}

console.log("[2b/4] Checking the anti-over-extraction standard for FFA slices...");
for (const marker of [
  "Standard for minimal FFA slice and anti-over-extraction",
  "An FFA slice should reduce coupling",
  "request/command construction, normalization and validation",
  "simple i18n label bindings",
  "reset/refresh side effects after mutation",
  "mechanical wrappers over a single formatting line",
  "If a change adds more boilerplate than it removes coupling",
  "if over-extraction is detected, revert it",
]) {
  if (!plan.includes(marker)) {
    fail(`${planPath}: missing anti-over-extraction marker ${marker}`);
  }
}

console.log("[3/5] Searching for Leptos dependencies inside the core layer (core.rs and core/)...");
const coreFiles = listFiles("crates", (filePath) => {
  const normalized = relativePath(filePath);
  return normalized.endsWith(".rs") && (normalized.endsWith("/core.rs") || normalized.includes("/core/"));
});
const leptosPattern = /use .*leptos|leptos::|leptos_router|leptos_ui_routing|#\[component|#\[server|IntoView|ReadSignal|WriteSignal|Resource</;
const coreHits = findMatches(coreFiles, leptosPattern);
if (coreHits.length > 0) {
  fail(`Found Leptos dependencies inside the core layer:\n${coreHits.join("\n")}`);
}

console.log("[4/5] Checking UI i18n ownership boundaries...");
const moduleI18nFiles = listFiles("crates", (filePath) => {
  const normalized = relativePath(filePath);
  return normalized.endsWith("/src/i18n.rs") && /\/(admin|storefront)\/src\/i18n\.rs$/.test(normalized);
});
const rustokApiI18nPattern = /rustok_api::.*(build_ui_message_catalog|resolve_ui_message|UiMessageCatalog)|rustok_api::build_ui_message_catalog/;
const rustokApiI18nHits = findMatches(moduleI18nFiles, rustokApiI18nPattern);
if (rustokApiI18nHits.length > 0) {
  fail(`Module UI i18n files must not import UI i18n helpers from rustok_api:\n${rustokApiI18nHits.join("\n")}`);
}

const docsFiles = listFiles("docs", (filePath) => /\.(md|mjs|js)$/i.test(filePath)).concat(
  listFiles("apps", (filePath) => /AI_AGENT_RULES\.md$|docs[\\/].*\.md$/i.test(filePath)),
);
const staleRustokApiI18nDocsPattern = /rustok_api::build_ui_message_catalog|rustok_api` pattern|rustok-api.*re-exports UI i18n|temporary re-exports for `rustok-ui-i18n`/;
const staleRustokApiI18nDocsHits = findMatches(docsFiles, staleRustokApiI18nDocsPattern);
if (staleRustokApiI18nDocsHits.length > 0) {
  fail(`Documentation must not describe rustok_api as the UI i18n owner:\n${staleRustokApiI18nDocsHits.join("\n")}`);
}

console.log("[5/5] Checking that docs/index.md links to the plan...");
if (!readText("docs/index.md").includes("dioxus-ffa-ui-migration-plan")) {
  fail("docs/index.md: missing dioxus-ffa-ui-migration-plan link");
}

if (failures.length > 0) {
  console.error("[verify-ffa-ui-doc-patterns] FAIL");
  for (const failure of failures) {
    console.error(failure);
  }
  process.exit(1);
}

console.log("OK: FFA UI doc/pattern checks passed");
