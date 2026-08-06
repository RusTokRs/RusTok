#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
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
  evidence: "crates/rustok-pages/contracts/evidence/pages-inline-edit-asset-delivery-source.json",
  pagesCargo: "crates/rustok-pages/Cargo.toml",
  pagesHttp: "crates/rustok-pages/src/http.rs",
  assets: "crates/rustok-pages/src/http/inline_edit_assets.rs",
  serverCargo: "apps/server/Cargo.toml",
  clientBuilder: "apps/storefront/scripts/build-pages-inline-edit-client.mjs",
  serverBuilder: "scripts/build/build-pages-inline-edit-server.sh",
  localPlan: "crates/rustok-pages/docs/implementation-plan.md",
  plan: "docs/modules/pages-page-builder-parity-continuation-plan.md",
  packet: "docs/modules/pages-page-builder-inline-edit-asset-delivery-packet-2026-08-06.md",
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
const packet = read(files.packet);

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
for (const key of [
  "pages_module_http_owner_reused",
  "asset_router_is_feature_gated",
  "server_asset_profile_is_opt_in",
  "server_default_profile_does_not_enable_assets",
  "fixed_bootstrap_path_preserved",
  "fixed_javascript_module_path_preserved",
  "fixed_wasm_module_path_preserved",
  "generated_assets_are_embedded_in_server_binary",
  "missing_generated_assets_fail_asset_profile_compilation",
  "runtime_filesystem_asset_dependency_removed",
  "javascript_content_type_is_explicit",
  "wasm_content_type_is_explicit",
  "stable_paths_require_revalidation",
  "content_derived_sha256_etag_added",
  "if_none_match_returns_not_modified",
  "weak_if_none_match_is_supported",
  "cross_origin_resource_policy_is_same_origin",
  "asset_payloads_contain_no_grant_or_authorization_proof",
  "client_builder_uses_locked_cargo_graph",
  "client_builder_resolves_exact_wasm_bindgen_version_from_cargo_lock",
  "client_builder_rejects_mismatched_wasm_bindgen_cli",
  "client_builder_respects_cargo_target_dir",
  "client_builder_publishes_generated_pair_atomically",
  "server_orchestrator_installs_exact_wasm_bindgen_cli",
  "server_orchestrator_builds_assets_before_embedding_binary",
  "server_orchestrator_uses_locked_cargo_graph",
  "binary_only_release_packaging_is_compatible",
]) {
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`source_contract.${key} must be true`);
  }
}
for (const key of [
  "anonymous_public_pages_route_changed",
  "database_schema_changed",
  "graphql_schema_changed",
  "rest_mutation_changed",
  "page_document_persistence_owner_changed",
  "publish_or_rollback_behavior_changed",
  "release_workflow_integrated",
  "production_docker_builder_integrated",
  "admin_launch_link_added",
  "client_artifact_built",
  "server_binary_built_with_assets",
  "asset_http_delivery_observed",
  "authenticated_browser_edit_observed",
  "ffa_promoted",
  "fba_promoted",
]) {
  if (evidence.source_contract?.[key] !== false) {
    failures.push(`source_contract.${key} must be false`);
  }
}

for (const marker of [
  "[features]",
  "default = []",
  "inline-edit-assets = []",
]) need(pagesCargo, marker, "Pages asset feature");
for (const marker of [
  '#[cfg(feature = "inline-edit-assets")]\nmod inline_edit_assets;',
  '#[cfg(feature = "inline-edit-assets")]\n    let router = router.merge(inline_edit_assets::router());',
]) need(pagesHttp, marker, "Pages HTTP owner");

for (const marker of [
  '"/assets/pages-inline-edit-bootstrap.js"',
  '"/assets/pages-inline-edit/rustok_storefront.js"',
  '"/assets/pages-inline-edit/rustok_storefront_bg.wasm"',
  'include_bytes!(concat!(',
  '"/../../target/site/assets/pages-inline-edit-bootstrap.js"',
  '"/../../target/site/assets/pages-inline-edit/rustok_storefront.js"',
  '"/../../target/site/assets/pages-inline-edit/rustok_storefront_bg.wasm"',
  '"text/javascript; charset=utf-8"',
  '"application/wasm"',
  '"public, max-age=0, must-revalidate"',
  '"cross-origin-resource-policy"',
  '"same-origin"',
  "Sha256::digest(bytes)",
  "IF_NONE_MATCH",
  "StatusCode::NOT_MODIFIED",
  'candidate.strip_prefix("W/")',
]) need(assets, marker, "embedded Pages asset router");
for (const marker of ["authorization_proof", "PageInlineEditGrant", "PAGES_INLINE_EDIT_HMAC_KEY"]) {
  forbid(assets, marker, "embedded Pages asset router");
}

need(
  serverCargo,
  'pages-inline-edit-assets = ["pages-inline-edit", "rustok-pages/inline-edit-assets"]',
  "server asset feature",
);
forbid(
  featureBody(serverCargo, "default", "server manifest"),
  "pages-inline-edit-assets",
  "server default feature",
);

for (const marker of [
  'readFileSync(path.join(repoRoot, "Cargo.lock"), "utf8")',
  '"--print-wasm-bindgen-version"',
  '"--locked"',
  'process.env.CARGO_TARGET_DIR?.trim()',
  'RUSTOK_WASM_BINDGEN_BIN',
  'wasm-bindgen ${lockedWasmBindgenVersion}',
  '`${targetRoot}.tmp-${process.pid}`',
  "renameSync(stagingRoot, targetRoot)",
  'requireNonEmptyFile(path.join(stagingRoot, file)',
]) need(clientBuilder, marker, "client artifact builder");
forbid(clientBuilder, 'path.join(repoRoot, "target", "wasm32-unknown-unknown"', "client artifact builder");

for (const marker of [
  "set -euo pipefail",
  "wasm-bindgen-cli",
  '--version "$wasm_bindgen_version"',
  "--locked",
  "rustup target add wasm32-unknown-unknown",
  'RUSTOK_WASM_BINDGEN_BIN="$wasm_bindgen"',
  'node "$client_builder"',
  "--features pages-inline-edit-assets",
  'test -s "$repo_root/target/site/assets/pages-inline-edit-bootstrap.js"',
  'test -s "$repo_root/target/site/assets/pages-inline-edit/rustok_storefront.js"',
  'test -s "$repo_root/target/site/assets/pages-inline-edit/rustok_storefront_bg.wasm"',
  'test -x "$target_dir/$profile/rustok-server"',
]) need(serverBuilder, marker, "embedded server build orchestrator");
for (const marker of ["eval ", "|| true", "cargo build --offline"]) {
  forbid(serverBuilder, marker, "embedded server build orchestrator");
}

for (const marker of [
  "inline edit asset delivery: source-ready",
  "release workflow integration remains pending",
  "admin-owned launch link remains pending",
]) need(localPlan, marker, "Pages implementation plan");
for (const marker of [
  "inline-edit-asset-delivery-source-ready",
  "Dedicated authoring asset delivery: source-ready",
  "release workflow and admin launch integration remain pending",
]) need(plan, marker, "canonical Pages/Page Builder plan");
for (const marker of [
  "source-ready / execution-pending",
  "binary-embedded",
  "public, max-age=0, must-revalidate",
  "Cross-Origin-Resource-Policy: same-origin",
  "release workflow integration",
  "Execution evidence remains pending",
]) need(packet, marker, "asset delivery packet");

if (failures.length > 0) {
  console.error("[verify-pages-inline-edit-asset-delivery] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log(
  "[verify-pages-inline-edit-asset-delivery] PASS source_ready=true execution=pending release_integration=open admin_launch=open",
);
