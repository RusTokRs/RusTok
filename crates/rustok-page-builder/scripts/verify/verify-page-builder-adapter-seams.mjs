#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..", "..", "..", "..");
const moduleRoot = path.join(repoRoot, "crates", "rustok-page-builder");
const contract = JSON.parse(
  fs.readFileSync(path.join(moduleRoot, "contracts", "page-builder-service-boundary.json"), "utf8"),
);
const service = fs.readFileSync(path.join(moduleRoot, "src", "service.rs"), "utf8");
const flyService = fs.readFileSync(
  path.join(moduleRoot, "src", "adapters", "fly_service.rs"),
  "utf8",
);
const adapters = fs.readFileSync(path.join(moduleRoot, "src", "adapters.rs"), "utf8");
const telemetry = fs.readFileSync(path.join(moduleRoot, "src", "runtime_telemetry.rs"), "utf8");
const readme = fs.readFileSync(path.join(moduleRoot, "docs", "README.md"), "utf8");
const implementationPlan = fs.readFileSync(
  path.join(moduleRoot, "docs", "implementation-plan.md"),
  "utf8",
);

function fail(message) {
  console.error(`[verify-page-builder-adapter-seams] ${message}`);
  process.exit(1);
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) fail(`${label} is missing ${marker}`);
}

if (contract.domain_owner !== "fly") fail("Fly must remain the Page Builder domain owner");
requireMarker(flyService, `pub struct ${contract.service}`, "Fly service");
requireMarker(adapters, `pub use fly_service::${contract.service}`, "adapter exports");

for (const port of Object.values(contract.ports ?? {})) {
  requireMarker(service + telemetry, port.trait, "service ports");
  for (const method of port.methods ?? []) {
    requireMarker(service + telemetry, method, `port ${port.trait}`);
  }
}

for (const guard of contract.guards ?? []) {
  requireMarker(service, `pub struct ${guard}`, "service guards");
}

for (const entrypoint of contract.transport_entrypoints ?? []) {
  requireMarker(adapters, entrypoint, "transport entrypoints");
}

for (const capability of contract.capabilities ?? []) {
  requireMarker(service, `BuilderCapabilityKind::${capability[0].toUpperCase()}${capability.slice(1)}`, "capability service");
}

const currentSources = [service, flyService, adapters, telemetry, readme, implementationPlan];
for (const forbidden of contract.forbidden_symbols ?? []) {
  if (currentSources.some((source) => source.includes(forbidden))) {
    fail(`obsolete symbol '${forbidden}' must not exist in the current service boundary`);
  }
}

for (const marker of [
  "FlyProjectInspection::decode_with",
  "inspection.require_valid()",
  ".renderer",
  ".render_preview(context, &input.project_data)",
  "PageBuilderRuntimeCallEvidence::render_preview",
  "PageBuilderRuntimeCallEvidence::load_project",
  "PageBuilderRuntimeCallEvidence::save_project",
]) {
  requireMarker(flyService, marker, "Fly-backed service lifecycle");
}

requireMarker(readme, "src/adapters/fly_service.rs", "local documentation");
requireMarker(readme, "FlyAdapterBackedPageBuilderService", "local documentation");
requireMarker(implementationPlan, "page-builder-service-boundary.json", "implementation plan");

console.log("[verify-page-builder-adapter-seams] PASS");
