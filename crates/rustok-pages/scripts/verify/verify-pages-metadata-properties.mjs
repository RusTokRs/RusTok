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
const providerFacade = read(contract.provider.facade_source);
const providerCanvas = read(contract.provider.composition_source);
const pagesContributions = read(contract.pages_consumer.contribution_source);
const pagesOwnerPort = read(contract.pages_consumer.owner_port_source);
const pagesBoundary = read(contract.pages_consumer.composition_source);
const pagesComposition = read(contract.pages_consumer.legacy_form.source);

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
  contract.status !== "source_connected_legacy_form_pending" ||
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
  contract.pages_consumer.legacy_form.state !== "pending_removal" ||
  contract.pages_consumer.legacy_form.component !== "PageMetadataEditor"
) {
  fail("legacy metadata form cutover must remain explicit until the form is removed");
}

for (const marker of [
  '"page_builder_consumer_properties_v1"',
  "pub struct ConsumerPropertyEditorSchema",
  "pub struct ConsumerPropertyEditorSnapshot",
  "pub struct SaveConsumerPropertiesInput",
  "pub struct ConsumerPropertySaveReceipt",
  "pub trait ConsumerPropertyEditorPort: Send + Sync",
  "pub struct ConsumerPropertyEditorRuntime",
  "pub provider: String",
  "pub component_type: String",
  "contribution.provider != self.provider",
  "property_editor.provider != self.provider",
  "property_editor.component_type != self.component_type",
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
  "pub(crate) fn ConsumerPropertiesPanel",
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

for (const marker of [
  "fn consumer_properties(&self) -> Option<Arc<ConsumerPropertyEditorRuntime>>",
  "None",
]) {
  requireMarker(providerFacade, marker, "optional Page Builder consumer property facade");
}
for (const marker of [
  "facade.consumer_properties()",
  "use_context::<Arc<ConsumerPropertyEditorRuntime>>()",
  "<ConsumerPropertiesPanel",
  "contribution_assembly=consumer_property_assembly",
]) {
  requireMarker(providerCanvas, marker, "Page Builder consumer property composition");
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
  "PAGES_OWNER_PROVIDER",
  "PAGES_METADATA_COMPONENT_TYPE",
  "impl ConsumerPropertyEditorPort for PagesMetadataPropertyPort",
  "fn load(&self) -> ConsumerPropertyLoadFuture",
  "fn save(&self, input: SaveConsumerPropertiesInput)",
  "fetch_expected_page(&snapshot).await?",
  "schema.validate_values(&input.values)?",
  "expected_metadata_version(&snapshot.page_id, &input.expected_revision)",
  "current.version != expected_version",
  "transport::patch_page_metadata(",
  "page.version <= expected_version",
  "on_saved(PageMutationResult::from(&page))",
  'format!("pages:{page_id}:metadata:v{version}")',
  "PAGE_METADATA_REVISION_CONFLICT",
]) {
  requireMarker(pagesOwnerPort, marker, "Pages metadata owner port");
}
for (const forbidden of [
  "save_page_document",
  "PageBuilderCapabilityRequest::Publish",
  "EditorCommand",
  "PageCommand",
  "content_json",
  "project_data",
]) {
  forbidMarker(pagesOwnerPort, forbidden, "Pages metadata owner port");
}

for (const marker of [
  "let metadata_runtime = pages_metadata_property_runtime(",
  "provide_context(metadata_runtime)",
  "PagesBuilderSaveSnapshot",
  "metadata_refresh.update",
]) {
  requireMarker(pagesBoundary, marker, "Pages admin metadata composition boundary");
}

const legacyFormPresent =
  pagesComposition.includes("fn PageMetadataEditor(") ||
  pagesComposition.includes("<PageMetadataEditor");
if (!legacyFormPresent) {
  fail("machine contract still says pending removal, but the legacy metadata form is absent");
}
console.log("[verify-pages-metadata-properties] PASS legacy_form_pending=true");
