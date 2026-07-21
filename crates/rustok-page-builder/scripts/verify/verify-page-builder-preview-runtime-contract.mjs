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
if (contract.context_shape !== "json_object") {
  fail(`unsupported context shape: ${contract.context_shape}`);
}
if (!Number.isInteger(contract.context_max_bytes) || contract.context_max_bytes <= 0) {
  fail("context_max_bytes must be a positive integer");
}
if (!Number.isInteger(contract.scenario_id_max_bytes) || contract.scenario_id_max_bytes <= 0) {
  fail("scenario_id_max_bytes must be a positive integer");
}
if (staticContract.raw_context_persisted !== false) {
  fail("static materialization must not persist raw runtime context");
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

requireMarker(
  staticMaterialization,
  `pub const PAGE_BUILDER_STATIC_MATERIALIZATION_FORMAT: &str`,
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
  "case.document_hash.as_deref()",
  "Some(page.content_hash.as_str())",
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

console.log("[verify-page-builder-preview-runtime-contract] PASS");
