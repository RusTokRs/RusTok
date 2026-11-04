#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), "utf8");
const failures = [];
const need = (text, marker, label) => {
  if (!text.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (text, marker, label) => {
  if (text.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
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
  "crates/modules/rustok-pages/contracts/evidence/pages-authenticated-authoring-route-source.json",
));
const assetEvidence = JSON.parse(read(
  "crates/modules/rustok-pages/contracts/evidence/pages-inline-edit-asset-delivery-source.json",
));
const storefrontCore = read("apps/storefront/src/modules/core.rs");
const storefrontCargo = read("apps/storefront/Cargo.toml");
const storefrontBuild = read("apps/storefront/build.rs");
const bootstrap = read("apps/storefront/public/assets/pages-inline-edit-bootstrap.js");
const builder = [
  read("apps/storefront/scripts/build-pages-inline-edit-client.mjs"),
  read("apps/storefront/scripts/build-wasm-client.mjs"),
].join("\n");
const auth = read("apps/server/src/middleware/auth_context.rs");
const serverCargo = read("apps/server/Cargo.toml");
const consumer = read("crates/modules/rustok-pages/storefront/src/inline_edit.rs");
const canonicalPlan = read("docs/modules/pages-page-builder-parity-continuation-plan.md");
const localPlan = read("crates/modules/rustok-pages/docs/implementation-plan.md");
const packet = read(
);

if (evidence.format !== "pages_authenticated_authoring_route_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_authenticated_authoring_route_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
, marker, "authenticated authoring route packet");

if (failures.length > 0) {
  console.error("[verify-pages-authenticated-authoring-route] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log(
  "[verify-pages-authenticated-authoring-route] PASS route_source_ready=true asset_delivery_source_ready=true execution=pending browser=pending",
);
