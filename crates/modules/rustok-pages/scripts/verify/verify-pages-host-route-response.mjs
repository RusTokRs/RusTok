#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
const failures = [];

const evidence = JSON.parse(read(
  "crates/modules/rustok-pages/contracts/evidence/pages-host-route-response-source.json",
));
const historicalEvidence = JSON.parse(read(
  "crates/modules/rustok-pages/contracts/evidence/pages-published-slug-route-alias-source.json",
));
const pagesLib = read("crates/modules/rustok-pages/src/lib.rs");
const adapter = read(
  "crates/modules/rustok-pages/storefront/src/transport/host_route_adapter.rs",
);
const transport = read("crates/modules/rustok-pages/storefront/src/transport/mod.rs");
const storefrontLib = read("crates/modules/rustok-pages/storefront/src/lib.rs");
const host = read("apps/storefront/src/lib.rs");
const harness = read(
  "crates/modules/rustok-pages/storefront/tests/host_route_decision_sqlite.rs",
);
const plan = read("docs/modules/pages-page-builder-parity-continuation-plan.md");
const localPlan = read("crates/modules/rustok-pages/docs/implementation-plan.md");
const packet = read(
);

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

if (evidence.format !== "pages_host_route_response_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_host_route_response_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
, marker, "host route response packet");

for (const text of [adapter, host, harness, packet]) {
  forbid(text, "Iggy", "Pages host routing slice");
}

if (failures.length > 0) {
  console.error("[verify-pages-host-route-response] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("[verify-pages-host-route-response] PASS source_ready=true execution=pending");
