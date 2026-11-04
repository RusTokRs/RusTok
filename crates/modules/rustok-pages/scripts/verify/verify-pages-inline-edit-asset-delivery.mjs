#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), "utf8");
const need = (text, marker, label) => {
  if (!text.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (text, marker, label) => {
  if (text.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const featureBody = (manifest, feature, label) => {
  const escaped = feature.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = manifest.match(new RegExp(`^${escaped}\\s*=\\s*\\[(.*?)\\]`, "ms"));
  if (!match) {
    failures.push(`${label}: missing ${feature} feature`);
    return "";
  }
  return match[1];
};

const files = {
  evidence: "crates/modules/rustok-pages/contracts/evidence/pages-inline-edit-asset-delivery-source.json",
  pagesCargo: "crates/modules/rustok-pages/Cargo.toml",
  pagesHttp: "crates/modules/rustok-pages/src/http.rs",
  assets: "crates/modules/rustok-pages/src/http/inline_edit_assets.rs",
  serverCargo: "apps/server/Cargo.toml",
  clientBuilder: "apps/storefront/scripts/build-pages-inline-edit-client.mjs",
  serverBuilder: "scripts/build/build-pages-inline-edit-server.sh",
  localPlan: "crates/modules/rustok-pages/docs/implementation-plan.md",
  plan: "docs/modules/pages-page-builder-parity-continuation-plan.md",
};
for (const [label, relativePath] of Object.entries(files)) {
  if (!fs.existsSync(path.join(root, relativePath))) failures.push(`${label}: missing ${relativePath}`);
}
if (failures.length > 0) {
  console.error("[verify-pages-inline-edit-asset-delivery] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

const evidence = JSON.parse(read(files.evidence));
const pagesCargo = read(files.pagesCargo);
const pagesHttp = read(files.pagesHttp);
const assets = read(files.assets);
const serverCargo = read(files.serverCargo);
const clientBuilder = read(files.clientBuilder);
const serverBuilder = read(files.serverBuilder);
const localPlan = read(files.localPlan);
const plan = read(files.plan);

if (evidence.format !== "pages_inline_edit_asset_delivery_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_inline_edit_asset_delivery_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("source evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
, marker, "asset delivery packet");

if (failures.length > 0) {
  console.error("[verify-pages-inline-edit-asset-delivery] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log(
  "[verify-pages-inline-edit-asset-delivery] PASS source_ready=true execution=pending release_integration=open admin_launch=open",
);
