#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..", "..", "..", "..");
const read = (relativePath) =>
  fs.readFileSync(path.join(repoRoot, relativePath), "utf8");

const contract = JSON.parse(
  read("crates/rustok-page-builder/contracts/page-builder-consumer-properties.json"),
);
const providerContract = read(contract.provider.contract_source);
const providerPanel = read(contract.provider.panel_source);
const providerModuleExport = read(contract.provider.module_export_source);
const providerPanelExport = read(contract.provider.panel_export_source);
const providerCanvas = read(contract.provider.composition_source);
const moduleManifestTooling = read("crates/rustok-build/src/module_manifest_contribution.rs");
const pagesContributions = read(contract.pages_consumer.contribution_source);
const pagesContributionBuild = read("crates/rustok-pages/admin/build.rs");
const pagesModuleManifest = read("crates/rustok-pages/rustok-module.toml");
const pagesOwnerPort = read(contract.pages_consumer.owner_port_source);
const pagesOwnerPortProduction = pagesOwnerPort.split("#[cfg(test)]")[0];
const pagesBoundary = read(contract.pages_consumer.composition_source);
const pagesWorkspace = read(contract.pages_consumer.workspace_source);
const pagesPublishedSurface = read(contract.pages_consumer.published_surface_source);
const parityPlan = read(contract.parity_plan);

function fail(message) {
  console.error(`[verify-pages-metadata-properties] ${message}`);
  process.exit(1);
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) fail(`${label} is missing ${marker}`);
}

function forbidMarker(source, marker, label) {
  if (source.includes(marker)) fail(`${label} still contains ${marker}`);
}

function requireOrderedMarkers(source, markers, label) {
  let previous = -1;
  for (const marker of markers) {
    const index = source.indexOf(marker, previous + 1);
    if (index < 0) fail(`${label} is missing or out of order at ${marker}`);
    previous = index;
  }
}

if (
  contract.status !== "metadata_surface_cutover_complete" ||
  contract.format !== "page_builder_consumer_properties_v1" ||
  contract.pages_consumer.owner_persistence !== "pages" ||
  contract.pages_consumer.document_revision_independent !== true ||
  contract.pages_consumer.fly_document_write !== false ||
  contract.executed_evidence !== "pending"
) {
  fail("consumer metadata property contract status or ownership is invalid");
}

if (
  JSON.stringify(contract.provider.identity_binding) !==
    JSON.stringify([
      "contribution_id",
      "property_editor_id",
      "provider",
      "component_type",
    ]) ||
  contract.pages_consumer.provider !== "rustok.pages"
) {
  fail("consumer property provider identity binding is invalid");
}

if (
  contract.provider.standalone_host_surface.component !== "ConsumerPropertiesPanel" ||
  contract.provider.standalone_host_surface.state !== "source_ready" ||
  contract.provider.standalone_host_surface.requires_fly_canvas !== false ||
  JSON.stringify(contract.provider.standalone_host_surface.required_inputs) !==
    JSON.stringify([
      "ConsumerPropertyEditorRuntime",
      "ContributionAssemblyResult",
    ])
) {
  fail("standalone consumer property host surface contract is invalid");
}

if (
  contract.pages_consumer.draft_surface.state !== "source_connected" ||
  contract.pages_consumer.draft_surface.host !== "fly_properties_column" ||
  contract.pages_consumer.draft_surface.fly_canvas_mounted !== true ||
  contract.pages_consumer.published_surface.component !==
    "PagesPublishedMetadataSurface" ||
  contract.pages_consumer.published_surface.state !== "source_connected" ||
  contract.pages_consumer.published_surface.selection !==
    "selected_published_page_only" ||
  contract.pages_consumer.published_surface.admission !==
    "exact_status_published_case_insensitive" ||
  contract.pages_consumer.published_surface.draft_hidden !== true ||
  contract.pages_consumer.published_surface.archived_hidden !== true ||
  contract.pages_consumer.published_surface.missing_selection_hidden !== true ||
  contract.pages_consumer.published_surface.fly_canvas_mounted !== false ||
  contract.pages_consumer.published_surface.document_authoring_mounted !== false ||
  contract.pages_consumer.published_surface.runtime !==
    "existing_pages_metadata_property_runtime" ||
  contract.pages_consumer.published_surface.contribution_assembly !==
    "pages_admin_contribution_policy" ||
  contract.pages_consumer.published_surface.persistence !==
    "delegated_to_pages_metadata_owner_port"
) {
  fail("Pages draft or published metadata surface contract is invalid");
}

if (
  contract.pages_consumer.legacy_form.state !== "removed" ||
  contract.pages_consumer.legacy_form.component !== "PageMetadataEditor" ||
  contract.pages_consumer.legacy_form.direct_persistence_path_removed !== true ||
  contract.pages_consumer.legacy_form.replacement !==
    "registered_consumer_property_surfaces"
) {
  fail("legacy metadata cutover contract is invalid");
}

const metadataEvidence =
  contract.pages_consumer.source_evidence?.metadata_revision_isolation;
if (
  metadataEvidence?.state !== "source_ready_execution_pending" ||
  metadataEvidence?.contract !==
    "crates/rustok-pages/contracts/evidence/pages-metadata-revision-isolation-source.json" ||
  metadataEvidence?.verifier !==
    "crates/rustok-pages/scripts/verify/verify-pages-metadata-revision-isolation.mjs"
) {
  fail("metadata revision/isolation source evidence registration is invalid");
}

const publishedSurfaceEvidence =
  contract.pages_consumer.source_evidence?.published_metadata_surface;
if (
  publishedSurfaceEvidence?.state !== "source_ready_execution_pending" ||
  publishedSurfaceEvidence?.contract !==
    "crates/rustok-pages/contracts/evidence/pages-published-metadata-surface-source.json" ||
  publishedSurfaceEvidence?.verifier !==
    "crates/rustok-pages/scripts/verify/verify-pages-published-metadata-surface.mjs"
) {
  fail("published metadata surface source evidence registration is invalid");
}

for (const marker of [
  '"page_builder_consumer_properties_v1"',
  "pub struct ConsumerPropertyEditorSchema",
  "pub struct ConsumerPropertyEditorSnapshot",
  "pub struct SaveConsumerPropertiesInput",
  "pub struct ConsumerPropertySaveReceipt",
  "pub trait ConsumerPropertyEditorPort: Send + Sync",
  "pub struct ConsumerPropertyEditorRuntime",
  "verify_contribution(",
  "registered_schema != self.schema",
  "validate_values(&snapshot.values)",
  "validate_values(&values)",
  "PAGE_BUILDER_CONSUMER_PROPERTY_CONTRACT_INVALID",
  "PAGE_BUILDER_CONSUMER_PROPERTY_EDITOR_UNAVAILABLE",
  "PAGE_BUILDER_CONSUMER_PROPERTY_SAVE_FAILED",
]) {
  requireMarker(providerContract, marker, "Page Builder consumer property contract");
}

for (const marker of [
  "pub fn ConsumerPropertiesPanel",
  "runtime.verify_contribution(&assembly)",
  "LocalResource::new",
  "runtime.load().await",
  "prepare_save_input(&current_snapshot, current_values)",
  "runtime.save(input).await",
  'data-fly-consumer-properties="ready"',
  "data-fly-consumer-property-editor",
]) {
  requireMarker(providerPanel, marker, "Page Builder consumer property panel");
}
requireOrderedMarkers(
  providerPanel,
  [
    "runtime.verify_contribution(&assembly)",
    "LocalResource::new",
    "runtime.load().await",
  ],
  "consumer property descriptor validation before load",
);
requireMarker(
  providerModuleExport,
  "pub use consumer_properties::ConsumerPropertiesPanel;",
  "Page Builder editor module export",
);
requireMarker(
  providerPanelExport,
  "pub use editor::ConsumerPropertiesPanel;",
  "Page Builder admin crate export",
);

for (const marker of [
  "facade.consumer_properties()",
  "use_context::<Arc<ConsumerPropertyEditorRuntime>>()",
  "<ConsumerPropertiesPanel",
  "contribution_assembly=consumer_property_assembly",
]) {
  requireMarker(providerCanvas, marker, "draft consumer property composition");
}

for (const marker of [
  "pub struct NormalizedModuleContributionManifest",
  "pub fn normalize_module_contribution_manifest(",
  "OWNER_PROVIDER_METADATA_KEY",
  "PROVIDER_VERSION_METADATA_KEY",
  "outside fba.builder_consumer.capabilities",
  "must not hand-author",
]) {
  requireMarker(moduleManifestTooling, marker, "shared module contribution metadata tooling");
}

for (const marker of [
  'include!(concat!(env!("OUT_DIR"), "/pages_contribution_manifest.rs"));',
  "GENERATED_PAGES_CONTRIBUTION_MANIFEST_JSON",
  "pub fn pages_metadata_property_schema()",
  "pub fn pages_metadata_contribution()",
  "generated_admin_contribution(PAGES_METADATA_CONTRIBUTION_ID)",
  "serde_json::from_value::<ConsumerPropertyEditorSchema>",
  "schema.validate()",
  "registered_schema.validate()",
]) {
  requireMarker(pagesContributions, marker, "Pages generated metadata contribution runtime");
}
for (const forbidden of [
  /\bPropertyEditorDescriptor\s*\{\s*id\s*:/,
  /\bConsumerPropertyFieldDescriptor\s*\{\s*id\s*:/,
  /\bContributionDescriptor\s*\{\s*id\s*:/,
  /\bModuleContributionManifest\s*\{\s*module\s*:/,
]) {
  if (forbidden.test(pagesContributions)) {
    fail(
      `Pages generated metadata contribution runtime still contains ${forbidden}`,
    );
  }
}

for (const marker of [
  `id = "${contract.pages_consumer.contribution_id}"`,
  `id = "${contract.pages_consumer.property_editor_id}"`,
  `owner_provider = "${contract.pages_consumer.provider}"`,
  `provider = "${contract.pages_consumer.provider}"`,
  `component_type = "${contract.pages_consumer.component_type}"`,
  `format = "${contract.format}"`,
  'role = "metadata"',
  'persistence = "consumer_facade"',
]) {
  requireMarker(pagesModuleManifest, marker, "Pages canonical metadata contribution source");
}
for (const field of contract.pages_consumer.fields) {
  requireMarker(
    pagesModuleManifest,
    `id = "${field}"`,
    "Pages canonical metadata contribution fields",
  );
}
for (const marker of [
  "module_manifest_contribution.rs",
  "normalize_module_contribution_manifest",
  '.role("landing_blocks")',
  '.role("metadata")',
  ".manifest_json()",
  "metadata contribution must declare exactly one property editor",
  "GENERATED_PAGES_CONTRIBUTION_MANIFEST_JSON",
  "PAGES_METADATA_CONTRIBUTION_ID",
  "PAGES_METADATA_PROPERTY_EDITOR_ID",
  "PAGES_METADATA_COMPONENT_TYPE",
]) {
  requireMarker(pagesContributionBuild, marker, "Pages shared contribution build adapter");
}
for (const forbidden of [
  "struct ModuleManifestRoot",
  "struct ContributionManifestSource",
  "fn normalize_targets(",
  "fn normalize_contributions(",
]) {
  forbidMarker(
    pagesContributionBuild,
    forbidden,
    "Pages shared contribution build adapter",
  );
}

for (const marker of [
  "pub fn pages_metadata_property_runtime(",
  "trait PagesMetadataTransport: Send + Sync",
  "transport: Arc<dyn PagesMetadataTransport>",
  "transport: Arc::new(ServerPagesMetadataTransport)",
  "impl ConsumerPropertyEditorPort for PagesMetadataPropertyPort",
  "fn load(&self) -> ConsumerPropertyLoadFuture",
  "fn save(&self, input: SaveConsumerPropertiesInput)",
  "let command = metadata_save_command(&schema, &snapshot, &input)?;",
  "fetch_expected_page(transport.as_ref(), &snapshot).await?",
  "require_current_metadata_version(command.expected_version, current.version)?;",
  "let request = PageMetadataPatch {",
  "transport.patch_metadata(request).await?",
  "schema.validate_values(&input.values)?",
  "expected_metadata_version(&snapshot.page_id, &input.expected_revision)",
  "transport::patch_page_metadata(",
  "page.version <= command.expected_version",
  "on_saved(PageMutationResult::from(&page))",
  'format!("pages:{page_id}:metadata:v{version}")',
  "PAGE_METADATA_REVISION_CONFLICT",
]) {
  requireMarker(pagesOwnerPortProduction, marker, "Pages metadata owner port");
}
requireOrderedMarkers(
  pagesOwnerPortProduction,
  [
    "let command = metadata_save_command(&schema, &snapshot, &input)?;",
    "fetch_expected_page(transport.as_ref(), &snapshot).await?",
    "require_current_metadata_version(command.expected_version, current.version)?;",
    "let request = PageMetadataPatch {",
    "transport.patch_metadata(request).await?",
  ],
  "Pages metadata conflict-before-patch ordering",
);
for (const forbidden of [
  "save_page_document",
  "PageBuilderCapabilityRequest::Publish",
  "EditorCommand",
  "PageCommand",
  "content_json",
  "project_data",
]) {
  forbidMarker(pagesOwnerPortProduction, forbidden, "Pages metadata owner port");
}

for (const marker of [
  "let metadata_runtime = pages_metadata_property_runtime(",
  "provide_context(metadata_runtime)",
  "PagesBuilderSaveSnapshot",
  "metadata_refresh.update",
  "mod standalone_metadata;",
  "use standalone_metadata::PagesPublishedMetadataSurface;",
  "<PagesPublishedMetadataSurface refresh_generation />",
]) {
  requireMarker(pagesBoundary, marker, "Pages admin metadata composition boundary");
}

for (const marker of [
  "enum PublishedMetadataSurfaceAdmission",
  "fn published_metadata_surface_admission(",
  'page.status.eq_ignore_ascii_case("published")',
  "pub(crate) fn PagesPublishedMetadataSurface(",
  "use_context::<Arc<ConsumerPropertyEditorRuntime>>()",
  "build_pages_admin_contribution_registry(",
  "&pages_admin_contribution_policy()",
  "transport::fetch_page(token, tenant, page_id).await",
  "Ok(page) => match published_metadata_surface_admission(page.as_ref())",
  'data-pages-published-metadata-surface="registered"',
  'data-pages-published-metadata-admission="published-only"',
  'data-pages-fly-canvas-mounted="false"',
  'data-pages-document-authoring="false"',
  'data-pages-metadata-runtime="registered"',
  'data-pages-metadata-persistence="owner-port"',
  "<ConsumerPropertiesPanel",
  "runtime=runtime.clone()",
  "contribution_assembly=contribution_assembly.clone()",
  "The immutable Fly document remains unmounted.",
]) {
  requireMarker(
    pagesPublishedSurface,
    marker,
    "Pages published registered metadata surface",
  );
}
for (const forbidden of [
  "PagesBuilderFacade",
  "PageBuilderAdminHostContext",
  "patch_page_metadata(",
  "save_page_document",
  "provide_context(",
]) {
  forbidMarker(
    pagesPublishedSurface,
    forbidden,
    "Pages published registered metadata surface",
  );
}

for (const marker of [
  "<PagesFlyBuilder",
  "<PublishedDocumentLocked",
]) {
  requireMarker(pagesWorkspace, marker, "Pages workspace lifecycle composition");
}
for (const forbidden of [
  "fn PageMetadataEditor(",
  "<PageMetadataEditor",
  "let page_for_metadata",
  "transport::patch_page_metadata(",
]) {
  forbidMarker(pagesWorkspace, forbidden, "Pages legacy metadata workspace");
}

for (const marker of [
  "`source-ready` means code, contracts, build source or retained harness source exists.",
  "Pages and Page Builder remain one vertical pipeline with explicit owners:",
  "Pages owns persistence, lifecycle, immutable bindings",
  "Pages admin owns the optional same-origin authoring launch control",
  "Page Builder/Fly owns the reviewed document, sanitizer, runtime materialization",
  "No build, workflow, Docker, HTTP or browser execution is claimed by source inspection.",
]) {
  requireMarker(parityPlan, marker, "Pages/Page Builder parity continuation plan");
}

console.log(
  "[verify-pages-metadata-properties] PASS metadata_surface_cutover_complete=true metadata_revision_isolation_source_ready=true published_metadata_surface_source_ready=true shared_manifest_tooling=true execution_evidence=pending",
);
