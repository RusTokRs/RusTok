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
const pagesContributions = read(contract.pages_consumer.contribution_source);
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
  contract.pages_consumer.published_surface.fly_canvas_mounted !== false ||
  contract.pages_consumer.published_surface.runtime !==
    "existing_pages_metadata_property_runtime" ||
  contract.pages_consumer.published_surface.contribution_assembly !==
    "pages_admin_contribution_policy"
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
  `PAGES_METADATA_CONTRIBUTION_ID: &str = "${contract.pages_consumer.contribution_id}"`,
  `PAGES_METADATA_PROPERTY_EDITOR_ID: &str = "${contract.pages_consumer.property_editor_id}"`,
  `PAGES_OWNER_PROVIDER: &str = "${contract.pages_consumer.provider}"`,
  `PAGES_METADATA_COMPONENT_TYPE: &str = "${contract.pages_consumer.component_type}"`,
  "pub fn pages_metadata_property_schema()",
  "PAGE_BUILDER_CONSUMER_PROPERTIES_FORMAT",
  "pub fn pages_metadata_contribution()",
  "PropertyEditorDescriptor",
  "property_schema: serde_json::to_value(schema)",
  "pages_metadata_contribution(),",
  "registered_schema.validate()",
]) {
  requireMarker(pagesContributions, marker, "Pages metadata contribution");
}
for (const field of contract.pages_consumer.fields) {
  requireMarker(
    pagesContributions,
    `"${field}"`,
    "Pages metadata contribution fields",
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
  "let request = MetadataPatchRequest {",
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
    "let request = MetadataPatchRequest {",
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
  "pub(crate) fn PagesPublishedMetadataSurface(",
  "use_context::<Arc<ConsumerPropertyEditorRuntime>>()",
  "build_pages_admin_contribution_registry(",
  "&pages_admin_contribution_policy()",
  "transport::fetch_page(token, tenant, page_id).await",
  'page.status.eq_ignore_ascii_case("published")',
  'data-pages-published-metadata-surface="registered"',
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
  "Metadata UI cutover: source-complete",
  "Metadata revision/isolation source packet: ready, unvalidated",
  "Draft registered metadata surface: source-connected",
  "Published registered metadata surface: source-connected",
  "Legacy PageMetadataEditor: removed",
  "Execution evidence remains pending",
]) {
  requireMarker(parityPlan, marker, "Pages/Page Builder parity continuation plan");
}

console.log(
  "[verify-pages-metadata-properties] PASS metadata_surface_cutover_complete=true metadata_revision_isolation_source_ready=true execution_evidence=pending",
);
