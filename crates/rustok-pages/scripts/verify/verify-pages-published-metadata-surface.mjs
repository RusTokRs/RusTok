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
const evidence = JSON.parse(
  read(
    "crates/rustok-pages/contracts/evidence/pages-published-metadata-surface-source.json",
  ),
);
const revisionEvidence = JSON.parse(
  read(
    "crates/rustok-pages/contracts/evidence/pages-metadata-revision-isolation-source.json",
  ),
);
const surface = read(contract.pages_consumer.published_surface_source);
const surfaceProduction = surface.split("#[cfg(test)]")[0];
const surfaceTests = surface.includes("#[cfg(test)]")
  ? surface.slice(surface.indexOf("#[cfg(test)]"))
  : "";
const parityPlan = read(contract.parity_plan);
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireOrdered = (content, values, label) => {
  let previous = -1;
  for (const value of values) {
    const index = content.indexOf(value, previous + 1);
    if (index < 0) {
      failures.push(`${label}: missing or out of order ${value}`);
      return;
    }
    previous = index;
  }
};

if (evidence.status !== "pages_published_metadata_surface_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("executed evidence must remain empty");
}
for (const key of [
  "tests_run",
  "cargo_run",
  "format_run",
  "verifiers_run",
  "browser_run",
  "workflow_checks_run",
  "ci_run",
  "runtime_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`validation.${key} must remain false`);
  }
}

for (const [key, expected] of Object.entries({
  selected_page_read_preserved: true,
  published_status_case_insensitive: true,
  published_page_admits_registered_surface: true,
  draft_page_admits_registered_surface: false,
  archived_page_admits_registered_surface: false,
  missing_page_admits_registered_surface: false,
  registered_consumer_properties_panel_used: true,
  existing_metadata_runtime_reused: true,
  pages_contribution_assembly_reused: true,
  shared_refresh_generation_preserved: true,
  fly_canvas_mounted: false,
  document_authoring_mounted: false,
  page_builder_facade_mounted: false,
  direct_metadata_patch_in_surface: false,
  metadata_persistence_delegated_to_owner_port: true,
  metadata_revision_isolation_evidence_linked: true,
  stable_dom_contract_added: true,
  unit_regressions_added: true,
  execution_behavior_changed: false,
  public_transport_changed: false,
  ffa_promoted: false,
  fba_promoted: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`source_contract.${key} must be ${expected}`);
  }
}

const published = contract.pages_consumer.published_surface;
if (
  published?.component !== "PagesPublishedMetadataSurface" ||
  published?.selection !== "selected_published_page_only" ||
  published?.admission !== "exact_status_published_case_insensitive" ||
  published?.draft_hidden !== true ||
  published?.archived_hidden !== true ||
  published?.missing_selection_hidden !== true ||
  published?.fly_canvas_mounted !== false ||
  published?.document_authoring_mounted !== false ||
  published?.runtime !== "existing_pages_metadata_property_runtime" ||
  published?.contribution_assembly !== "pages_admin_contribution_policy" ||
  published?.persistence !== "delegated_to_pages_metadata_owner_port"
) {
  failures.push("published metadata surface machine contract is invalid");
}

for (const [key, expected] of Object.entries({
  surface: "data-pages-published-metadata-surface=registered",
  admission: "data-pages-published-metadata-admission=published-only",
  fly_canvas: "data-pages-fly-canvas-mounted=false",
  document_authoring: "data-pages-document-authoring=false",
  runtime: "data-pages-metadata-runtime=registered",
  persistence: "data-pages-metadata-persistence=owner-port",
})) {
  if (published?.dom_contract?.[key] !== expected) {
    failures.push(`published_surface.dom_contract.${key} mismatch`);
  }
  if (evidence.dom_contract?.[key] !== expected) {
    failures.push(`evidence.dom_contract.${key} mismatch`);
  }
}

const registration =
  contract.pages_consumer.source_evidence?.published_metadata_surface;
if (
  registration?.state !== "source_ready_execution_pending" ||
  registration?.contract !==
    "crates/rustok-pages/contracts/evidence/pages-published-metadata-surface-source.json" ||
  registration?.verifier !==
    "crates/rustok-pages/scripts/verify/verify-pages-published-metadata-surface.mjs"
) {
  failures.push("published metadata surface evidence registration is invalid");
}

for (const [value, label] of [
  ["enum PublishedMetadataSurfaceAdmission", "closed admission state"],
  ["Hidden,", "hidden admission state"],
  ["Registered,", "registered admission state"],
  ["fn published_metadata_surface_admission(", "admission function"],
  [
    'Some(page) if page.status.eq_ignore_ascii_case("published")',
    "case-insensitive published gate",
  ],
  [
    "Ok(page) => match published_metadata_surface_admission(page.as_ref())",
    "loaded page admission routing",
  ],
  [
    "PublishedMetadataSurfaceAdmission::Registered => view!",
    "registered render branch",
  ],
  [
    "PublishedMetadataSurfaceAdmission::Hidden => ().into_any()",
    "hidden render branch",
  ],
  ["use_context::<Arc<ConsumerPropertyEditorRuntime>>()", "existing runtime"],
  ["build_pages_admin_contribution_registry(", "Pages contribution assembly"],
  ["&pages_admin_contribution_policy()", "Pages contribution policy"],
  ["transport::fetch_page(token, tenant, page_id).await", "selected page read"],
  ["let _generation = refresh_generation.get();", "shared refresh generation"],
  ['data-pages-published-metadata-surface="registered"', "surface DOM marker"],
  [
    'data-pages-published-metadata-admission="published-only"',
    "admission DOM marker",
  ],
  ['data-pages-fly-canvas-mounted="false"', "Fly canvas DOM marker"],
  ['data-pages-document-authoring="false"', "authoring DOM marker"],
  ['data-pages-metadata-runtime="registered"', "runtime DOM marker"],
  ['data-pages-metadata-persistence="owner-port"', "persistence DOM marker"],
  ["<ConsumerPropertiesPanel", "canonical registered panel"],
  ["runtime=runtime.clone()", "runtime binding"],
  ["contribution_assembly=contribution_assembly.clone()", "assembly binding"],
  ["The immutable Fly document remains unmounted.", "immutable document message"],
]) {
  requireText(surfaceProduction, value, label);
}

requireOrdered(
  surfaceProduction,
  [
    "transport::fetch_page(token, tenant, page_id).await",
    "Ok(page) => match published_metadata_surface_admission(page.as_ref())",
    "PublishedMetadataSurfaceAdmission::Registered => view!",
    "<ConsumerPropertiesPanel",
  ],
  "published surface read-admit-render ordering",
);

for (const value of [
  "PagesBuilderFacade",
  "PageBuilderAdminHostContext",
  "PagesFlyBuilder",
  "PageBuilderAdmin",
  "patch_page_metadata(",
  "save_page_document",
  "provide_context(",
  "project_data",
  "content_json",
]) {
  forbidText(surfaceProduction, value, "published surface authoring boundary");
}

for (const [value, label] of [
  [
    "fn published_page_admits_registered_metadata_surface()",
    "published admission regression",
  ],
  [
    "fn non_published_or_missing_page_hides_registered_metadata_surface()",
    "hidden admission regression",
  ],
  [
    'published_metadata_surface_admission(Some(&page("published")))',
    "lowercase published case",
  ],
  [
    'published_metadata_surface_admission(Some(&page("PUBLISHED")))',
    "uppercase published case",
  ],
  ['for status in ["draft", "archived", ""]', "non-published cases"],
  ["published_metadata_surface_admission(None)", "missing page case"],
  ["PublishedMetadataSurfaceAdmission::Registered", "registered assertion"],
  ["PublishedMetadataSurfaceAdmission::Hidden", "hidden assertion"],
]) {
  requireText(surfaceTests, value, label);
}

if (
  revisionEvidence.status !== "pages_metadata_revision_isolation_source_unvalidated" ||
  revisionEvidence.source_contract?.metadata_patch_request_contains_document_payload !== false ||
  revisionEvidence.source_contract?.metadata_patch_request_contains_project_data !== false ||
  revisionEvidence.source_contract?.dirty_fly_state_mutated_by_metadata_port !== false ||
  revisionEvidence.source_contract?.metadata_revision_advances_independently !== true
) {
  failures.push("linked metadata revision/isolation evidence is invalid");
}

for (const marker of [
  "`source-ready` means code, contracts, build source or retained harness source exists.",
  "Pages and Page Builder remain one vertical pipeline with explicit owners:",
  "Pages owns persistence, lifecycle, immutable bindings",
  "Pages admin owns the optional same-origin authoring launch control",
  "Page Builder/Fly owns the reviewed document, sanitizer, runtime materialization",
  "No build, workflow, Docker, HTTP or browser execution is claimed by source inspection.",
]) {
  requireText(parityPlan, marker, "parity continuation plan");
}

if (failures.length > 0) {
  console.error("[verify-pages-published-metadata-surface] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "[verify-pages-published-metadata-surface] PASS source_ready=true browser_execution=pending fly_canvas_mounted=false",
);
