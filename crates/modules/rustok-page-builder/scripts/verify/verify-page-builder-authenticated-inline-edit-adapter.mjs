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
  "crates/modules/rustok-page-builder/contracts/evidence/page-builder-authenticated-inline-edit-adapter-source.json",
));
const flyCargo = read("crates/ui/fly/leptos/Cargo.toml");
const flyRoot = read("crates/ui/fly/leptos/src/root.rs");
const flyFoundation = read("crates/ui/fly/leptos/src/foundation.rs");
const realDom = read("crates/ui/fly/web/src/real_dom_inline.rs");
const storefrontCargo = read("crates/modules/rustok-page-builder/storefront/Cargo.toml");
const storefrontLib = read("crates/modules/rustok-page-builder/storefront/src/lib.rs");
const inline = read("crates/modules/rustok-page-builder/storefront/src/inline_edit.rs");
const pagesStorefrontCargo = read("crates/modules/rustok-pages/storefront/Cargo.toml");
const pagesStorefrontLib = read("crates/modules/rustok-pages/storefront/src/lib.rs");
const plan = read("docs/modules/pages-page-builder-parity-continuation-plan.md");
const actualization = read(
);
const packet = read(
);

if (evidence.format !== "page_builder_authenticated_inline_edit_adapter_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "page_builder_authenticated_inline_edit_adapter_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
, marker, "Page Builder actualization overlay");
, marker, "authenticated inline adapter packet");

for (const text of [realDom, inline, packet, actualization]) {
  forbid(text, "Iggy", "authenticated inline adapter slice");
}

if (failures.length > 0) {
  console.error("[verify-page-builder-authenticated-inline-edit-adapter] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log(
  "[verify-page-builder-authenticated-inline-edit-adapter] PASS adapter_source_ready=true session_dom_exposure=false historical_consumer_boundary=retained execution=pending",
);
