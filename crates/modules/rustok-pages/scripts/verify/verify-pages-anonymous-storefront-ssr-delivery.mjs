#!/usr/bin/env node

import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "..", "..", "..", "..", "..");
const failures = [];

const files = {
  evidence:
    "crates/modules/rustok-pages/contracts/evidence/pages-anonymous-storefront-ssr-delivery-source.json",
  graphEvidence:
    "crates/modules/rustok-pages/contracts/evidence/pages-anonymous-storefront-graph-source.json",
  authoringEvidence:
    "crates/modules/rustok-pages/contracts/evidence/pages-authenticated-authoring-route-source.json",
  graphVerifier:
    "crates/modules/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-graph.mjs",
  inspector:
    "crates/modules/rustok-pages/scripts/verify/inspect-pages-anonymous-storefront-ssr-artifact.mjs",
  plan: "docs/modules/pages-page-builder-parity-continuation-plan.md",
  cargo: "apps/storefront/Cargo.toml",
  host: "apps/storefront/src/lib.rs",
  authoringCore: "apps/storefront/src/modules/core.rs",
  regression: "apps/storefront/tests/pages_anonymous_ssr_delivery.rs",
};

const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => readFileSync(absolute(relativePath), "utf8");
const need = (text, marker, label) => {
  if (!text.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (text, marker, label) => {
  if (text.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const between = (text, start, end, label) => {
  const from = text.indexOf(start);
  if (from < 0) {
    failures.push(`${label}: missing start ${start}`);
    return "";
  }
  const to = text.indexOf(end, from + start.length);
  if (to < 0) {
    failures.push(`${label}: missing end ${end}`);
    return "";
  }
  return text.slice(from, to);
};

for (const [label, relativePath] of Object.entries(files)) {
  if (!existsSync(absolute(relativePath))) failures.push(`${label}: missing ${relativePath}`);
}
if (failures.length > 0) {
  console.error("[verify-pages-anonymous-storefront-ssr-delivery] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

const evidence = JSON.parse(read(files.evidence));
const graphEvidence = JSON.parse(read(files.graphEvidence));
const authoringEvidence = JSON.parse(read(files.authoringEvidence));
const cargo = read(files.cargo);
const host = read(files.host);
const authoringCore = read(files.authoringCore);
const regression = read(files.regression);
const inspector = read(files.inspector);
const plan = read(files.plan);

if (evidence.format !== "pages_anonymous_storefront_ssr_delivery_source_v1") {
  failures.push(`historical evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_anonymous_storefront_ssr_delivery_source_unvalidated") {
  failures.push(`historical evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("historical source evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`historical validation.${key} must remain false`);
}
if (authoringEvidence.status !== "pages_authenticated_authoring_route_source_unvalidated") {
  failures.push(`authoring evidence status mismatch: ${authoringEvidence.status}`);
}
if (authoringEvidence.source_contract?.anonymous_pages_route_is_unchanged !== true) {
  failures.push("authoring evidence must retain unchanged anonymous Pages route");
}
if (
  authoringEvidence.source_contract
    ?.anonymous_default_csr_hydrate_ssr_profiles_remain_without_inline_edit !== true
) {
  failures.push("authoring evidence must retain anonymous profile exclusion");
}
if (authoringEvidence.source_contract?.built_client_artifact_produced !== false) {
  failures.push("authoring evidence must not claim a built client artifact");
}

if (graphEvidence.status !== "pages_anonymous_storefront_graph_source_unvalidated") {
  failures.push(`linked graph evidence status mismatch: ${graphEvidence.status}`);
}
if (graphEvidence.source_contract?.host_client_profiles_keep_optional_pages_module_disabled !== true) {
  failures.push("linked graph evidence must retain disabled Pages host client profiles");
}

, marker, "SSR delivery packet");
}
for (const marker of [
  "anonymous-storefront-ssr-delivery-source-ready",
  "Anonymous storefront SSR delivery: source-ready",
  "public Pages route remains SSR-only",
  "authenticated inline profiles are opt-in",
  "client artifact build and browser execution remain pending"
]) {
  need(plan, marker, "canonical Pages/Page Builder plan");
}

if (failures.length > 0) {
  console.error("[verify-pages-anonymous-storefront-ssr-delivery] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log(
  "[verify-pages-anonymous-storefront-ssr-delivery] PASS public_ssr_only=true authenticated_client=gated artifact_execution=pending",
);
