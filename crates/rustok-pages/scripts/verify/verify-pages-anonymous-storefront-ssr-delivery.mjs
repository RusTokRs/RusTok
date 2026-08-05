#!/usr/bin/env node

import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "..", "..", "..", "..");
const failures = [];

const files = {
  evidence:
    "crates/rustok-pages/contracts/evidence/pages-anonymous-storefront-ssr-delivery-source.json",
  graphEvidence:
    "crates/rustok-pages/contracts/evidence/pages-anonymous-storefront-graph-source.json",
  graphVerifier:
    "crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-graph.mjs",
  inspector:
    "crates/rustok-pages/scripts/verify/inspect-pages-anonymous-storefront-ssr-artifact.mjs",
  packet:
    "docs/modules/pages-page-builder-anonymous-storefront-ssr-delivery-packet-2026-08-05.md",
  plan: "docs/modules/pages-page-builder-parity-continuation-plan.md",
  cargo: "apps/storefront/Cargo.toml",
  host: "apps/storefront/src/lib.rs",
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
const cargo = read(files.cargo);
const host = read(files.host);
const regression = read(files.regression);
const inspector = read(files.inspector);
const packet = read(files.packet);
const plan = read(files.plan);

if (evidence.format !== "pages_anonymous_storefront_ssr_delivery_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_anonymous_storefront_ssr_delivery_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("source evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
for (const key of [
  "current_pages_public_host_is_ssr_rendered",
  "host_library_crate_type_is_rlib",
  "host_csr_profile_keeps_pages_storefront_disabled",
  "host_hydrate_profile_keeps_pages_storefront_disabled",
  "host_ssr_profile_enables_pages_storefront",
  "rendered_document_has_no_executable_client_script_source",
  "rendered_document_has_no_module_script",
  "rendered_document_has_no_module_preload",
  "storefront_source_has_no_wasm_start_entrypoint",
  "storefront_source_has_no_mount_to_body_entrypoint",
  "storefront_source_has_no_hydrate_body_entrypoint",
  "source_regression_added",
  "artifact_inspector_added",
  "artifact_inspector_requires_explicit_artifact_paths",
  "artifact_inspector_records_sha256",
  "artifact_inspector_rejects_pages_admin",
  "artifact_inspector_rejects_page_builder_admin",
  "artifact_inspector_rejects_fly_authoring_packages",
  "artifact_inspector_rejects_editor_composition_markers",
  "json_ld_structured_data_remains_allowed",
  "client_bundle_proof_is_not_claimed",
  "client_bundle_gate_reopens_when_pages_client_bootstrap_is_added"
]) {
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`source_contract.${key} must be true`);
  }
}
for (const key of [
  "production_pages_behavior_changed",
  "production_page_builder_behavior_changed",
  "production_storefront_behavior_changed",
  "dependencies_changed",
  "features_changed",
  "database_schema_changed",
  "public_route_changed",
  "cache_policy_changed",
  "optional_event_infrastructure_changed",
  "ffa_promoted",
  "fba_promoted"
]) {
  if (evidence.source_contract?.[key] !== false) {
    failures.push(`source_contract.${key} must be false`);
  }
}

if (graphEvidence.status !== "pages_anonymous_storefront_graph_source_unvalidated") {
  failures.push(`linked graph evidence status mismatch: ${graphEvidence.status}`);
}
if (graphEvidence.source_contract?.host_client_profiles_keep_optional_pages_module_disabled !== true) {
  failures.push("linked graph evidence must retain disabled Pages host client profiles");
}

for (const marker of [
  'crate-type = ["rlib"]',
  'csr = ["leptos/csr", "leptos_i18n/csr"]',
  'hydrate = ["leptos/hydrate", "leptos_i18n/hydrate"]',
  '"dep:rustok-pages-storefront"',
  '"rustok-pages-storefront/ssr"',
  'rustok-pages-storefront = { path = "../../crates/rustok-pages/storefront", default-features = false, optional = true }'
]) {
  need(cargo, marker, "storefront Cargo contract");
}
const csrFeature = between(cargo, 'csr = [', ']\nhydrate = [', "CSR feature");
const hydrateFeature = between(cargo, 'hydrate = [', ']\nssr = [', "hydrate feature");
for (const feature of [csrFeature, hydrateFeature]) {
  forbid(feature, "rustok-pages-storefront", "host client feature must not enable Pages");
}

const renderDocument = between(
  host,
  "fn render_document(",
  "#[cfg(feature = \"ssr\")]\nasync fn enabled_modules_or_empty",
  "SSR document renderer",
);
for (const marker of [
  '<link rel="stylesheet" href="/assets/app.css" />',
  '<div id="app">{app_html}</div>',
  "{extra_head}"
]) {
  need(renderDocument, marker, "SSR document renderer");
}
for (const marker of [
  "<script src=",
  '<script type="module"',
  'rel="modulepreload"',
  ".wasm",
  "/pkg/",
  "hydrate_body",
  "mount_to_body"
]) {
  forbid(renderDocument, marker, "SSR document renderer");
}

function rustFiles(root) {
  const result = [];
  const pending = [absolute(root)];
  while (pending.length > 0) {
    const current = pending.pop();
    for (const entry of readdirSync(current)) {
      const item = path.join(current, entry);
      const stat = statSync(item);
      if (stat.isDirectory()) pending.push(item);
      else if (entry.endsWith(".rs")) result.push(item);
    }
  }
  return result;
}
for (const file of rustFiles("apps/storefront/src")) {
  const source = readFileSync(file, "utf8");
  const label = path.relative(repoRoot, file);
  for (const marker of [
    "#[wasm_bindgen(start)]",
    "wasm_bindgen(start)",
    "mount_to_body(",
    "hydrate_body(",
    "leptos::mount::mount_to_body",
    "leptos::mount::hydrate_body"
  ]) {
    forbid(source, marker, label);
  }
}

for (const marker of [
  "const HOST_SOURCE: &str = include_str!(\"../src/lib.rs\")",
  "fn render_document_source()",
  "anonymous_pages_host_source_has_no_executable_client_bootstrap",
  'document.contains("<!DOCTYPE html>")',
  'document.contains("<div id=\\\"app\\\">{app_html}</div>")',
  'document.contains("<link rel=\\\"stylesheet\\\" href=\\\"/assets/app.css\\\" />")',
  '"#[wasm_bindgen(start)]"',
  '"rustok-pages-admin"',
  '"rustok-page-builder-admin"',
  '"PagesFlyBuilder"',
  '"PageBuilderAdmin"'
]) {
  need(regression, marker, "anonymous SSR source regression");
}

for (const marker of [
  "--artifact",
  "--output",
  "at least one --artifact is required",
  "createHash(\"sha256\")",
  "verify-pages-anonymous-storefront-graph.mjs",
  "absence_of_a_client_bundle_is_not_reported_as_a_passing_client_bundle",
  "forbidden_markers_found",
  "pages_anonymous_storefront_ssr_artifact_execution_v1"
]) {
  need(inspector, marker, "SSR artifact inspector");
}

for (const marker of [
  "source-ready / execution-pending",
  "SSR-only anonymous Pages delivery",
  "no executable client bootstrap",
  "explicit built artifact",
  "client bundle gate reopens",
  "Execution evidence remains pending"
]) {
  need(packet, marker, "SSR delivery packet");
}
for (const marker of [
  "anonymous-storefront-ssr-delivery-source-ready",
  "Anonymous storefront SSR delivery: source-ready",
  "current public Pages host is SSR-only",
  "client bundle gate is conditional"
]) {
  need(plan, marker, "canonical Pages/Page Builder plan");
}

if (failures.length > 0) {
  console.error("[verify-pages-anonymous-storefront-ssr-delivery] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log(
  "[verify-pages-anonymous-storefront-ssr-delivery] PASS source_ready=true execution=pending host_mode=ssr_only",
);
