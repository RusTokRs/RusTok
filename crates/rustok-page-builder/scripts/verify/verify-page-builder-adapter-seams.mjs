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

function fail(message) {
  console.error(`[verify-page-builder-adapter-seams] ${message}`);
  process.exit(1);
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) fail(`${label} is missing ${marker}`);
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

for (const port of Object.values(contract.ports ?? {})) {
  requireMarker(service + telemetry, port.trait, "service ports");
  for (const method of port.methods ?? []) {
    requireMarker(service + telemetry, method, `port ${port.trait}`);
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
  flyService,
  adapters,
  composition,
  telemetry,
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
  ".render_preview(context, &input.project_data)",
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
requireMarker(
  pagesBuilder,
  pagesConsumer.persisted_result_marker,
  "Pages persisted capability result",
);
for (const forbidden of pagesConsumer.forbidden_symbols ?? []) {
  if (pagesBuilder.includes(forbidden)) {
    fail(`Pages production consumer contains obsolete save side-channel: ${forbidden}`);
  }
}

const publishPaths = isolateEntrypoint(
  pagesBuilder,
  `#[cfg(feature = "ssr")]\nasync fn ${pagesConsumer.server_entrypoint}`,
  pagesConsumer.client_entrypoint_marker,
  "Pages publish",
);
requireOrderedMarkers(
  publishPaths.server,
  pagesConsumer.required_server_order,
  "Pages SSR authorization/composition order",
);
rejectBeforeDispatch(
  publishPaths.server,
  pagesConsumer.dispatch_marker,
  pagesConsumer.forbidden_before_dispatch,
  "Pages SSR publish path",
);

const previewPaths = isolateEntrypoint(
  pagesBuilder,
  `#[cfg(feature = "ssr")]\nasync fn ${pagesConsumer.preview_server_entrypoint}`,
  pagesConsumer.preview_client_entrypoint_marker,
  "Pages preview",
);
requireOrderedMarkers(
  previewPaths.server,
  pagesConsumer.preview_required_server_order,
  "Pages SSR preview authorization/composition order",
);
rejectBeforeDispatch(
  previewPaths.server,
  pagesConsumer.preview_dispatch_marker,
  pagesConsumer.preview_forbidden_before_dispatch,
  "Pages SSR preview path",
);
requireMarker(
  previewPaths.client,
  pagesConsumer.preview_transport_marker,
  "Pages preview client transport",
);
requireMarker(
  pagesBuilder,
  pagesConsumer.preview_renderer_context_guard,
  "Pages preview renderer tenant guard",
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
