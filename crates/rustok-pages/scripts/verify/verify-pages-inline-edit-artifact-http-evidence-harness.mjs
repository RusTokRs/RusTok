#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  contract:
    "crates/rustok-pages/contracts/evidence/pages-inline-edit-artifact-http-execution-contract.json",
  evidence:
    "crates/rustok-pages/contracts/evidence/pages-inline-edit-artifact-http-evidence-harness-source.json",
  buildCapture: "scripts/evidence/capture-pages-inline-edit-build-snapshot.mjs",
  dockerCapture: "scripts/evidence/capture-pages-inline-edit-docker-evidence.mjs",
  httpCapture: "scripts/evidence/capture-pages-inline-edit-http-evidence.mjs",
  assembler: "scripts/evidence/assemble-pages-inline-edit-artifact-http-evidence.mjs",
  anonymousInspector:
    "crates/rustok-pages/scripts/verify/inspect-pages-anonymous-storefront-ssr-artifact.mjs",
  assetRoute: "crates/rustok-pages/src/http/inline_edit_assets.rs",
  auth: "apps/server/src/middleware/auth_context.rs",
  packet:
    "docs/modules/pages-page-builder-inline-edit-artifact-http-evidence-harness-packet-2026-08-06.md",
  executionPlan: "docs/modules/pages-page-builder-inline-edit-execution-plan.md",
  canonicalPlan: "docs/modules/pages-page-builder-parity-continuation-plan.md",
  localPlan: "crates/rustok-pages/docs/implementation-plan.md",
};

const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => fs.readFileSync(absolute(relativePath), "utf8");
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const exact = (actual, expected, label) => {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    failures.push(`${label} drifted`);
  }
};

for (const [label, relativePath] of Object.entries(files)) {
  if (!fs.existsSync(absolute(relativePath))) failures.push(`${label}: missing ${relativePath}`);
}
if (failures.length > 0) {
  console.error("[verify-pages-inline-edit-artifact-http-evidence-harness] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const contract = JSON.parse(read(files.contract));
const evidence = JSON.parse(read(files.evidence));
const sources = Object.fromEntries(
  Object.entries(files)
    .filter(([key]) => !["contract", "evidence"].includes(key))
    .map(([key, relativePath]) => [key, read(relativePath)]),
);

if (
  contract.schema_version !== 1 ||
  contract.module !== "pages" ||
  contract.packet !== "pages-inline-edit-artifact-http-execution" ||
  contract.status !== "source_ready_maintainer_execution_pending"
) failures.push("execution contract identity drifted");
exact(contract.tools, {
  build_snapshot: "scripts/evidence/capture-pages-inline-edit-build-snapshot.mjs",
  docker_capture: "scripts/evidence/capture-pages-inline-edit-docker-evidence.mjs",
  http_capture: "scripts/evidence/capture-pages-inline-edit-http-evidence.mjs",
  assembler: "scripts/evidence/assemble-pages-inline-edit-artifact-http-evidence.mjs",
  source_verifier:
    "crates/rustok-pages/scripts/verify/verify-pages-inline-edit-artifact-http-evidence-harness.mjs",
}, "execution contract tools");
if (
  contract.output?.path !== "target/pages-inline-edit-artifact-http-evidence.json" ||
  contract.output?.format !== "pages_inline_edit_artifact_http_execution_v1" ||
  contract.output?.status !== "artifact_http_execution_passed_browser_rollout_pending" ||
  contract.output?.same_commit_inputs_required !== true ||
  contract.output?.atomic_replace !== true ||
  contract.output?.automatic_canonical_source_mutation !== false
) failures.push("execution contract output boundary drifted");
exact(contract.build_snapshots?.profiles, ["build-a", "build-b"], "build profiles");
exact(contract.build_snapshots?.required_artifacts, [
  "embedded_admin_index",
  "embedded_admin_css",
  "authoring_bootstrap",
  "authoring_module",
  "authoring_wasm",
  "server_binary",
], "critical artifact ids");
if (
  contract.build_snapshots?.format !== "pages_inline_edit_build_snapshot_v1" ||
  contract.build_snapshots?.full_admin_dist_manifest_required !== true ||
  contract.build_snapshots?.source_hashes_required !== true ||
  contract.build_snapshots?.toolchain_versions_required !== true ||
  contract.build_snapshots?.critical_hashes_must_match_between_builds !== true ||
  contract.build_snapshots?.admin_dist_manifest_must_match_between_builds !== true
) failures.push("build snapshot boundary drifted");
if (
  contract.docker_capture?.format !== "pages_inline_edit_docker_execution_v1" ||
  contract.docker_capture?.required_platform !== "linux/amd64" ||
  contract.docker_capture?.required_user !== "10001:10001" ||
  contract.docker_capture?.required_entrypoint !== "/app/rustok-server" ||
  contract.docker_capture?.revision_label_must_match_source_commit !== true ||
  contract.docker_capture?.immutable_repo_digest_required !== true
) failures.push("Docker evidence boundary drifted");
exact(
  contract.http_capture?.asset_paths?.map(({ id, path, content_type }) => [id, path, content_type]),
  [
    ["authoring_bootstrap", "/assets/pages-inline-edit-bootstrap.js", "text/javascript; charset=utf-8"],
    ["authoring_module", "/assets/pages-inline-edit/rustok_storefront.js", "text/javascript; charset=utf-8"],
    ["authoring_wasm", "/assets/pages-inline-edit/rustok_storefront_bg.wasm", "application/wasm"],
  ],
  "HTTP asset set",
);
exact(
  contract.http_capture?.authoring_scenarios?.map(({ id, expected_status }) => [id, expected_status]),
  [
    ["anonymous", 401],
    ["direct_user", 200],
    ["service", 403],
    ["delegated", 403],
    ["missing_session", 401],
    ["permission_denied", 403],
  ],
  "authoring scenarios",
);
for (const [key, expected] of Object.entries({
  asset_cache_control: "public, max-age=0, must-revalidate",
  asset_cross_origin_resource_policy: "same-origin",
  authoring_route_cache_control: "private, no-store",
  authoring_route_robots: "noindex, nofollow, noarchive",
})) {
  if (contract.http_capture?.[key] !== expected) failures.push(`http_capture.${key} drifted`);
}
for (const key of [
  "strong_etag_required",
  "exact_if_none_match_304_required",
  "weak_if_none_match_304_required",
]) {
  if (contract.http_capture?.[key] !== true) failures.push(`http_capture.${key} must be true`);
}
if (
  contract.anonymous_artifact_input?.format !==
    "pages_anonymous_storefront_ssr_artifact_execution_v1" ||
  contract.anonymous_artifact_input?.status !== "passed" ||
  contract.anonymous_artifact_input?.source_commit_required !== true ||
  contract.anonymous_artifact_input?.forbidden_markers_found_must_be_empty !== true
) failures.push("anonymous artifact input boundary drifted");
for (const value of [
  "authorization_header",
  "cookie_header",
  "bearer_token",
  "session_token",
  "session_id",
  "grant",
  "proof",
  "signing_key",
  "raw_html",
  "raw_denial_body",
  "raw_command_log",
  "docker_inspect_document",
]) {
  if (!contract.privacy_boundary?.forbidden_persisted_values?.includes(value)) {
    failures.push(`privacy boundary is missing ${value}`);
  }
}

if (evidence.format !== "pages_inline_edit_artifact_http_evidence_harness_source_v1") {
  failures.push(`source evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_inline_edit_artifact_http_evidence_harness_source_unvalidated") {
  failures.push(`source evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("source evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`source evidence validation.${key} must remain false`);
}
for (const key of [
  "machine_execution_contract_added",
  "two_build_profiles_are_locked",
  "critical_artifact_hash_and_size_capture_added",
  "full_embedded_admin_manifest_capture_added",
  "source_hash_capture_added",
  "toolchain_version_capture_added",
  "raw_build_log_is_not_persisted",
  "production_image_digest_capture_added",
  "production_image_platform_user_entrypoint_checked",
  "oci_revision_is_bound_to_source_commit",
  "raw_docker_inspect_document_is_not_persisted",
  "three_authoring_assets_are_requested",
  "strong_body_bound_etag_is_checked",
  "exact_if_none_match_304_is_checked",
  "weak_if_none_match_304_is_checked",
  "asset_mime_cache_and_corp_are_checked",
  "anonymous_authoring_denial_is_checked",
  "direct_user_authoring_success_is_checked",
  "service_authoring_denial_is_checked",
  "delegated_authoring_denial_is_checked",
  "missing_session_authoring_denial_is_checked",
  "permission_denied_authoring_denial_is_checked",
  "private_no_store_and_robots_headers_are_checked",
  "page_id_and_exact_locale_binding_are_checked",
  "credential_values_are_not_persisted",
  "grants_proofs_and_raw_html_are_not_persisted",
  "http_asset_hashes_are_bound_to_build_artifacts",
  "anonymous_artifact_inspector_output_is_required",
  "all_inputs_must_share_one_source_commit",
  "aggregate_output_is_atomically_replaced",
  "aggregate_does_not_mutate_canonical_source",
  "browser_edit_save_replay_expiry_remains_separate",
  "tenant_rollout_remains_separate",
]) {
  if (evidence.source_contract?.[key] !== true) failures.push(`source_contract.${key} must be true`);
}
for (const key of [
  "tests_run",
  "static_verifiers_run",
  "cargo_run",
  "npm_or_trunk_run",
  "wasm_or_server_build_run",
  "docker_build_or_inspect_run",
  "http_requests_run",
  "browser_run",
  "workflows_or_ci_run",
  "artifact_http_execution_observed",
  "browser_execution_observed",
  "tenant_rollout_observed",
  "ffa_promoted",
  "fba_promoted",
]) {
  if (evidence.source_contract?.[key] !== false) failures.push(`source_contract.${key} must be false`);
}

const requiredMarkers = {
  buildCapture: [
    "--profile build-a|build-b",
    "--command-log FILE",
    'execFileSync("git", ["rev-parse", "HEAD"]',
    "sourceHashes()",
    "directoryManifest(options.adminDist)",
    "raw_command_log_persisted: false",
    "renameSync(temporary, absolute)",
  ],
  dockerCapture: [
    'execFileSync("docker", ["image", "inspect", image]',
    "RepoDigests",
    'labels["org.opencontainers.image.revision"]',
    "docker_inspect_document_persisted: false",
    "renameSync(temporary, absolute)",
  ],
  httpCapture: [
    "AbortSignal.timeout(requestTimeoutMs)",
    '"if-none-match": etag',
    '"if-none-match": `W/${etag}`',
    "RUSTOK_PAGES_INLINE_EDIT_EVIDENCE_COMMON_HEADERS_JSON",
    "credential_environment_names",
    "credential_values_persisted: false",
    "raw_response_bodies_persisted: false",
    "direct_user HTML must bind the requested page id and exact locale",
  ],
  assembler: [
    'validateBuild(inputs.build_a, "build-a", head)',
    'validateBuild(inputs.build_b, "build-b", head)',
    "compareBuilds(buildA, buildB)",
    "HTTP body does not match the built artifact",
    "validateAnonymous(inputs.anonymous, head)",
    "browser_edit_save_replay_expiry_executed: false",
    "canonical_source_mutated: false",
  ],
  anonymousInspector: [
    "pages_anonymous_storefront_ssr_artifact_execution_v1",
    "absence_of_a_client_bundle_is_not_reported_as_a_passing_client_bundle",
    "forbidden_markers_found",
  ],
  assetRoute: [
    "PAGES_INLINE_EDIT_BOOTSTRAP_PATH",
    "PAGES_INLINE_EDIT_MODULE_PATH",
    "PAGES_INLINE_EDIT_WASM_PATH",
    '"public, max-age=0, must-revalidate"',
    '"same-origin"',
    "IF_NONE_MATCH",
  ],
  auth: [
    'PAGES_AUTHORING_CACHE_CONTROL: &str = "private, no-store"',
    'PAGES_AUTHORING_ROBOTS_POLICY: &str = "noindex, nofollow, noarchive"',
    "current_user.principal_kind.is_direct_user()",
    "current_user.session_id.is_nil()",
    "Permission::PAGES_UPDATE",
  ],
  packet: [
    "source-ready / maintainer-execution-pending / browser-rollout-pending",
    "artifact_http_execution_passed_browser_rollout_pending",
    "anonymous          → 401",
    "direct_user        → 200",
    "permission_denied  → 403",
    "Artifact, HTTP, browser and rollout evidence remain pending",
  ],
  executionPlan: [
    "artifact-http-evidence-harness-source-ready",
    "inline-edit-artifact-http-evidence-harness-source-ready",
    "artifact/HTTP evidence harness: source-ready",
    "artifact_http_execution_passed_browser_rollout_pending",
    "Browser edit/save/replay/expiry and tenant rollout remain pending",
  ],
  canonicalPlan: [
    "release-composition-source-ready / execution-browser-rollout-pending",
    "inline-edit-release-composition-source-ready",
    "Execution evidence remains pending",
  ],
  localPlan: [
    "release-composition-source-ready / execution-browser-rollout-pending",
    "Release composition: source-ready",
    "Execution remains pending",
  ],
};
for (const [sourceName, markers] of Object.entries(requiredMarkers)) {
  for (const marker of markers) need(sources[sourceName], marker, sourceName);
}
for (const sourceName of ["buildCapture", "dockerCapture", "httpCapture", "assembler"]) {
  forbid(sources[sourceName], "eval(", sourceName);
  forbid(sources[sourceName], "shell: true", sourceName);
  forbid(sources[sourceName], "|| true", sourceName);
}
forbid(sources.assembler, "docs/modules/pages-page-builder-parity-continuation-plan.md", "assembler canonical mutation");
forbid(sources.assembler, "crates/rustok-pages/docs/implementation-plan.md", "assembler canonical mutation");

if (failures.length > 0) {
  console.error("[verify-pages-inline-edit-artifact-http-evidence-harness] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log(
  "[verify-pages-inline-edit-artifact-http-evidence-harness] PASS source_ready=true execution=pending browser=pending rollout=pending",
);
