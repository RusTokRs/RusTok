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

function fail(message) {
  console.error(`[verify-page-builder-adapter-seams] ${message}`);
  process.exit(1);
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) fail(`${label} is missing ${marker}`);
}

function requireOrderedMarkers(source, markers, label) {
  let previousIndex = -1;
  for (const marker of markers) {
    const index = source.indexOf(marker);
    if (index < 0) fail(`${label} is missing ${marker}`);
    if (index <= previousIndex) fail(`${label} is out of order at ${marker}`);
    previousIndex = index;
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
  compositionRoot.invocation_order ?? [],
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

for (const marker of [
  "compose_fly_page_builder_handlers",
  "PagesPageBuilderProjectStore",
  "PagesPageBuilderRenderer",
  "PageBuilderRequestAuth::new",
  ".with_deadline(PAGE_BUILDER_PORT_DEADLINE)",
  ".with_idempotency_key",
  ".handle(",
  "context.tenant_id.as_str()",
  "context.actor.id.as_str()",
  "ensure_context(context)?;",
  "persisted_revision(&saved_page)",
]) {
  requireMarker(pagesBuilder, marker, "Pages production consumer");
}

const ssrPublishMarker = '#[cfg(feature = "ssr")]\nasync fn execute_publish';
const clientPublishMarker =
  '#[cfg(not(feature = "ssr"))]\nasync fn execute_publish';
requireMarker(pagesBuilder, ssrPublishMarker, "Pages SSR composition path");
requireMarker(pagesBuilder, clientPublishMarker, "Pages client transport path");
const ssrPublishStart = pagesBuilder.indexOf(ssrPublishMarker);
const clientPublishStart = pagesBuilder.indexOf(clientPublishMarker);
if (clientPublishStart <= ssrPublishStart) {
  fail("Pages SSR composition path must precede the client transport path");
}
const pagesSsrPublish = pagesBuilder.slice(ssrPublishStart, clientPublishStart);
requireOrderedMarkers(
  pagesSsrPublish,
  [
    "fetch_current_user",
    "PageBuilderRequestAuth::new",
    "compose_fly_page_builder_handlers",
    ".handle(",
  ],
  "Pages SSR authorization and composition",
);
if (pagesSsrPublish.includes("transport::fetch_page(")) {
  fail("Pages SSR facade must not read persistence before canonical authorization");
}

requireMarker(
  pagesAdminManifest,
  '"rustok-page-builder/server"',
  "Pages SSR feature composition",
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

console.log("[verify-page-builder-adapter-seams] PASS");
