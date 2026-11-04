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

const evidence = JSON.parse(read(
  "crates/modules/rustok-pages/contracts/evidence/pages-delete-route-tombstone-source.json",
));
const migration = read(
  "crates/modules/rustok-pages/src/migrations/m20260806_000011_create_page_route_publications.rs",
);
const migrations = read("crates/modules/rustok-pages/src/migrations/mod.rs");
const entity = read("crates/modules/rustok-pages/src/entities/page_route_publication.rs");
const entities = read("crates/modules/rustok-pages/src/entities/mod.rs");
const route = read("crates/modules/rustok-pages/src/services/page/route.rs");
const lifecycle = read("crates/modules/rustok-pages/src/services/page/lifecycle.rs");
const regression = read("crates/modules/rustok-pages/tests/page_delete_route_tombstone_sqlite.rs");
const plan = read("docs/modules/pages-page-builder-parity-continuation-plan.md");

if (evidence.format !== "pages_delete_route_tombstone_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_delete_route_tombstone_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
, marker, "parity actualization");
, marker, "delete tombstone packet");

for (const text of [migration, route, lifecycle, regression]) {
  forbid(text, "Iggy", "Pages route deletion slice");
}

if (failures.length > 0) {
  console.error("[verify-pages-delete-route-tombstone] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("[verify-pages-delete-route-tombstone] PASS source_ready=true execution=pending");
