#!/usr/bin/env node

import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

function requireContains(text, needle, message) {
  if (!text.includes(needle)) throw new Error(message);
}

function requireAbsent(text, needle, message) {
  if (text.includes(needle)) throw new Error(message);
}

const contractPath =
  "crates/rustok-forum/contracts/evidence/forum-page-builder-serverfn-deployment-attestation-contract.json";
const runnerPath = "scripts/evidence/capture-forum-page-builder-serverfn-attestation.mjs";
const verifierPath = "scripts/verify/verify-forum-page-builder-serverfn-deployment-attestation.mjs";
const previewTransportPath = "crates/rustok-forum/admin/src/widget_preview_transport.rs";
const propertyTransportPath = "crates/rustok-forum/admin/src/widget_property_transport.rs";
const adminLibPath = "crates/rustok-forum/admin/src/lib.rs";
const appRouterPath = "apps/server/src/services/app_router.rs";
const dockerfilePath = "apps/server/Dockerfile";
const releaseDockerfilePath = "apps/server/Dockerfile.release";
const packetPath =
  "docs/modules/forum-page-builder-serverfn-deployment-attestation-actualization-2026-08-08.md";

const contract = JSON.parse(read(contractPath));
const runner = read(runnerPath);
const previewTransport = read(previewTransportPath);
const propertyTransport = read(propertyTransportPath);
const adminLib = read(adminLibPath);
const appRouter = read(appRouterPath);
const dockerfile = read(dockerfilePath);
const releaseDockerfile = read(releaseDockerfilePath);
const packet = read(packetPath);

if (contract.status !== "source_ready_maintainer_execution_pending") {
  throw new Error("deployment attestation contract must not claim execution");
}
if (contract.runner !== runnerPath) {
  throw new Error("deployment attestation contract must point to the retained runner");
}
if (contract.endpoint !== "/api/fn/forum/page-builder-transport-attestation") {
  throw new Error("Forum deployment attestation endpoint drifted");
}
if (contract.output?.format !== "forum_page_builder_server_fn_deployment_attestation_v1") {
  throw new Error("Forum deployment attestation output format drifted");
}
if (contract.output?.status !== "server_fn_deployment_attestation_passed_wave_pending") {
  throw new Error("Forum deployment attestation output status must keep Wave pending");
}
if (contract.deployment_identity?.cryptographic_origin_to_repo_digest_binding_claimed !== false) {
  throw new Error("source must not claim cryptographic origin-to-RepoDigest binding");
}
if (contract.deployment_identity?.origin_to_repo_digest_binding_is_maintainer_reviewed_external_fact !== true) {
  throw new Error("origin-to-RepoDigest binding must stay an external reviewed fact");
}

const scenarioIds = contract.scenarios?.map((scenario) => scenario.id);
if (JSON.stringify(scenarioIds) !== JSON.stringify(["anonymous", "authorized", "no_read"])) {
  throw new Error("deployment attestation scenario matrix drifted");
}
const authorized = contract.scenarios.find((scenario) => scenario.id === "authorized");
if (authorized?.expected_status !== 200) {
  throw new Error("authorized deployment attestation must require HTTP 200");
}
for (const id of ["anonymous", "no_read"]) {
  if (!contract.scenarios.find((scenario) => scenario.id === id)?.success_forbidden) {
    throw new Error(`${id} must be forbidden from a valid success attestation`);
  }
}
for (const pending of [
  "server-function deployment attestation execution",
  "cryptographic origin-to-RepoDigest binding",
  "browser execution",
  "runtime authorization execution",
  "observed Page Builder Wave",
  "provider SLO health",
]) {
  if (!contract.not_claimed?.includes(pending)) {
    throw new Error(`deployment contract must keep ${pending} unclaimed`);
  }
}
for (const source of [runnerPath, verifierPath, previewTransportPath, propertyTransportPath, adminLibPath, appRouterPath, dockerfilePath, releaseDockerfilePath, packetPath]) {
  if (!contract.required_source_files?.includes(source)) {
    throw new Error(`deployment contract must hash ${source}`);
  }
}

for (const marker of [
  'const FORUM_PAGE_BUILDER_ATTESTATION_CONTRACT: &str =',
  '"forum_page_builder_server_fn_attestation_v1"',
  'pub struct ForumPageBuilderTransportAttestationResponse',
  'pub source_commit: Option<String>',
  'std::env::var("RUSTOK_SOURCE_COMMIT")',
  'value.len() == 40',
  'value.bytes().all(|byte| byte.is_ascii_hexdigit())',
  '#[server(prefix = "/api/fn", endpoint = "forum/page-builder-transport-attestation")]',
  'pub async fn attest_forum_page_builder_transport(',
  'challenge: String',
  'validate_attestation_challenge(&challenge)?',
  'leptos_axum::extract::<rustok_api::AuthContext>()',
  'leptos_axum::extract::<rustok_api::TenantContext>()',
  'require_forum_transport_authorization(&auth, &tenant)?',
  'let (host, _event_bus) = runtime()?;',
  'require_forum_module_enabled(&host, tenant.id).await?',
  'crate::forum_contribution_manifest()',
  'rustok_forum::ForumWidgetContractService::catalog()',
  'FORUM_PAGE_BUILDER_PREVIEW_ENDPOINT',
  'FORUM_PAGE_BUILDER_PROPERTY_SCHEMA_ENDPOINT',
  'FORUM_PAGE_BUILDER_PROPERTY_VALIDATE_ENDPOINT',
]) {
  requireContains(previewTransport, marker, `Forum transport attestation source missing ${marker}`);
}

const attestationStart = previewTransport.indexOf(
  '#[server(prefix = "/api/fn", endpoint = "forum/page-builder-transport-attestation")]',
);
const previewStart = previewTransport.indexOf(
  '#[server(prefix = "/api/fn", endpoint = "forum/page-builder-widget-preview")]',
);
if (attestationStart < 0 || previewStart <= attestationStart) {
  throw new Error("cannot isolate deployed attestation server-function source");
}
const attestationEndpointSource = previewTransport.slice(attestationStart, previewStart);
for (const forbidden of [
  "ForumWidgetPreviewService",
  "TopicService",
  "ReplyService",
  "forum_topic::",
  "forum_reply::",
  "DatabaseConnection",
]) {
  requireAbsent(
    attestationEndpointSource,
    forbidden,
    `deployed attestation endpoint must not become a Forum data read path via ${forbidden}`,
  );
}

for (const marker of [
  'endpoint = "forum/page-builder-widget-property-schema"',
  'endpoint = "forum/page-builder-widget-property-validate"',
  'require_forum_transport_authorization(&auth, &tenant)?',
  'require_forum_module_enabled(&host, tenant.id).await',
]) {
  requireContains(propertyTransport, marker, `property transport boundary missing ${marker}`);
}
for (const marker of [
  "ForumPageBuilderTransportAttestationResponse",
  "attest_forum_page_builder_transport",
]) {
  requireContains(adminLib, marker, `Forum admin export missing ${marker}`);
}

for (const marker of [
  'router.route(',
  '"/api/fn/{*fn_name}"',
  'handle_server_fns_with_context(',
  'middleware::auth_context::resolve_optional',
  'middleware::tenant::resolve',
]) {
  requireContains(appRouter, marker, `server /api/fn composition missing ${marker}`);
}

for (const [label, source] of [
  ["production Dockerfile", dockerfile],
  ["release Dockerfile", releaseDockerfile],
]) {
  for (const marker of [
    "ARG OCI_REVISION=unknown",
    'org.opencontainers.image.revision="${OCI_REVISION}"',
    "RUSTOK_SOURCE_COMMIT=${OCI_REVISION}",
  ]) {
    requireContains(source, marker, `${label} missing source-revision binding ${marker}`);
  }
}

for (const marker of [
  'spawnSync("git", ["rev-parse", "HEAD"]',
  "shell: false",
  'new URLSearchParams({ challenge }).toString()',
  'method: "POST"',
  '"content-type": "application/x-www-form-urlencoded"',
  "AbortSignal.timeout(REQUEST_TIMEOUT_MS)",
  "requireDeploymentImageDigest(options.deploymentImageDigest)",
  "rmSync(output, { force: true })",
  "sourceHashes(contract)",
  'const challenge = `forum-attest-${randomUUID()}`',
  "requireAuthorizedBody(contract, captured.responseBody, challenge, sourceCommit)",
  "text.includes(challenge)",
  "text.includes(sourceCommit)",
  "credential_environment_names: credentials.environment_names",
  "credential_values_persisted: false",
  "origin_sha256: sha256(baseUrl)",
  "raw_origin_persisted: false",
  "raw_value_persisted: false",
  "raw_body_persisted: false",
  'origin_to_repo_digest_binding: "maintainer_reviewed_external_fact"',
  "cryptographic_origin_to_repo_digest_binding: false",
  "browser_execution_not_claimed: true",
  "runtime_authorization_execution_not_claimed: true",
  "provider_slo_health_not_claimed: true",
  "observed_page_builder_wave_pending: true",
]) {
  requireContains(runner, marker, `deployment attestation runner missing ${marker}`);
}
for (const forbidden of [
  "execSync(",
  "shell: true",
  "raw_origin: baseUrl",
  "authorization_value",
  "cookie_value",
  "raw_response_body",
  "response_body_text:",
  "tenant_id:",
  "actor_id:",
]) {
  requireAbsent(runner, forbidden, `deployment attestation runner must not persist/execute through ${forbidden}`);
}
const staleCleanup = runner.indexOf("rmSync(output, { force: true })");
const requestLoop = runner.indexOf("for (const scenario of contract.scenarios)");
if (staleCleanup < 0 || requestLoop < 0 || staleCleanup > requestLoop) {
  throw new Error("stale deployment attestation output must be removed before network requests");
}

for (const marker of [
  "Status: `source-ready / maintainer-deployment-attestation-execution-pending / browser-execution-pending / runtime-execution-pending / wave-pending`",
  "POST /api/fn/forum/page-builder-transport-attestation",
  "RUSTOK_SOURCE_COMMIT=${OCI_REVISION}",
  "forum_page_builder_server_fn_deployment_attestation_v1",
  "server_fn_deployment_attestation_passed_wave_pending",
  "cryptographic origin-to-RepoDigest",
  "maintainer/infrastructure provenance fact",
  "Provider SLO health remains `unobserved`",
  "No HTTP request, browser, Cargo command, Node verifier, Docker build/inspect, database fixture, formatter, build, workflow or CI execution is claimed",
]) {
  requireContains(packet, marker, `deployment attestation actualization missing ${marker}`);
}

console.log("Forum Page Builder deployed server-function attestation source: ok");
