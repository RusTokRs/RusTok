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

console.log("[1/4] Проверка конфликтующих формулировок transport...");
const textFiles = ["docs", "apps", "crates"].flatMap((dir) =>
  listFiles(dir, (filePath) => /\.(md|rs|toml|json|js|mjs|ts|tsx|jsx|sh|txt)$/i.test(filePath)),
);
const conflictPattern = /(replace GraphQL|удал(ить|яем) GraphQL|only \/api\/fn|только \/api\/fn)/;
const conflicts = findMatches(textFiles, conflictPattern);
if (conflicts.length > 0) {
  fail(`Найдены потенциально конфликтующие формулировки:\n${conflicts.join("\n")}`);
}

console.log("[2/4] Проверка, что план содержит обязательные execution-разделы...");
const planPath = "docs/research/dioxus-ffa-ui-migration-plan.md";
const plan = readText(planPath);
for (const marker of [
  "Принцип исполнения backlog",
  "Сверка с текущим кодом",
  "Phase-gate",
  "KPI parity",
  "RACI",
]) {
  if (!plan.includes(marker)) {
    fail(`${planPath}: missing marker ${marker}`);
  }
}

console.log("[2b/4] Проверка anti-over-extraction стандарта FFA-срезов...");
for (const marker of [
  "Стандарт минимального FFA-среза и anti-over-extraction",
  "FFA-срез должен уменьшать связность",
  "request/command construction, normalization и validation",
  "простые i18n label bindings",
  "reset/refresh side effects после mutation",
  "механические wrappers над одной строкой форматирования",
  "Если изменение добавляет больше boilerplate, чем удаляет coupling",
  "если обнаружен over-extraction, откатить его",
]) {
  if (!plan.includes(marker)) {
    fail(`${planPath}: missing anti-over-extraction marker ${marker}`);
  }
}

console.log("[3/4] Поиск Leptos-зависимостей внутри core-слоя (core.rs и core/)...");
const coreFiles = listFiles("crates", (filePath) => {
  const normalized = relativePath(filePath);
  return normalized.endsWith(".rs") && (normalized.endsWith("/core.rs") || normalized.includes("/core/"));
});
const leptosPattern = /use .*leptos|leptos::|leptos_router|leptos_ui_routing|#\[component|#\[server|IntoView|ReadSignal|WriteSignal|Resource</;
const coreHits = findMatches(coreFiles, leptosPattern);
if (coreHits.length > 0) {
  fail(`Найдены Leptos-зависимости в core-слое:\n${coreHits.join("\n")}`);
}

console.log("[4/4] Проверка наличия ссылки на план в docs/index.md...");
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
