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
const dto = read("crates", "rustok-page-builder", "src", "dto.rs");
const previewPort = read("crates", "rustok-page-builder", "src", "preview_port.rs");
const flyService = read(
  "crates",
  "rustok-page-builder",
  "src",
  "adapters",
  "fly_service.rs",
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
if (typeof providerVersion !== "string" || providerVersion.length === 0) {
  fail("provider.builder_contract_version is missing");
}
const contract = registry.provider?.preview_runtime_contract;
if (!contract) fail("provider.preview_runtime_contract is missing");
if (contract.context_shape !== "json_object") {
  fail(`unsupported context shape: ${contract.context_shape}`);
}
if (!Number.isInteger(contract.context_max_bytes) || contract.context_max_bytes <= 0) {
  fail("context_max_bytes must be a positive integer");
}
if (!Number.isInteger(contract.scenario_id_max_bytes) || contract.scenario_id_max_bytes <= 0) {
  fail("scenario_id_max_bytes must be a positive integer");
}

requireMarker(dto, `pub struct ${contract.input}`, "preview runtime DTO");
requireMarker(dto, "pub context: serde_json::Value", "preview runtime DTO");
requireMarker(dto, "pub scenario_id: Option<String>", "preview runtime DTO");
requireMarker(dto, `pub ${contract.response_identity}: Option<String>`, "preview response identity");
requireMarker(previewPort, `pub trait ${contract.port}`, "preview rendering port");
requireMarker(previewPort, "input: &PreviewPageBuilderInput", "preview rendering port");

requireMarker(
  flyService,
  "MAX_PREVIEW_RUNTIME_CONTEXT_BYTES: usize = 256 * 1024",
  "preview context limit",
);
if (contract.context_max_bytes !== 256 * 1024) {
  fail(`registry context_max_bytes must be ${256 * 1024}`);
}
requireMarker(
  flyService,
  `MAX_PREVIEW_SCENARIO_ID_BYTES: usize = ${contract.scenario_id_max_bytes}`,
  "preview scenario limit",
);
requireMarker(flyService, "runtime.context.is_object()", "preview context shape validation");
requireMarker(flyService, "serde_json::to_vec(&runtime.context)", "preview context size validation");
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
  pagesBuilder,
  `impl ${contract.port} for PagesPageBuilderRenderer`,
  "Pages preview port",
);
requireMarker(
  pagesBuilder,
  "render_runtime_document_html(",
  "Pages runtime materialization",
);
requireMarker(
  pagesBuilder,
  "input.runtime.context.clone()",
  "Pages runtime context binding",
);
requireMarker(adminRuntime, `${contract.input}::new`, "admin preview runtime request");
requireMarker(adminRuntime, "response.runtime_scenario_id", "admin preview response identity");
requireMarker(adminRuntime, "current_runtime_context", "admin preview stale-context guard");

console.log("[verify-page-builder-preview-runtime-contract] PASS");
