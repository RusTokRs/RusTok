#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..", "..", "..", "..");
const read = (...segments) => fs.readFileSync(path.join(repoRoot, ...segments), "utf8");

const registry = JSON.parse(
  read("crates", "rustok-page-builder", "contracts", "page-builder-fba-registry.json"),
);
const wavePackets = [
  "pages-wave0-dry-run-evidence.json",
  "pages-wave1-readiness-draft.json",
].map((filename) => ({
  filename,
  packet: JSON.parse(
    read("crates", "rustok-page-builder", "contracts", "evidence", filename),
  ),
}));
const manifest = read("crates", "rustok-page-builder", "Cargo.toml");
const dto = read("crates", "rustok-page-builder", "src", "dto.rs");
const previewPort = read("crates", "rustok-page-builder", "src", "preview_port.rs");
const flyService = read(
  "crates",
  "rustok-page-builder",
  "src",
  "adapters",
  "fly_service.rs",
);
const staticLanding = read("crates", "rustok-page-builder", "src", "static_landing.rs");
const staticMaterialization = read(
  "crates",
  "rustok-page-builder",
  "src",
  "static_landing_materialization.rs",
);
const health = read("crates", "rustok-page-builder", "src", "health.rs");
const pagesBuilder = read("crates", "rustok-pages", "admin", "src", "builder.rs");
const pagesArtifact = read(
  "crates",
  "rustok-pages",
  "src",
  "services",
  "page_builder_artifact.rs",
);
const pagesArtifactEntity = read(
  "crates",
  "rustok-pages",
  "src",
  "entities",
  "page_static_landing_artifact.rs",
);
const pagesMigrations = read("crates", "rustok-pages", "src", "migrations", "mod.rs");
const pagesMaterializationMigration = read(
  "crates",
  "rustok-pages",
  "src",
  "migrations",
  "m20260721_000006_add_static_landing_materialization_evidence.rs",
);
const adminRuntime = read(
  "crates",
  "rustok-page-builder",
  "admin",
  "src",
  "editor",
  "runtime.rs",
);

function fail(message) {
  console.error(`[verify-page-builder-preview-runtime-contract] ${message}`);
  process.exit(1);
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) fail(`${label} is missing ${marker}`);
}

const providerVersion = registry.provider?.builder_contract_version;
const consumerMinVersion = registry.provider?.consumer_min_version;
if (typeof providerVersion !== "string" || providerVersion.length === 0) {
  fail("provider.builder_contract_version is missing");
}
if (typeof consumerMinVersion !== "string" || consumerMinVersion.length === 0) {
  fail("provider.consumer_min_version is missing");
}
const contract = registry.provider?.preview_runtime_contract;
if (!contract) fail("provider.preview_runtime_contract is missing");
const staticContract = registry.provider?.static_materialization_contract;
if (!staticContract) fail("provider.static_materialization_contract is missing");
const pagesConsumer = registry.consumers?.find((consumer) => consumer.module_slug === "pages");
if (!pagesConsumer) fail("Pages consumer is missing");
const persistence = pagesConsumer.materialization_persistence;
if (!persistence || persistence.state !== "integrated") {
  fail("Pages materialization persistence is not integrated");
}
if (contract.context_shape !== "json_object") {
  fail(`unsupported context shape: ${contract.context_shape}`);
}
if (!Number.isInteger(contract.context_max_bytes) || contract.context_max_bytes <= 0) {
  fail("context_max_bytes must be a positive integer");
}
if (!Number.isInteger(contract.scenario_id_max_bytes) || contract.scenario_id_max_bytes <= 0) {
  fail("scenario_id_max_bytes must be a positive integer");
}
if (staticContract.raw_context_persisted !== false || persistence.raw_context_persisted !== false) {
  fail("static materialization must not persist raw runtime context");
}
if (staticContract.evidence_hash_algorithm !== "sha256") {
  fail("static materialization evidence_hash_algorithm must be sha256");
}
if (staticContract.snapshot_document_hash_algorithm !== "fly_project_hash_fnv1a64") {
  fail("static materialization snapshot_document_hash_algorithm must be fly_project_hash_fnv1a64");
}
if (staticContract.preview_static_parity !== "document_html") {
  fail("static materialization preview_static_parity must be document_html");
}
const expectedUniqueKey = [
  "tenant_id",
  "page_id",
  "locale",
  "build_hash",
  "materialization_hash",
];
if (JSON.stringify(persistence.unique_key) !== JSON.stringify(expectedUniqueKey)) {
  fail("Pages materialization unique key is invalid");
}
if (persistence.partial_evidence !== "rejected") {
  fail("Pages must reject partial materialization evidence");
}

for (const { filename, packet } of wavePackets) {
  if (packet.metadata?.provider?.builder_contract_version !== providerVersion) {
    fail(`${filename} builder_contract_version does not match registry provider`);
  }
  if (packet.metadata?.provider?.consumer_min_version !== consumerMinVersion) {
    fail(`${filename} consumer_min_version does not match registry provider`);
  }
}

requireMarker(dto, `pub struct ${contract.input}`, "preview runtime DTO");
requireMarker(dto, "pub context: serde_json::Value", "preview runtime DTO");
requireMarker(dto, "pub scenario_id: Option<String>", "preview runtime DTO");
requireMarker(dto, `pub ${contract.response_identity}: Option<String>`, "preview response identity");
requireMarker(
  dto,
  `MAX_PREVIEW_RUNTIME_CONTEXT_BYTES: usize = ${contract.context_max_bytes / 1024} * 1024`,
  "preview context limit",
);
requireMarker(
  dto,
  `MAX_PREVIEW_SCENARIO_ID_BYTES: usize = ${contract.scenario_id_max_bytes}`,
  "preview scenario limit",
);
requireMarker(dto, "pub fn validate(&self)", "canonical preview runtime validator");
requireMarker(dto, "self.context.is_object()", "preview context shape validation");
requireMarker(dto, "serde_json::to_vec(&self.context)", "preview context size validation");
requireMarker(previewPort, `pub trait ${contract.port}`, "preview rendering port");
requireMarker(previewPort, "input: &PreviewPageBuilderInput", "preview rendering port");
requireMarker(flyService, ".runtime\n            .validate()", "service runtime validation");
requireMarker(flyService, "render_preview(context, &input)", "canonical preview port dispatch");
requireMarker(
  flyService,
  `${contract.response_identity}: input.runtime.scenario_id`,
  "preview response identity",
);
requireMarker(
  health,
  `builder_contract_version: "${providerVersion}"`,
  "provider health evidence version",
);

requireMarker(manifest, "sha2.workspace = true", "materialization SHA-256 dependency");
requireMarker(
  staticMaterialization,
  "pub const PAGE_BUILDER_STATIC_MATERIALIZATION_FORMAT: &str",
  "static materialization format",
);
requireMarker(staticMaterialization, staticContract.format, "static materialization format");
requireMarker(
  staticMaterialization,
  `pub struct ${staticContract.envelope}`,
  "static materialization envelope",
);
requireMarker(
  staticMaterialization,
  `pub struct ${staticContract.identity}`,
  "static materialization identity",
);
requireMarker(
  staticMaterialization,
  `Vec<${staticContract.snapshot}>`,
  "static runtime snapshots",
);
for (const marker of [
  "runtime.validate()",
  "RuntimeScenarioRenderSnapshot::capture",
  "materialize_project_with_runtime_context",
  "stable_hash(&runtime.context)",
  "stable_hash(&runtime_snapshots)",
  "Sha256::digest(bytes)",
  "ProjectHash::from_bytes(page.document_html.as_bytes()).hex()",
  "case.document_hash.as_deref()",
]) {
  requireMarker(staticMaterialization, marker, "static runtime materialization");
}
for (const field of [
  staticContract.context_evidence,
  staticContract.scenario_evidence,
  staticContract.snapshot_evidence,
]) {
  requireMarker(staticMaterialization, `pub ${field}`, "static materialization evidence");
}
if (staticMaterialization.includes("pub context: Value")) {
  fail("static materialization identity must not persist raw runtime context");
}
for (const marker of [
  "pub(crate) fn prepare_document",
  "pub(crate) fn compile_prepared_document",
  "pub(crate) fn render_policy",
  "Re-run the public artifact security policy on the exact document being built",
]) {
  requireMarker(staticLanding, marker, "prepared static landing seam");
}

requireMarker(
  pagesBuilder,
  `impl ${contract.port} for PagesPageBuilderRenderer`,
  "Pages preview port",
);
requireMarker(pagesBuilder, "render_runtime_document_html(", "Pages runtime materialization");
requireMarker(pagesBuilder, "input.runtime.context.clone()", "Pages runtime context binding");
requireMarker(adminRuntime, `${contract.input}::new`, "admin preview runtime request");
requireMarker(adminRuntime, "response.runtime_scenario_id", "admin preview response identity");
requireMarker(adminRuntime, "current_runtime_context", "admin preview stale-context guard");

for (const [field, rustType] of [
  [persistence.materialization_hash_column, "Option<String>"],
  [persistence.materialization_identity_column, "Option<Json>"],
  [persistence.runtime_snapshots_column, "Option<Json>"],
]) {
  requireMarker(pagesArtifactEntity, `pub ${field}: ${rustType}`, "Pages artifact evidence entity");
}
requireMarker(
  pagesMigrations,
  "m20260721_000006_add_static_landing_materialization_evidence",
  "Pages materialization migration registration",
);
for (const marker of [
  "MaterializationHash",
  "MaterializationIdentity",
  "RuntimeSnapshots",
  ".col(PageStaticLandingArtifacts::BuildHash)",
  ".col(PageStaticLandingArtifacts::MaterializationHash)",
  ".unique()",
]) {
  requireMarker(pagesMaterializationMigration, marker, "Pages materialization migration");
}
for (const marker of [
  "compile_materialized_static_landing(",
  "PageBuilderPreviewRuntime::default()",
  "materialization_hash: Set(Some(",
  "materialization_identity: Set(Some(",
  "runtime_snapshots: Set(Some(",
  "page_static_landing_artifact::Column::MaterializationHash",
  "(None, None, None) => Ok(())",
  "stored landing materialization evidence is partial",
  "PageBuilderMaterializedStaticLandingArtifact",
  ".verify_integrity()",
  "materialization_hash: record.materialization_hash",
]) {
  requireMarker(pagesArtifact, marker, "Pages materialization persistence");
}
for (const forbidden of [
  "pub runtime_context:",
  "runtime_context: Set(",
  "raw_runtime_context",
]) {
  if (pagesArtifact.includes(forbidden) || pagesArtifactEntity.includes(forbidden)) {
    fail(`Pages materialization persistence contains forbidden raw context marker: ${forbidden}`);
  }
}

console.log("[verify-page-builder-preview-runtime-contract] PASS");
