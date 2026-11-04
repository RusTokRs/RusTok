#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
const failures = [];

const evidence = JSON.parse(read(
  "crates/modules/rustok-pages/contracts/evidence/pages-published-slug-route-alias-source.json",
));
const migration = read("crates/modules/rustok-pages/src/migrations/m20260805_000010_create_page_route_aliases.rs");
const migrations = read("crates/modules/rustok-pages/src/migrations/mod.rs");
const entity = read("crates/modules/rustok-pages/src/entities/page_route_alias.rs");
const entities = read("crates/modules/rustok-pages/src/entities/mod.rs");
const route = read("crates/modules/rustok-pages/src/services/page/route.rs");
const metadata = read("crates/modules/rustok-pages/src/services/page/metadata.rs");
const persistence = read("crates/modules/rustok-pages/src/services/page/persistence.rs");
const seo = read("crates/modules/rustok-pages/src/seo_targets.rs");
const regression = read("crates/modules/rustok-pages/tests/page_published_slug_route_alias_sqlite.rs");
const plan = read("docs/modules/pages-page-builder-parity-continuation-plan.md");
const localPlan = read("crates/modules/rustok-pages/docs/implementation-plan.md");

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

if (evidence.format !== "pages_published_slug_route_alias_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_published_slug_route_alias_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
, marker, "route alias packet");

for (const text of [migration, route, metadata, seo, regression]) {
  forbid(text, "Iggy", "Pages routing slice");
}

if (failures.length > 0) {
  console.error("[verify-pages-published-slug-route-alias] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("[verify-pages-published-slug-route-alias] PASS source_ready=true execution=pending");
