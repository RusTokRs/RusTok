#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");

const evidence = JSON.parse(read(
  "crates/rustok-pages/contracts/evidence/pages-selected-immutable-artifact-source.json",
));
const harness = read(
  "crates/rustok-pages/tests/selected_immutable_published_artifact_sqlite.rs",
);
const artifactService = read(
  "crates/rustok-pages/src/services/page_builder_artifact.rs",
);
const nativeAdapter = read(
  "crates/rustok-pages/storefront/src/transport/native_server_adapter.rs",
);
const packet = read(
  "docs/modules/pages-page-builder-selected-immutable-artifact-packet-2026-08-05.md",
);
const plan = read("docs/modules/pages-page-builder-parity-continuation-plan.md");
const localPlan = read("crates/rustok-pages/docs/implementation-plan.md");
const failures = [];

const need = (text, marker, label) => {
  if (!text.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (text, marker, label) => {
  if (text.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const ordered = (text, markers, label) => {
  let at = -1;
  for (const marker of markers) {
    at = text.indexOf(marker, at + 1);
    if (at < 0) {
      failures.push(`${label}: missing or out of order ${marker}`);
      return;
    }
  }
};
const between = (text, start, end, label) => {
  const from = text.indexOf(start);
  const to = text.indexOf(end, from + start.length);
  if (from < 0 || to < 0) {
    failures.push(`${label}: unable to locate source slice`);
    return "";
  }
  return text.slice(from, to);
};

if (evidence.format !== "pages_selected_immutable_artifact_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_selected_immutable_artifact_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
for (const key of [
  "real_pages_reviewed_publish_used",
  "page_builder_review_runtime_used",
  "immutable_artifact_and_binding_persisted",
  "materialization_evidence_retained",
  "exact_locale_public_read_used",
  "fallback_locale_public_read_used",
  "persisted_current_body_mutated_after_publish",
  "draft_marker_differs_from_published_artifact",
  "published_binding_unchanged_after_draft_mutation",
  "selected_artifact_hash_unchanged_after_draft_mutation",
  "selected_document_html_unchanged_after_draft_mutation",
  "draft_marker_absent_from_exact_locale_result",
  "draft_marker_absent_from_fallback_result",
  "public_read_resolves_binding_before_artifact_record",
  "public_read_verifies_artifact_integrity",
  "current_body_content_is_not_public_render_authority"
]) {
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`source_contract.${key} must be true`);
  }
}
for (const key of [
  "production_pages_behavior_changed",
  "production_page_builder_behavior_changed",
  "production_storefront_behavior_changed",
  "production_event_delivery_changed",
  "database_schema_changed",
  "dependencies_changed",
  "public_route_changed",
  "ffa_promoted",
  "fba_promoted"
]) {
  if (evidence.source_contract?.[key] !== false) {
    failures.push(`source_contract.${key} must be false`);
  }
}
if (
  evidence.harness?.path !==
    "crates/rustok-pages/tests/selected_immutable_published_artifact_sqlite.rs" ||
  evidence.harness?.test !==
    "storefront_reads_selected_immutable_artifact_after_persisted_draft_mutation" ||
  evidence.harness?.owner_read !==
    "PageBuilderArtifactService::load_public_bound_artifact_with_fallback" ||
  evidence.harness?.exact_locale !== "en" ||
  evidence.harness?.requested_fallback_locale !== "fr -> en" ||
  evidence.harness?.channel !== "web"
) {
  failures.push("selected immutable artifact harness registration is invalid");
}

for (const marker of [
  "PageBuilderReviewedPublishRuntime::new(",
  ".publish_reviewed(",
  "page_static_landing_artifact::Entity::find()",
  "page_published_landing_artifact::Entity::find_by_id(body_id)",
  "load_public_bound_artifact_with_fallback(",
  '"fr",',
  'Some("en")',
  "persist_new_draft_body(&db, tenant_id, &fixture).await?",
  'persisted_body.content.contains("Draft-only mutation")',
  "assert_eq!(binding.artifact_id, fixture.artifact_id)",
  "assert_eq!(exact_after.artifact_hash, exact_before.artifact_hash)",
  "assert_eq!(exact_after.document_html, exact_before.document_html)",
  "assert_eq!(fallback_after.artifact_hash, fallback_before.artifact_hash)",
  "assert_eq!(fallback_after.document_html, fallback_before.document_html)",
  '!exact_after.document_html.contains("Draft-only mutation")',
  '!fallback_after.document_html.contains("Draft-only mutation")'
]) need(harness, marker, "selected artifact harness");

const test = between(
  harness,
  "async fn storefront_reads_selected_immutable_artifact_after_persisted_draft_mutation(",
  "async fn persist_new_draft_body(",
  "selected artifact test",
);
ordered(test, [
  "create_reviewed_published_page",
  "let exact_before = artifacts",
  "let fallback_before = artifacts",
  "persist_new_draft_body",
  "let persisted_body = page_body::Entity::find_by_id",
  "let binding = page_published_landing_artifact::Entity::find_by_id",
  "let exact_after = artifacts",
  "let fallback_after = artifacts",
  "assert_eq!(exact_after.artifact_hash, exact_before.artifact_hash)",
  "assert_eq!(fallback_after.document_html, fallback_before.document_html)"
], "selected artifact read/mutate/read ordering");

const mutation = between(
  harness,
  "async fn persist_new_draft_body(",
  "async fn create_reviewed_published_page(",
  "draft mutation helper",
);
ordered(mutation, [
  "page_body::Entity::find_by_id(fixture.body_id)",
  "let mut active: page_body::ActiveModel = body.into()",
  '"Draft-only mutation"',
  "active.updated_at = Set(",
  "active.update(db).await?"
], "persisted draft mutation ordering");

const publish = between(
  harness,
  "async fn create_reviewed_published_page(",
  "async fn setup_db(",
  "reviewed publish fixture",
);
ordered(publish, [
  '"Published immutable artifact"',
  "PageService::new",
  ".create(",
  "page_body::Entity::find()",
  "PageBuilderReviewedPublishRuntime::new(",
  ".publish_reviewed(",
  "page_static_landing_artifact::Entity::find()",
  "page_published_landing_artifact::Entity::find_by_id(body_id)",
  "assert_eq!(binding.artifact_id, artifact.id)"
], "reviewed publish and binding ordering");

const publicRead = between(
  artifactService,
  "pub async fn load_public_bound_artifact_with_fallback(",
  "async fn find_artifact_in_tx(",
  "public selected artifact owner",
);
ordered(publicRead, [
  'page.status == "published"',
  "page_is_visible_for_channel_in_tx",
  "build_locale_candidates",
  "load_bound_artifact_in_tx",
  "page_body::Entity::find()",
  "page_published_landing_artifact::Entity::find_by_id(body.id)",
  "page_static_landing_artifact::Entity::find_by_id(binding.artifact_id)",
  "published_record(record).map(Some)"
], "binding selected artifact ordering");
need(artifactService, "verify_record(&record)?;", "immutable artifact integrity verification");
need(artifactService, "PageBuilderMaterializedStaticLandingArtifact", "materialization envelope verification");

const nativeRead = between(
  nativeAdapter,
  "async fn storefront_pages_native(",
  '#[cfg(not(feature = "ssr"))]',
  "native storefront owner",
);
ordered(nativeRead, [
  "get_by_slug_with_locale_fallback(",
  "PageBuilderArtifactService::new(runtime_ctx.db_clone())",
  ".load_public_bound_artifact_with_fallback(",
  "published_artifact_page_body("
], "native storefront delegates immutable selection");

for (const marker of [
  "A persisted current Fly body can advance after publication",
  "selected immutable published artifact",
  "exact-locale public read (en)",
  "fallback public read (fr → en)",
  "draft marker is absent from both public results",
  "Execution evidence remains pending"
]) need(packet, marker, "selected immutable artifact packet");
for (const marker of [
  "selected-immutable-artifact-source-ready",
  "Selected immutable artifact after draft mutation: source-ready",
  "current Fly body is not public render authority"
]) need(plan, marker, "canonical Pages/Page Builder plan");
for (const marker of [
  "selected immutable published artifact regression",
  "persisted draft body mutation",
  "current body content is not public render authority"
]) need(localPlan, marker, "Pages local plan");

forbid(harness, "Iggy", "Pages selected artifact harness");
forbid(packet, "IggyTransport", "Pages selected artifact packet");
forbid(harness, "redis::", "Pages selected artifact harness");
forbid(harness, 'cmd("SCAN")', "Pages selected artifact harness");
forbid(harness, 'cmd("KEYS")', "Pages selected artifact harness");

if (failures.length) {
  console.error("[verify-pages-selected-immutable-artifact] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log(
  "[verify-pages-selected-immutable-artifact] PASS source_ready=true execution=pending public_authority=immutable_binding",
);
