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
const featureBody = (manifest, feature, label) => {
  const match = manifest.match(new RegExp(`^${feature}\\s*=\\s*\\[(.*?)\\]`, "ms"));
  if (!match) {
    failures.push(`${label}: missing ${feature} feature`);
    return "";
  }
  return match[1];
};

const evidence = JSON.parse(read(
  "crates/modules/rustok-pages/contracts/evidence/pages-authenticated-inline-consumer-source.json",
));
const grant = read("crates/modules/rustok-pages/src/services/page/inline_edit.rs");
const feature = read("crates/modules/rustok-pages/src/services/page/inline_edit_feature.rs");
const runtime = read("crates/modules/rustok-pages/src/services/page/inline_edit_runtime.rs");
const document = read("crates/modules/rustok-pages/src/services/page/document.rs");
const pageServices = read("crates/modules/rustok-pages/src/services/page/mod.rs");
const services = read("crates/modules/rustok-pages/src/services/mod.rs");
const pagesLib = read("crates/modules/rustok-pages/src/lib.rs");
const storefrontCargo = read("crates/modules/rustok-pages/storefront/Cargo.toml");
const storefrontLib = read("crates/modules/rustok-pages/storefront/src/lib.rs");
const storefront = read("crates/modules/rustok-pages/storefront/src/inline_edit.rs");
const hostCargo = read("apps/storefront/Cargo.toml");
const serverCargo = read("apps/server/Cargo.toml");
const plan = read("docs/modules/pages-page-builder-parity-continuation-plan.md");
const localPlan = read("crates/modules/rustok-pages/docs/implementation-plan.md");
const packet = read(
);

if (evidence.format !== "pages_authenticated_inline_consumer_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_authenticated_inline_consumer_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
, marker, "authenticated inline consumer packet");

for (const text of [grant, runtime, storefront, packet]) {
  forbid(text, "Iggy", "authenticated inline consumer slice");
}

if (failures.length > 0) {
  console.error("[verify-pages-authenticated-inline-consumer] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log(
  "[verify-pages-authenticated-inline-consumer] PASS source_ready=true execution=pending route_mount=open",
);
