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
const count = (text, marker) => text.split(marker).length - 1;
const needCount = (text, marker, expected, label) => {
  const actual = count(text, marker);
  if (actual !== expected) failures.push(`${label}: expected ${expected} occurrence(s) of ${marker}, found ${actual}`);
};

const files = {
  evidence: "crates/rustok-pages/contracts/evidence/pages-inline-edit-release-composition-source.json",
  adminBuilder: "scripts/build/build-embedded-admin.sh",
  deploymentBuilder: "scripts/build/build-pages-inline-edit-deployment.sh",
  serverBuilder: "scripts/build/build-pages-inline-edit-server.sh",
  clientBuilder: "apps/storefront/scripts/build-pages-inline-edit-client.mjs",
  release: ".github/workflows/release.yml",
  infrastructure: ".github/workflows/release-infrastructure.yml",
  hardening: ".github/workflows/hardening-gates.yml",
  serverDocker: "apps/server/Dockerfile",
  runtimeDocker: "apps/server/Dockerfile.release",
  standaloneAdminDocker: "apps/admin/Dockerfile",
  approval: "scripts/verify/verify-release-infrastructure-approval.mjs",
  supplyChain: "scripts/verify/verify-release-supply-chain-contract.mjs",
  readinessGuard: "scripts/verify/verify-release-readiness-contract.mjs",
  readinessChecklist: "docs/release/RELEASE_READINESS_CHECKLIST.md",
  localPlan: "crates/rustok-pages/docs/implementation-plan.md",
  plan: "docs/modules/pages-page-builder-parity-continuation-plan.md",
  packet: "docs/modules/pages-page-builder-inline-edit-release-composition-packet-2026-08-06.md",
};

for (const [label, relativePath] of Object.entries(files)) {
  if (!fs.existsSync(path.join(root, relativePath))) failures.push(`${label}: missing ${relativePath}`);
}
if (failures.length > 0) {
  console.error("[verify-pages-inline-edit-release-composition] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

const evidence = JSON.parse(read(files.evidence));
const adminBuilder = read(files.adminBuilder);
const deploymentBuilder = read(files.deploymentBuilder);
const serverBuilder = read(files.serverBuilder);
const clientBuilder = read(files.clientBuilder);
const release = read(files.release);
const infrastructure = read(files.infrastructure);
const hardening = read(files.hardening);
const serverDocker = read(files.serverDocker);
const runtimeDocker = read(files.runtimeDocker);
const standaloneAdminDocker = read(files.standaloneAdminDocker);
const approval = read(files.approval);
const supplyChain = read(files.supplyChain);
const readinessGuard = read(files.readinessGuard);
const readinessChecklist = read(files.readinessChecklist);
const localPlan = read(files.localPlan);
const plan = read(files.plan);
const packet = read(files.packet);

if (evidence.format !== "pages_inline_edit_release_composition_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_inline_edit_release_composition_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("source evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
for (const key of [
  "single_deployment_orchestrator_added",
  "embedded_admin_builder_reused",
  "embedded_admin_launch_feature_enabled_explicitly",
  "same_origin_compile_time_acknowledgement_set_only_in_launch_mode",
  "standard_embedded_admin_build_clears_same_origin_acknowledgement",
  "trunk_version_is_exactly_pinned",
  "dedicated_client_builder_reused",
  "wasm_bindgen_version_remains_derived_from_cargo_lock",
  "wasm_bindgen_cli_version_is_rejected_on_mismatch",
  "server_asset_profile_remains_pages_inline_edit_assets",
  "embedded_admin_wasm_rustflags_are_separate",
  "dedicated_client_wasm_rustflags_are_separate",
  "native_server_rustflags_are_preserved",
  "release_build_uses_deployment_orchestrator",
  "release_reproducibility_build_uses_same_orchestrator",
  "production_server_docker_builder_uses_same_orchestrator",
  "development_server_docker_profile_remains_without_launch_feature",
  "standalone_admin_docker_profile_is_unchanged",
  "runtime_only_release_image_is_unchanged",
  "release_action_pins_match_allow_list",
  "infrastructure_action_pins_match_allow_list",
  "hardening_action_pins_match_allow_list",
  "release_infrastructure_approval_protects_all_inline_edit_build_owners",
  "release_supply_chain_guard_requires_composition",
  "release_readiness_checklist_requires_artifact_and_browser_evidence",
]) {
  if (evidence.source_contract?.[key] !== true) failures.push(`source_contract.${key} must be true`);
}
for (const key of [
  "anonymous_public_pages_route_changed",
  "database_schema_changed",
  "graphql_schema_changed",
  "rest_mutation_changed",
  "page_document_persistence_owner_changed",
  "publish_or_rollback_behavior_changed",
  "release_workflow_executed",
  "reproducibility_workflow_executed",
  "production_docker_build_executed",
  "embedded_admin_artifact_built",
  "authoring_client_artifact_built",
  "server_binary_built",
  "asset_http_delivery_observed",
  "admin_launch_navigation_observed",
  "browser_inline_edit_observed",
  "ffa_promoted",
  "fba_promoted",
]) {
  if (evidence.source_contract?.[key] !== false) failures.push(`source_contract.${key} must be false`);
}

for (const marker of [
  "--pages-inline-edit-launch",
  'cargo install trunk --version "=0.21.14" --locked',
  'env -u RUSTOK_PAGES_INLINE_EDIT_ADMIN_SAME_ORIGIN',
  'env RUSTOK_PAGES_INLINE_EDIT_ADMIN_SAME_ORIGIN=true',
  "--no-default-features",
  "--features hydrate,pages-inline-edit-launch",
  'TRUNK_BUILD_PUBLIC_URL="$public_url"',
  'TRUNK_BUILD_LOCKED="true"',
]) need(adminBuilder, marker, "embedded admin builder");
for (const marker of ["|| true", "eval "]) forbid(adminBuilder, marker, "embedded admin builder");

for (const marker of [
  "build-embedded-admin.sh",
  "build-pages-inline-edit-server.sh",
  "--pages-inline-edit-launch",
  "--profile release",
  "RUSTOK_EMBEDDED_ADMIN_RUSTFLAGS",
  "RUSTOK_PAGES_INLINE_EDIT_CLIENT_RUSTFLAGS",
  'RUSTFLAGS="$admin_rustflags"',
  'RUSTFLAGS="$server_rustflags"',
  "pages-inline-edit-bootstrap.js",
  "rustok_storefront.js",
  "rustok_storefront_bg.wasm",
  'test -x "$server_target_dir/release/rustok-server"',
]) need(deploymentBuilder, marker, "deployment orchestrator");
for (const marker of ["|| true", "eval "]) forbid(deploymentBuilder, marker, "deployment orchestrator");

for (const marker of [
  '"--print-wasm-bindgen-version"',
  'cargo install wasm-bindgen-cli',
  '--version "=$wasm_bindgen_version"',
  "--locked",
  "RUSTOK_PAGES_INLINE_EDIT_CLIENT_RUSTFLAGS",
  'RUSTFLAGS="$client_rustflags"',
  'RUSTFLAGS="$server_rustflags"',
  "RUSTOK_WASM_BINDGEN_BIN",
  "--features pages-inline-edit-assets",
]) need(serverBuilder, marker, "authoring client/server builder");
for (const marker of [
  'readFileSync(path.join(repoRoot, "Cargo.lock"), "utf8")',
  '"--print-wasm-bindgen-version"',
  '"--locked"',
  "RUSTOK_WASM_BINDGEN_BIN",
  'run(wasmBindgen, ["--version"], true)',
  "renameSync(stagingRoot, targetRoot)",
]) need(clientBuilder, marker, "dedicated client builder");

for (const marker of [
  "Build same-origin Pages inline edit release composition",
  "Rebuild same-origin Pages inline edit release composition",
  "scripts/build/build-pages-inline-edit-deployment.sh",
  "--trunk-tool-root \"$GITHUB_WORKSPACE/.tools/trunk-0.21.14\"",
  "--wasm-bindgen-tool-root \"$GITHUB_WORKSPACE/.tools/wasm-bindgen-cli\"",
  "--admin-target-dir \"$GITHUB_WORKSPACE/target/admin-assets\"",
  "--server-target-dir \"$CARGO_TARGET_DIR\"",
  "RUSTOK_EMBEDDED_ADMIN_RUSTFLAGS",
  "Release archive is not reproducible",
  "--provenance=mode=max",
  "--sbom=true",
]) need(release, marker, "release workflow");
needCount(release, "scripts/build/build-pages-inline-edit-deployment.sh", 2, "release workflow");
needCount(release, "actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0", 6, "release workflow");
needCount(release, "actions/setup-node@249970729cb0ef3589644e2896645e5dc5ba9c38", 6, "release workflow");
needCount(release, "actions/download-artifact@634f93cb2916e3fdff6788551b99b062d0335ce0", 4, "release workflow");
for (const marker of [
  "Build embedded admin assets",
  "Rebuild embedded admin assets",
  "cargo build --locked --release -p rustok-server --bin rustok-server",
  "actions/checkout@v",
  "actions/setup-node@v",
]) forbid(release, marker, "release workflow");

needCount(infrastructure, "actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0", 2, "release infrastructure workflow");
needCount(infrastructure, "actions/setup-node@249970729cb0ef3589644e2896645e5dc5ba9c38", 1, "release infrastructure workflow");
needCount(hardening, "actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0", 2, "hardening workflow");
needCount(hardening, "actions/setup-node@249970729cb0ef3589644e2896645e5dc5ba9c38", 2, "hardening workflow");

for (const marker of [
  "bash scripts/build/build-embedded-admin.sh",
  "bash scripts/build/build-pages-inline-edit-deployment.sh",
  "--skip-trunk-tool-install",
  "--wasm-bindgen-tool-root /opt/wasm-bindgen-cli",
  "pages-inline-edit-bootstrap.js",
  "rustok_storefront.js",
  "rustok_storefront_bg.wasm",
  "test -x /workspace/target/release/rustok-server",
]) need(serverDocker, marker, "production server Docker source");
needCount(serverDocker, "bash scripts/build/build-embedded-admin.sh", 1, "production server Docker source");
needCount(serverDocker, "bash scripts/build/build-pages-inline-edit-deployment.sh", 1, "production server Docker source");
forbid(serverDocker, "cargo build --locked --release -p rustok-server --bin rustok-server", "production server Docker source");

for (const marker of [
  "FROM debian:bookworm-20260713-slim@sha256:",
  "COPY --chown=10001:10001 rustok-server",
  'ENTRYPOINT ["/app/rustok-server"]',
]) need(runtimeDocker, marker, "runtime-only release image");
for (const marker of ["cargo build", "npm ci", "trunk", "wasm-bindgen"]) forbid(runtimeDocker, marker, "runtime-only release image");
forbid(standaloneAdminDocker, "RUSTOK_PAGES_INLINE_EDIT_ADMIN_SAME_ORIGIN", "standalone admin Docker source");
forbid(standaloneAdminDocker, "pages-inline-edit-launch", "standalone admin Docker source");

for (const marker of [
  'const APPROVAL_LABEL = "release-infra-approved"',
  '"scripts/build/build-pages-inline-edit-deployment.sh"',
  '"scripts/build/build-pages-inline-edit-server.sh"',
  '"apps/storefront/scripts/build-pages-inline-edit-client.mjs"',
]) need(approval, marker, "release infrastructure approval policy");
for (const marker of [
  "Build same-origin Pages inline edit release composition",
  "build-pages-inline-edit-deployment.sh",
  "RUSTOK_PAGES_INLINE_EDIT_CLIENT_RUSTFLAGS",
  "pages-inline-edit-assets",
  "release-infra-approved",
]) need(supplyChain, marker, "release supply-chain policy");
for (const marker of [
  "scripts/build/build-pages-inline-edit-deployment.sh",
  "RUSTOK_EMBEDDED_ADMIN_RUSTFLAGS",
  "locked same-origin Pages inline-edit composition",
]) need(readinessGuard, marker, "release readiness guard");
for (const marker of [
  "same-origin Pages inline-edit deployment composition was built from locked npm and Cargo inputs",
  "Both isolated build jobs run the same locked Pages inline-edit deployment composition",
  "verify-pages-inline-edit-release-composition.mjs",
  "Embedded admin JS/WASM SHA-256 and sizes",
  "Authoring client JS/WASM SHA-256 and sizes",
  "Inline-edit browser evidence",
  "A checkbox without a durable run, artifact or operator record is not release evidence.",
]) need(readinessChecklist, marker, "release readiness checklist");

for (const marker of [
  "release-composition-source-ready",
  "Release composition: source-ready",
  "admin asset build integration remains pending",
  "Execution evidence remains pending",
]) need(localPlan, marker, "Pages implementation plan");
for (const marker of [
  "release-composition-source-ready",
  "Deterministic release composition: source-ready",
  "release workflow and admin launch integration remain pending",
  "execution-browser-rollout-pending",
]) need(plan, marker, "canonical Pages/Page Builder plan");
for (const marker of [
  "source-ready / execution-browser-rollout-pending",
  "scripts/build/build-pages-inline-edit-deployment.sh",
  "RUSTOK_PAGES_INLINE_EDIT_CLIENT_RUSTFLAGS",
  "release-infra-approved",
  "Execution evidence remains pending",
]) need(packet, marker, "release composition packet");

if (failures.length > 0) {
  console.error("[verify-pages-inline-edit-release-composition] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log(
  "[verify-pages-inline-edit-release-composition] PASS source_ready=true release_execution=pending docker_execution=pending browser=pending rollout=pending",
);
