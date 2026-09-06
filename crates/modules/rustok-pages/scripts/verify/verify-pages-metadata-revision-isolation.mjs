#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..", "..", "..", "..");
const read = (relativePath) =>
  fs.readFileSync(path.join(repoRoot, relativePath), "utf8");

const sourcePath = "crates/rustok-pages/admin/src/metadata_properties.rs";
const modelPath = "crates/rustok-pages/admin/src/model.rs";
const evidencePath =
  "crates/rustok-pages/contracts/evidence/pages-metadata-revision-isolation-source.json";
const planPath = "docs/modules/pages-page-builder-parity-continuation-plan.md";
const consumerContractPath =
  "crates/rustok-page-builder/contracts/page-builder-consumer-properties.json";
const cargoPath = "crates/rustok-pages/admin/Cargo.toml";

const source = read(sourcePath);
const model = read(modelPath);
const evidence = JSON.parse(read(evidencePath));
const plan = read(planPath);
const consumerContract = JSON.parse(read(consumerContractPath));
const cargo = read(cargoPath);
const failures = [];

function requireText(content, value, label) {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
}

function forbidText(content, value, label) {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
}

function between(content, start, end, label) {
  const startIndex = content.indexOf(start);
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return "";
  }
  return content.slice(startIndex, endIndex);
}

function requireOrdered(content, markers, label) {
  let previous = -1;
  for (const marker of markers) {
    const index = content.indexOf(marker, previous + 1);
    if (index < 0) {
      failures.push(`${label}: missing or out of order at ${marker}`);
      return;
    }
    previous = index;
  }
}

const patchRequestStart = model.indexOf("pub struct PageMetadataPatch {");
const patchRequest = patchRequestStart < 0 ? "" : model.slice(patchRequestStart);
const serverTransport = between(
  source,
  "impl PagesMetadataTransport for ServerPagesMetadataTransport",
  "pub fn pages_metadata_property_runtime(",
  "production metadata transport",
);
const saveBlock = between(
  source,
  "    fn save(&self, input: SaveConsumerPropertiesInput) -> ConsumerPropertySaveFuture {",
  "#[derive(Debug, Clone, PartialEq, Eq)]\nstruct MetadataSaveCommand",
  "metadata owner-port save",
);
const commandBlock = between(
  source,
  "fn metadata_save_command(",
  "fn require_current_metadata_version(",
  "metadata save command",
);
const versionBlock = between(
  source,
  "fn require_current_metadata_version(",
  "async fn fetch_expected_page(",
  "metadata version recheck",
);
const tests = source.slice(source.indexOf("#[cfg(test)]"));

for (const marker of [
  "trait PagesMetadataTransport: Send + Sync",
  "fn fetch_page(&self, snapshot: PagesBuilderSaveSnapshot)",
  "fn patch_metadata(&self, request: PageMetadataPatch)",
  "transport: Arc<dyn PagesMetadataTransport>",
  "transport: Arc::new(ServerPagesMetadataTransport)",
]) {
  requireText(source, marker, "metadata transport seam");
}

for (const marker of [
  "transport::fetch_page(",
  "snapshot.token",
  "snapshot.tenant_slug",
  "snapshot.page_id",
  "ConsumerPropertyEditorError::unavailable(error.to_string())",
  "transport::patch_page_metadata(",
  "ConsumerPropertyEditorError::save(error.to_string())",
]) {
  requireText(serverTransport, marker, "production metadata transport preservation");
}

for (const marker of [
  "token: Option<String>",
  "tenant_slug: Option<String>",
  "page_id: String",
  "expected_version: i32",
  "locale: String",
  "title: String",
  "slug: String",
  "meta_title: Option<String>",
  "meta_description: Option<String>",
  "template: Option<String>",
  "channel_slugs: Vec<String>",
]) {
  requireText(patchRequest, marker, "metadata patch request");
}

for (const forbidden of [
  "body",
  "content:",
  "content_json",
  "project_data",
  "controller",
  "revision_id",
  "PageBuilder",
  "EditorCommand",
  "PageCommand",
]) {
  forbidText(patchRequest, forbidden, "metadata patch request document isolation");
}

requireOrdered(
  saveBlock,
  [
    "let command = metadata_save_command(&schema, &snapshot, &input)?;",
    "let current = fetch_expected_page(transport.as_ref(), &snapshot).await?;",
    "require_current_metadata_version(command.expected_version, current.version)?;",
    "let request = PageMetadataPatch {",
    "let page = transport.patch_metadata(request).await?;",
    "if page.id != snapshot.page_id",
    "if page.version <= command.expected_version",
    "on_saved(PageMutationResult::from(&page));",
  ],
  "metadata save conflict-before-patch ordering",
);

for (const forbidden of [
  "save_page_document",
  "project_data",
  "content_json",
  "AdminCanvasController",
  "PagesBuilderFacade",
  "PageBuilderAdminFacade",
  "PageBuilderCapabilityRequest",
]) {
  forbidText(saveBlock, forbidden, "metadata owner-port Fly isolation");
}

for (const marker of [
  "input.contribution_id != PAGES_METADATA_CONTRIBUTION_ID",
  "input.property_editor_id != PAGES_METADATA_PROPERTY_EDITOR_ID",
  "schema.validate_values(&input.values)?;",
  "expected_metadata_version(&snapshot.page_id, &input.expected_revision)?",
  "required_value(&input.values, \"title\")?",
  "required_value(&input.values, \"slug\")?",
  "optional_value(&input.values, \"meta_title\")?",
  "optional_value(&input.values, \"meta_description\")?",
  "optional_value(&input.values, \"template\")?",
  "core::parse_channel_slugs(value(&input.values, \"channel_slugs\")?)",
]) {
  requireText(commandBlock, marker, "metadata command preparation");
}

for (const marker of [
  "if current_version == expected_version",
  "Err(metadata_revision_conflict(",
  "expected_version,",
  "current_version,",
]) {
  requireText(versionBlock, marker, "metadata exact version recheck");
}

for (const marker of [
  "const PAGE_METADATA_REVISION_CONFLICT: &str = \"REVISION_CONFLICT\";",
  "ConsumerPropertyEditorError::with_stable_code(",
  "Pages metadata version changed from {expected} to {actual}; reload and retry",
  "PAGE_METADATA_REVISION_CONFLICT",
]) {
  requireText(source, marker, "stable metadata conflict");
}

for (const marker of [
  "#[tokio::test]",
  "async fn stale_metadata_revision_short_circuits_before_patch_transport()",
  "expect_err(\"stale metadata revision must fail\")",
  "assert_eq!(error.stable_code, PAGE_METADATA_REVISION_CONFLICT);",
  "assert_eq!(patch_calls.load(Ordering::SeqCst), 0);",
  "async fn metadata_save_is_document_free_and_preserves_dirty_fly_state()",
  "\"dirty\": true",
  "\"projectData\"",
  "let dirty_before = dirty_fly_state.lock()",
  "assert_eq!(patch_calls.load(Ordering::SeqCst), 1);",
  "let request = last_patch",
  "assert_eq!(request.expected_version, 7);",
  "assert_eq!(request.title, \"Updated Home\");",
  "assert_eq!(\n            *dirty_fly_state.lock().expect(\"dirty Fly lock\"),\n            dirty_before\n        );",
  "assert_eq!(receipt.revision, \"pages:page-1:metadata:v8\");",
]) {
  requireText(tests, marker, "metadata regression tests");
}

requireText(cargo, "[dev-dependencies]", "Pages admin test runtime");
requireText(cargo, "tokio.workspace = true", "Pages admin Tokio test runtime");

if (
  evidence.status !==
  "pages_metadata_revision_isolation_source_unvalidated"
) {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}

const expectedContract = {
  registered_metadata_runtime_preserved: true,
  metadata_transport_injected_for_regression_only: true,
  production_fetch_transport_preserved: true,
  production_patch_transport_preserved: true,
  exact_metadata_revision_parsed_before_current_read: true,
  current_page_version_rechecked_before_patch: true,
  stale_revision_rejected_before_patch_transport: true,
  metadata_patch_request_contains_document_payload: false,
  metadata_patch_request_contains_project_data: false,
  metadata_patch_request_contains_controller: false,
  metadata_save_callback_contains_project_data: false,
  dirty_fly_state_mutated_by_metadata_port: false,
  metadata_revision_advances_independently: true,
  unit_regressions_added: true,
  execution_behavior_changed: false,
  public_transport_changed: false,
  ffa_promoted: false,
  fba_promoted: false,
};

for (const [key, expected] of Object.entries(expectedContract)) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`evidence source_contract.${key} must be ${expected}`);
  }
}

if (evidence.source_contract?.stable_conflict_code !== "REVISION_CONFLICT") {
  failures.push("evidence stable conflict code must remain REVISION_CONFLICT");
}

const expectedTests = [
  "stale_metadata_revision_short_circuits_before_patch_transport",
  "metadata_save_is_document_free_and_preserves_dirty_fly_state",
];
if (JSON.stringify(evidence.unit_regressions) !== JSON.stringify(expectedTests)) {
  failures.push("evidence unit regression list is invalid");
}

if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("source-only evidence must not contain executed packets");
}

for (const [key, expected] of Object.entries({
  tests_run: false,
  cargo_run: false,
  format_run: false,
  verifiers_run: false,
  workflow_checks_run: false,
  ci_run: false,
  runtime_proven: false,
})) {
  if (evidence.validation?.[key] !== expected) {
    failures.push(`evidence validation.${key} must be ${expected}`);
  }
}

if (
  consumerContract.status !== "metadata_surface_cutover_complete" ||
  consumerContract.executed_evidence !== "pending"
) {
  failures.push("consumer property cutover status or execution boundary changed");
}

const evidenceRegistration =
  consumerContract.pages_consumer?.source_evidence?.metadata_revision_isolation;
if (
  evidenceRegistration?.state !== "source_ready_execution_pending" ||
  evidenceRegistration?.contract !== evidencePath ||
  evidenceRegistration?.verifier !==
    "crates/rustok-pages/scripts/verify/verify-pages-metadata-revision-isolation.mjs"
) {
  failures.push("consumer property metadata revision/isolation evidence registration is invalid");
}

for (const marker of [
  "`source-ready` means code, contracts, build source or retained harness source exists.",
  "Pages and Page Builder remain one vertical pipeline with explicit owners:",
  "Pages owns persistence, lifecycle, immutable bindings",
  "Pages admin owns the optional same-origin authoring launch control",
  "Page Builder/Fly owns the reviewed document, sanitizer, runtime materialization",
  "No build, workflow, Docker, HTTP or browser execution is claimed by source inspection.",
]) {
  requireText(plan, marker, "Pages/Page Builder parity plan");
}

if (failures.length > 0) {
  console.error("[verify-pages-metadata-revision-isolation] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "[verify-pages-metadata-revision-isolation] PASS source_ready=true execution_evidence=pending",
);
