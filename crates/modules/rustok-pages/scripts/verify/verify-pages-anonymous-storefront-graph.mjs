#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "..", "..", "..", "..", "..");
const failures = [];
const read = (relativePath) => readFileSync(path.join(repoRoot, relativePath), "utf8");
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

const evidencePath =
  "crates/modules/rustok-pages/contracts/evidence/pages-anonymous-storefront-graph-source.json";
const planPath = "docs/modules/pages-page-builder-parity-continuation-plan.md";
const localPlanPath = "crates/modules/rustok-pages/docs/implementation-plan.md";
const pagesManifestPath = "crates/modules/rustok-pages/storefront/Cargo.toml";
const pagesLibPath = "crates/modules/rustok-pages/storefront/src/lib.rs";
const pagesInlineSourcePath = "crates/modules/rustok-pages/storefront/src/inline_edit.rs";
const builderManifestPath = "crates/modules/rustok-page-builder/storefront/Cargo.toml";
const builderLibPath = "crates/modules/rustok-page-builder/storefront/src/lib.rs";
const builderInlineSourcePath = "crates/modules/rustok-page-builder/storefront/src/inline_edit.rs";
const hostManifestPath = "apps/storefront/Cargo.toml";

const evidence = JSON.parse(read(evidencePath));
const plan = read(planPath);
const localPlan = read(localPlanPath);
const pagesManifest = read(pagesManifestPath);
const pagesLib = read(pagesLibPath);
const builderManifest = read(builderManifestPath);
const builderLib = read(builderLibPath);
const hostManifest = read(hostManifestPath);

if (evidence.format !== "pages_anonymous_storefront_graph_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_anonymous_storefront_graph_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
, marker, "anonymous storefront graph packet");
for (const marker of [
  "anonymous-storefront-graph-source-ready",
  "Anonymous storefront authoring exclusion: source-ready",
  "feature-resolved `cargo metadata`",
  "bundle artifact execution remains pending",
  "authenticated inline profiles are opt-in"
]) need(plan, marker, "canonical Pages/Page Builder plan");
for (const marker of [
  "anonymous storefront dependency graph verifier",
  "Compiled SSR/CSR/hydrate bundle artifact evidence remains open",
  "authenticated inline profiles are opt-in"
]) need(localPlan, marker, "Pages local plan");

if (failures.length > 0) {
  console.error("[verify-pages-anonymous-storefront-graph] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log(
  "[verify-pages-anonymous-storefront-graph] PASS graph_profiles=6 authoring_packages=excluded opt_in_inline_profiles=separate bundle_artifact_execution=pending"
);
