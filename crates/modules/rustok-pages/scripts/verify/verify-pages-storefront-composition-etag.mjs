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
  "crates/modules/rustok-pages/contracts/evidence/pages-storefront-composition-etag-source.json",
));
const routeAdapter = read(
  "crates/modules/rustok-pages/storefront/src/transport/host_route_adapter.rs",
);
const navigationModel = read("crates/modules/rustok-navigation/storefront/src/model.rs");
const navigationLib = read("crates/modules/rustok-navigation/storefront/src/lib.rs");
const navigationUi = read("crates/modules/rustok-navigation/storefront/src/ui/menu.rs");
const composition = read("apps/storefront/src/shared/context/pages_composition.rs");
const host = read("apps/storefront/src/lib.rs");
const cargo = read("apps/storefront/Cargo.toml");
const plan = read("docs/modules/pages-page-builder-parity-continuation-plan.md");
const localPlan = read("crates/modules/rustok-pages/docs/implementation-plan.md");
const packet = read(
);

if (evidence.format !== "pages_storefront_composition_etag_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_storefront_composition_etag_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
, marker, "storefront composition packet");

for (const text of [routeAdapter, navigationModel, navigationUi, composition, host, packet]) {
  forbid(text, "Iggy", "Pages storefront composition slice");
}

if (failures.length > 0) {
  console.error("[verify-pages-storefront-composition-etag] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("[verify-pages-storefront-composition-etag] PASS source_ready=true execution=pending");
