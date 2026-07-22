#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..", "..", "..", "..");
const moduleRoot = path.join(repoRoot, "crates", "rustok-page-builder");
const contract = JSON.parse(
  fs.readFileSync(
    path.join(moduleRoot, "contracts", "page-builder-service-boundary.json"),
    "utf8",
  ),
);
const service = fs.readFileSync(
  path.join(moduleRoot, "src", "service.rs"),
  "utf8",
);
const dto = fs.readFileSync(path.join(moduleRoot, "src", "dto.rs"), "utf8");
const previewPort = fs.readFileSync(
  path.join(moduleRoot, "src", "preview_port.rs"),
  "utf8",
);
const flyService = fs.readFileSync(
  path.join(moduleRoot, "src", "adapters", "fly_service.rs"),
  "utf8",
);
const adapters = fs.readFileSync(
  path.join(moduleRoot, "src", "adapters.rs"),
  "utf8",
);
const composition = fs.readFileSync(
  path.join(moduleRoot, "src", "composition.rs"),
  "utf8",
);
const telemetry = fs.readFileSync(
  path.join(moduleRoot, "src", "runtime_telemetry.rs"),
  "utf8",
);
const readme = fs.readFileSync(
  path.join(moduleRoot, "docs", "README.md"),
  "utf8",
);
const implementationPlan = fs.readFileSync(
  path.join(moduleRoot, "docs", "implementation-plan.md"),
  "utf8",
);
const pagesBuilder = fs.readFileSync(
  path.join(repoRoot, "crates", "rustok-pages", "admin", "src", "builder.rs"),
  "utf8",
);
const pagesAdminManifest = fs.readFileSync(
  path.join(repoRoot, "crates", "rustok-pages", "admin", "Cargo.toml"),
  "utf8",
);
const pageBuilderAdminRuntime = fs.readFileSync(
  path.join(moduleRoot, "admin", "src", "editor", "runtime.rs"),
  "utf8",
);
const pageBuilderServerPreview = fs.readFileSync(
  path.join(moduleRoot, "admin", "src", "editor", "server_preview.rs"),
  "utf8",
);
const pageBuilderModularCanvas = fs.readFileSync(
  path.join(moduleRoot, "admin", "src", "editor", "modular_canvas.rs"),
  "utf8",
);

const namedSources = {
  service,
  dto,
  preview_port: previewPort,
  telemetry,
};

function fail(message) {
  console.error(`[verify-page-builder-adapter-seams] ${message}`);
  process.exit(1);
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) fail(`${label} is missing ${marker}`);
}

function requireOccurrenceCount(source, marker, expected, label) {
  const actual = source.split(marker).length - 1;
  if (actual !== expected) {
    fail(`${label} expected ${expected} occurrence(s) of ${marker}, found ${actual}`);
  }
}

function requireOrderedMarkers(source, markers, label) {
  let previousIndex = -1;
  for (const marker of markers ?? []) {
    const index = source.indexOf(marker);
    if (index < 0) fail(`${label} is missing ${marker}`);
    if (index <= previousIndex) fail(`${label} is out of order at ${marker}`);
    previousIndex = index;
  }
}

function isolateEntrypoint(source, serverMarker, clientMarker, label) {
  requireMarker(source, serverMarker, `${label} server path`);
  requireMarker(source, clientMarker, `${label} client path`);
  const serverStart = source.indexOf(serverMarker);
  const clientStart = source.indexOf(clientMarker, serverStart);
  if (serverStart < 0 || clientStart <= serverStart) {
    fail(`${label} server path cannot be isolated from the client path`);
  }
  return {
    server: source.slice(serverStart, clientStart),
    client: source.slice(clientStart),
  };
}

function isolateRange(source, startMarker, endMarker, label) {
  requireMarker(source, startMarker, `${label} start`);
  requireMarker(source, endMarker, `${label} end`);
  const start = source.indexOf(startMarker);
  const end = source.indexOf(endMarker, start + startMarker.length);
  if (start < 0 || end <= start) {
    fail(`${label} cannot be isolated`);
  }
  return source.slice(start, end);
}

function rejectBeforeDispatch(source, dispatchMarker, forbiddenMarkers, label) {
  const dispatchIndex = source.indexOf(dispatchMarker);
  if (dispatchIndex < 0) fail(`${label} is missing handler dispatch`);
  const preDispatchSource = source.slice(0, dispatchIndex);
  for (const forbidden of forbiddenMarkers ?? []) {
    if (preDispatchSource.includes(forbidden)) {
      fail(`${label} accesses tenant persistence before authorization: ${forbidden}`);
    }
  }
}

if (contract.domain_owner !== "fly")
  fail("Fly must remain the Page Builder domain owner");
requireMarker(flyService, `pub struct ${contract.service}`, "Fly service");
requireMarker(
  adapters,
  `pub use fly_service::${contract.service}`,
  "adapter exports",
);

for (const [name, dtoContract] of Object.entries(contract.dto_contracts ?? {})) {
  const source = namedSources[dtoContract.source];
  if (!source) fail(`DTO contract ${name} names unknown source ${dtoContract.source}`);
  for (const marker of dtoContract.markers ?? []) {
    requireMarker(source, marker, `DTO contract ${name}`);
  }
}

for (const port of Object.values(contract.ports ?? {})) {
  const portSource = namedSources[port.source] ?? service + telemetry;
  requireMarker(portSource, port.trait, "service ports");
  for (const method of port.methods ?? []) {
    requireMarker(portSource, method, `port ${port.trait}`);
  }
  if (port.input) {
    requireMarker(portSource, port.input, `port ${port.trait} input`);
  }
  for (const marker of port.runtime_validation_markers ?? []) {
    requireMarker(flyService, marker, `port ${port.trait} runtime validation`);
  }
  if (port.save_result) {
    requireMarker(service, `pub struct ${port.save_result}`, "persistence save result");
    requireMarker(
      service,
      `PageBuilderServiceResult<${port.save_result}>`,
      "persistence save signature",
    );
    requireOrderedMarkers(
      flyService,
      port.save_result_order,
      "persistence save result validation",
    );
  }
  if (port.runtime_order) {
    requireOrderedMarkers(
      flyService,
      port.runtime_order,
      "preview runtime context flow",
    );
  }
}

for (const guard of contract.guards ?? []) {
  requireMarker(service, `pub struct ${guard}`, "service guards");
}

const compositionRoot = contract.composition_root;
if (!compositionRoot) fail("service contract is missing composition_root");
requireMarker(
  composition,
  `pub fn ${compositionRoot.default_entrypoint}`,
  "default composition entrypoint",
);
requireMarker(
  composition,
  `pub fn ${compositionRoot.configured_entrypoint}`,
  "configured composition entrypoint",
);
requireMarker(
  composition,
  compositionRoot.preview_port,
  "composition preview port",
);
requireMarker(composition, "flags.validate()?", "composition rollout validation");
requireOrderedMarkers(
  composition,
  compositionRoot.invocation_order,
  "composition root invocation",
);

for (const entrypoint of contract.transport_entrypoints ?? []) {
  requireMarker(adapters, entrypoint, "transport entrypoints");
}

for (const capability of contract.capabilities ?? []) {
  requireMarker(
    service,
    `BuilderCapabilityKind::${capability[0].toUpperCase()}${capability.slice(1)}`,
    "capability service",
  );
}

const currentSources = [
  service,
  dto,
  previewPort,
  flyService,
  adapters,
  composition,
  telemetry,
  pagesBuilder,
  readme,
  implementationPlan,
];
for (const forbidden of contract.forbidden_symbols ?? []) {
  if (currentSources.some((source) => source.includes(forbidden))) {
    fail(
      `obsolete symbol '${forbidden}' must not exist in the current service boundary`,
    );
  }
}

for (const marker of [
  "FlyProjectInspection::decode_with",
  "inspection.require_valid()",
  ".renderer",
  ".render_preview(context, &input)",
  "PageBuilderRuntimeCallEvidence::render_preview",
  "PageBuilderRuntimeCallEvidence::load_project",
  "PageBuilderRuntimeCallEvidence::save_project",
]) {
  requireMarker(flyService, marker, "Fly-backed service lifecycle");
}

const pagesConsumer = contract.production_consumers?.pages;
if (!pagesConsumer) fail("service contract is missing the Pages production consumer");
for (const marker of pagesConsumer.tenant_ports ?? []) {
  requireMarker(pagesBuilder, marker, "Pages tenant-scoped ports");
}
for (const marker of pagesConsumer.tenant_context_guards ?? []) {
  requireMarker(pagesBuilder, marker, "Pages tenant context guard");
}
for (const marker of pagesConsumer.preview_runtime_markers ?? []) {
  requireMarker(pagesBuilder, marker, "Pages preview runtime port");
}
requireMarker(
  pagesBuilder,
  pagesConsumer.preview_renderer_context_guard,
  "Pages preview renderer tenant guard",
);
requireMarker(
  pagesBuilder,
  pagesConsumer.persisted_result_marker,
  "Pages persisted capability result",
);
for (const forbidden of pagesConsumer.forbidden_symbols ?? []) {
  if (pagesBuilder.includes(forbidden)) {
    fail(`Pages production consumer contains obsolete capability path: ${forbidden}`);
  }
}

for (const marker of pagesConsumer.capability_server_function_markers ?? []) {
  requireMarker(pagesBuilder, marker, "Pages capability server function");
}
for (const marker of pagesConsumer.capability_wrapper_markers ?? []) {
  requireMarker(pagesBuilder, marker, "Pages capability wrapper");
}

const capabilityPaths = isolateEntrypoint(
  pagesBuilder,
  `#[cfg(feature = "ssr")]\nasync fn ${pagesConsumer.capability_dispatch_helper}`,
  pagesConsumer.capability_client_entrypoint_marker,
  "Pages capability dispatch",
);
requireOrderedMarkers(
  capabilityPaths.server,
  pagesConsumer.capability_required_server_order,
  "Pages SSR authorization/composition order",
);
rejectBeforeDispatch(
  capabilityPaths.server,
  pagesConsumer.capability_dispatch_marker,
  pagesConsumer.capability_forbidden_before_dispatch,
  "Pages SSR capability path",
);

const capabilityClient = isolateRange(
  pagesBuilder,
  pagesConsumer.capability_client_entrypoint_marker,
  pagesConsumer.capability_client_end_marker,
  "Pages client capability transport",
);
requireMarker(
  capabilityClient,
  pagesConsumer.capability_client_transport_marker,
  "Pages client capability transport",
);
for (const forbidden of pagesConsumer.capability_client_forbidden_symbols ?? []) {
  if (capabilityClient.includes(forbidden)) {
    fail(`Pages client capability transport contains a parallel write path: ${forbidden}`);
  }
}
requireOccurrenceCount(
  pagesBuilder,
  pagesConsumer.composition_call_marker,
  pagesConsumer.composition_call_count,
  "Pages composition root call",
);
requireMarker(
  pagesAdminManifest,
  pagesConsumer.server_feature_marker,
  "Pages SSR feature composition",
);

const adminPreview = pagesConsumer.admin_preview;
if (!adminPreview) fail("Pages production consumer is missing admin_preview guardrail");
for (const marker of adminPreview.runtime_markers ?? []) {
  requireMarker(pageBuilderAdminRuntime, marker, "Page Builder admin preview runtime");
}
for (const marker of adminPreview.surface_markers ?? []) {
  requireMarker(pageBuilderServerPreview, marker, "Page Builder server preview surface");
}
requireMarker(
  pageBuilderModularCanvas,
  adminPreview.host_marker,
  "Page Builder server preview host",
);

requireMarker(readme, "src/adapters/fly_service.rs", "local documentation");
requireMarker(
  readme,
  "compose_fly_page_builder_handlers",
  "local documentation",
);
requireMarker(
  implementationPlan,
  "page-builder-service-boundary.json",
  "implementation plan",
);
requireMarker(
  implementationPlan,
  "compose_fly_page_builder_handlers",
  "implementation plan",
);
requireMarker(
  implementationPlan,
  "rustok-pages",
  "production consumer rollout plan",
);
requireMarker(
  implementationPlan,
  "server preview",
  "production preview rollout plan",
);

console.log("[verify-page-builder-adapter-seams] PASS");
