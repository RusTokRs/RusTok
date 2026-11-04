#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
const failures = [];

const need = (text, marker, label) => {
  if (!text.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (text, marker, label) => {
  if (text.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const ordered = (text, markers, label) => {
  let previous = -1;
  for (const marker of markers) {
    const index = text.indexOf(marker, previous + 1);
    if (index < 0) {
      failures.push(`${label}: missing or out of order ${marker}`);
      return;
    }
    previous = index;
  }
};
const between = (text, start, end, label) => {
  const from = text.indexOf(start);
  const to = from < 0 ? -1 : text.indexOf(end, from + start.length);
  if (from < 0 || to < 0) {
    failures.push(`${label}: unable to locate source slice`);
    return "";
  }
  return text.slice(from, to);
};

const evidence = JSON.parse(read(
  "crates/modules/rustok-pages/contracts/evidence/pages-route-history-import-source.json",
));
const migration = read(
  "crates/modules/rustok-pages/src/migrations/m20260806_000012_create_page_route_history_imports.rs",
);
const migrations = read("crates/modules/rustok-pages/src/migrations/mod.rs");
const entity = read("crates/modules/rustok-pages/src/entities/page_route_history_import.rs");
const entities = read("crates/modules/rustok-pages/src/entities/mod.rs");
const service = read("crates/modules/rustok-pages/src/services/page/route_history_import.rs");
const pageServices = read("crates/modules/rustok-pages/src/services/page/mod.rs");
const services = read("crates/modules/rustok-pages/src/services/mod.rs");
const pagesLib = read("crates/modules/rustok-pages/src/lib.rs");
const regression = read("crates/modules/rustok-pages/tests/page_route_history_import_sqlite.rs");
const plan = read("docs/modules/pages-page-builder-parity-continuation-plan.md");
const localPlan = read("crates/modules/rustok-pages/docs/implementation-plan.md");
const packet = read(
);

if (evidence.format !== "pages_route_history_import_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_route_history_import_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
, marker, "route history import packet");

for (const text of [migration, service, regression, packet]) {
  forbid(text, "Iggy", "Pages route history import slice");
}

if (failures.length > 0) {
  console.error("[verify-pages-route-history-import] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("[verify-pages-route-history-import] PASS source_ready=true execution=pending mode=explicit_import");
