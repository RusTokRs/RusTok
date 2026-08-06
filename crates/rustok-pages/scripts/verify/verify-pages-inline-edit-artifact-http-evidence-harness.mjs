#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
  "..",
  "..",
);
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
  releasePacket:
    "docs/modules/pages-page-builder-inline-edit-release-composition-packet-2026-08-06.md",
  canonicalPlan: "docs/modules/pages-page-builder-parity-continuation-plan.md",
  localPlan: "crates/rustok-pages/docs/implementation-plan.md",
};

function absolute(relativePath) {
  return path.join(repoRoot, relativePath);
}

function read(relativePath) {
  return fs.readFileSync(absolute(relativePath), "utf8");
}

function need(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function forbid(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

for (const [label, relativePath] of Object.entries(files)) {
  if (!fs.existsSync(absolute(relativePath))) failures.push(`${label}: missing ${relativePath}`);
}
if (failures.length > 0) {
  console.error("[verify-pages-inline-edit-artifact-http-evidence-harness] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

const contract = JSON.parse(read(files.contract));
const evidence = JSON.parse(read(files.evidence));
const buildCapture = read(files.buildCapture);
const dockerCapture = read(files.dockerCapture);
const httpCapture = read(files.httpCapture);
const assembler = read(files.assembler);
const anonymousInspector = read(files.anonymousInspector);
const assetRoute = read(files.assetRoute);
const auth = read(files.auth);
const packet = read(files.packet);
const releasePacket = read(files.releasePacket);
const canonicalPlan = read(files.canonicalPlan);
const localPlan = read(files.localPlan);

if (
  contract.schema_version !== 1 ||
  contract.module !== "pages" ||
  contract.packet !== "pages-inline-edit-artifact-http-execution" ||
  contract.status !== "source_ready_maintainer_execution_pending"
) {
  failures.push("execution contract identity drifted");
}
const expectedTools = {
  build_snapshot: "scripts/evidence/capture-pages-inline-edit-build-snapshot.mjs",
  docker_capture: "scripts/evidence/capture-pages-inline-edit-docker-evidence.mjs",
  http_capture: "scripts/evidence/capture-pages-inline-edit-http-evidence.mjs",
  assembler: "scripts/evidence/assemble-pages-inline-edit-artifact-http-evidence.mjs",
  source_verifier:
    "crates/rustok-pages/scripts/verify/verify-pages-inline-edit-artifact-http-evidence-harness.mjs",
};
if (JSON.stringify(contract.tools) !== JSON.stringify(expectedTools)) {
  failures.push("execution contract tool ownership drifted");
}
if (
  contract.output?.path !== "target/pages-inline-edit-artifact-http-evidence.json" ||
  contract.output?.format !== "pages_inline_edit_artifact_http_execution_v1" ||
  contract.output?.status !== "artifact_http_execution_passed_browser_rollout_pending" ||
  contract.output?.same_commit_inputs_required !== true ||
  contract.output?.atomic_replace !== true ||
  contract.output?.automatic_canonical_source_mutation !== false
) {
  failures.push("execution contract output boundary drifted");
}
if (
  JSON.stringify(contract.build_snapshots?.profiles) !== JSON.stringify(["build-a", "build-b"]) ||
  contract.build_snapshots?.format !== "pages_inline_edit_build_snapshot_v1" ||
  contract.build_snapshots?.full_admin_dist_manifest_required !== true ||
  contract.build_snapshots?.critical_hashes_must_match_between_builds !== true ||
  contract.build_snapshots?.admin_dist_manifest_must_match_between_builds !== true
) {
  failures.push("execution contract build snapshot boundary drifted");
}
const requiredArtifacts = [
  "embedded_admin_index",
  "embedded_admin_css",
  "authoring_bootstrap",
  "authoring_module",
  "authoring_wasm",
  "server_binary",
];
if (JSON.stringify(contract.build_snapshots?.required_artifacts) !== JSON.stringify(requiredArtifacts)) {
  failures.push("execution contract critical artifacts drifted");
}
if (
  contract.docker_capture?.format !== "pages_inline_edit_docker_execution_v1" ||
  contract.docker_capture?.required_platform !== "linux/amd64" ||
  contract.docker_capture?.required_user !== "10001:10001" ||
  contract.docker_capture?.required_entrypoint !== "/app/rustok-server" ||
  contract.docker_capture?.revision_label_must_match_source_commit !== true ||
  contract.docker_capture?.immutable_repo_digest_required !== true
) {
  failures.push("execution contract Docker boundary drifted");
}
const expectedAssets = [
  [
    "authoring_bootstrap",
    "/assets/pages-inline-edit-bootstrap.js",
    "text/javascript; charset=utf-8",
  ],
  [
    "authoring_module",
    "/assets/pages-inline-edit/rustok_storefront.js",
    "text/javascript; charset=utf-8",
  ],
  [
    "authoring_wasm",
    "/assets/pages-inline-edit/rustok_storefront_bg.wasm",
    "application/wasm",
  ],
];
if (
  JSON.stringify(
    contract.http_capture?.asset_paths?.map(({ id, path, content_type }) => [
      id,
      path,
      content_type,
    ]),
  ) !== JSON.stringify(expectedAssets)
) {
  failures.push("execution contract HTTP asset set drifted");
}
if (
  contract.http_capture?.asset_cache_control !== "public, max-age=0, must-revalidate" ||
  contract.http_capture?.asset_cross_origin_resource_policy !== "same-origin" ||
  contract.http_capture?.strong_etag_required !== true ||
  contract.http_capture?.exact_if_none_match_304_required !== true ||
  contract.http_capture?.weak_if_none_match_304_required !== true ||
  contract.http_capture?.authoring_route_cache_control !== "private, no-store" ||
  contract.http_capture?.authoring_route_robots !== "noindex, nofollow, noarchive"
) {
  failures.push("execution contract HTTP policy drifted");
}
const expectedScenarios = [
  ["anonymous", 401],
  ["direct_user", 200],
  ["service", 403],
  ["delegated", 403],
  ["missing_session", 401],
  ["permission_denied", 403],
];
if (
  JSON.stringify(
    contract.http_capture?.authoring_scenarios?.map(({ id, expected_status }) => [
      id,
      expected_status,
    ]),
  ) !== JSON.stringify(expectedScenarios)
) {
  failures.push("execution contract authoring scenarios drifted");
}
if (
  contract.anonymous_artifact_input?.format !==
    "pages_anonymous_storefront_ssr_artifact_execution_v1" ||
  contract.anonymous_artifact_input?.status !== "passed" ||
  contract.anonymous_artifact_input?.source_commit_required !== true ||
  contract.anonymous_artifact_input?.forbidden_markers_found_must_be_empty !== true
) {
  failures.push("execution contract anonymous artifact boundary drifted");
}
for (const key of [
  "persist_environment_names_only",
  "persist_response_and_command_hashes_only",
]) {
  if (contract.privacy_boundary?.[key] !== true) {
    failures.push(`execution contract privacy_boundary.${key} must be true`);
  }
}
for (const forbiddenValue of [
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
  if (!contract.privacy_boundary?.forbidden_persisted_values?.includes(forbiddenValue)) {
    failures.push(`execution contract privacy list is missing ${forbiddenValue}`);
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
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`source evidence source_contract.${key} must be true`);
  }
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
  if (evidence.source_contract?.[key] !== false) {
    failures.push(`source evidence source_contract.${key} must be false`);
  }
}

for (const marker of [
  "--profile build-a|build-b",
  "--server-binary FILE",
  "--trunk FILE",
  "--wasm-bindgen FILE",
  "--command-log FILE",
  'execFileSync("git", ["rev-parse", "HEAD"]',
  "sourceHashes()",
  "directoryManifest(options.adminDist)",
  "raw_command_log_persisted: false",
  "credentials_persisted: false",
  "grants_or_proofs_persisted: false",
  "renameSync(temporary, absolute)",
]) {
  need(buildCapture, marker, "build snapshot capture");
}
for (const marker of [
  'execFileSync("docker", ["image", "inspect", image]',
  "RepoDigests",
  "required_platform",
  "required_user",
  "required_entrypoint",
  'labels["org.opencontainers.image.revision"]',
  "raw_document_persisted: false",
  "docker_inspect_document_persisted: false",
  "renameSync(temporary, absolute)",
]) {
  need(dockerCapture, marker, "Docker evidence capture");
}
for (const marker of [
  "AbortSignal.timeout(requestTimeoutMs)",
  '"if-none-match": etag',
  '"if-none-match": `W/${etag}`',
  "RUSTOK_PAGES_INLINE_EDIT_EVIDENCE_COMMON_HEADERS_JSON",
  'headers.authorization = authorization',
  'headers.cookie = cookie',
  "credential_environment_names",
  "credential_values_persisted: false",
  "raw_response_bodies_persisted: false",
  "grants_or_proofs_persisted: false",
  "direct_user HTML must bind the requested page id and exact locale",
  "renameSync(temporary, absolute)",
]) {
  need(httpCapture, marker, "HTTP evidence capture");
}
for (const marker of [
  'validateBuild(inputs.build_a, "build-a", head)',
  'validateBuild(inputs.build_b, "build-b", head)',
  "compareBuilds(buildA, buildB)",
  "build toolchain versions do not match",
  "build source hashes do not match",
  "embedded admin dist manifest is not reproducible",
  "HTTP body does not match the built artifact",
  "validateAnonymous(inputs.anonymous, head)",
  "browser_edit_save_replay_expiry_executed: false",
  "tenant_rollout_executed: false",
  "canonical_source_mutated: false",
  "renameSync(temporary, location)",
]) {
  need(assembler, marker, "artifact/HTTP evidence assembler");
}
for (const marker of [
  "pages_anonymous_storefront_ssr_artifact_execution_v1",
  "absence_of_a_client_bundle_is_not_reported_as_a_passing_client_bundle",
  "forbidden_markers_found",
  "source_commit",
]) {
  need(anonymousInspector, marker, "anonymous artifact inspector");
}
for (const marker of [
  'PAGES_INLINE_EDIT_BOOTSTRAP_PATH: &str =',
  'PAGES_INLINE_EDIT_MODULE_PATH: &str =',
  'PAGES_INLINE_EDIT_WASM_PATH: &str =',
  '"public, max-age=0, must-revalidate"',
  '"same-origin"',
  "If-None-Match",
]) {
  const normalizedMarker = marker === "If-None-Match" ? "IF_NONE_MATCH" : marker;
  need(assetRoute, normalizedMarker, "asset route source");
}
for (const marker of [
  'PAGES_AUTHORING_CACHE_CONTROL: &str = "private, no-store"',
  'PAGES_AUTHORING_ROBOTS_POLICY: &str = "noindex, nofollow, noarchive"',
  "current_user.principal_kind.is_direct_user()",
  "current_user.session_id.is_nil()",
  "Permission::PAGES_UPDATE",
  "SecurityActorKind::User",
]) {
  need(auth, marker, "authoring admission source");
}

for (const marker of [
  "source-ready / maintainer-execution-pending / browser-rollout-pending",
  "two independent deterministic build snapshots",
  "artifact_http_execution_passed_browser_rollout_pending",
  "anonymous          → 401",
  "direct_user        → 200",
  "service            → 403",
  "delegated          → 403",
  "missing_session    → 401",
  "permission_denied  → 403",
  "Credential values are read only from named environment variables",
  "Artifact, HTTP, browser and rollout evidence remain pending",
]) {
  need(packet, marker, "artifact/HTTP evidence packet");
}
for (const marker of [
  "Run the Pages route, asset, launch and release-composition static guards",
  "Build the composition twice",
  "Prove asset `200`/`304`",
  "Observe edit, save, replacement grant, stale revision, replay and expiry browser behavior",
]) {
  need(releasePacket, marker, "release composition packet");
}
for (const [document, label] of [
  [canonicalPlan, "canonical plan"],
  [localPlan, "local plan"],
]) {
  for (const marker of [
    "inline-edit-artifact-http-evidence-harness-source-ready",
    "artifact/HTTP evidence harness: source-ready",
    "artifact_http_execution_passed_browser_rollout_pending",
    "Browser edit/save/replay/expiry and tenant rollout remain pending",
  ]) {
    need(document, marker, label);
  }
}

for (const [source, label] of [
  [buildCapture, "build snapshot capture"],
  [dockerCapture, "Docker evidence capture"],
  [httpCapture, "HTTP evidence capture"],
  [assembler, "artifact/HTTP evidence assembler"],
]) {
  forbid(source, "eval(", label);
  forbid(source, "shell: true", label);
  forbid(source, "|| true", label);
}
forbid(httpCapture, "set-cookie", "HTTP retained response headers");
forbid(assembler, "writeFileSync(absolute(files.canonicalPlan)", "assembler canonical mutation");
forbid(assembler, "writeFileSync(absolute(files.localPlan)", "assembler canonical mutation");

if (failures.length > 0) {
  console.error("[verify-pages-inline-edit-artifact-http-evidence-harness] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "[verify-pages-inline-edit-artifact-http-evidence-harness] PASS source_ready=true execution=pending browser=pending rollout=pending",
);
