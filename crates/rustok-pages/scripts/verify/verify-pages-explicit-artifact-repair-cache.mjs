#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..", "..", "..", "..");
const read = (relativePath) =>
  fs.readFileSync(path.join(repoRoot, relativePath), "utf8");

const evidence = JSON.parse(
  read(
    "crates/rustok-pages/contracts/evidence/pages-explicit-artifact-repair-cache-source.json",
  ),
);
const harness = read(
  "crates/rustok-pages/tests/explicit_artifact_repair_cache_postgres.rs",
);
const reviewedPublish = read(
  "crates/rustok-pages/src/services/page/reviewed_publish.rs",
);
const rebuildOwner = read(
  "crates/rustok-pages/src/services/page/artifact_rebuild.rs",
);
const activationOwner = read(
  "crates/rustok-pages/src/services/page/artifact_binding_replacement.rs",
);
const artifactOwner = read(
  "crates/rustok-pages/src/services/page_builder_artifact.rs",
);
const cacheOwner = read("crates/rustok-pages/src/cache_invalidation.rs");
const continuation = read(
  "docs/modules/pages-page-builder-repair-cache-continuation-2026-08-07.md",
);
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireOrder = (content, values, label) => {
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

if (evidence.format !== "pages_explicit_artifact_repair_cache_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_explicit_artifact_repair_cache_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const key of [
  "source_guard",
  "postgres_harness",
  "exact_rebuilt_model_reproduction",
  "rebuild_no_event_boundary",
  "activation_after_commit_boundary",
  "durable_lifecycle_pair",
  "node_updated_generation_rotation",
  "node_published_generation_rotation",
  "event_receipt_identity",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`validation.${key} must remain false`);
  }
}

for (const [key, expected] of Object.entries({
  postgres_environment_gated: true,
  isolated_postgres_schema_per_run: true,
  real_outbox_module_migrations_used: true,
  real_pages_module_migrations_used: true,
  real_page_service_publish_rebuild_activation_used: true,
  reviewed_publish_revision_matches_owner_updated_at_snapshot: true,
  canonical_artifact_snapshot_retained_before_damage: true,
  canonical_artifact_payload_damaged_before_rebuild: true,
  rebuild_uses_retained_provenance_not_damaged_artifact_payload: true,
  rebuilt_model_matches_pre_damage_canonical_model_except_storage_identity_and_created_at: true,
  rebuild_emits_no_lifecycle_events: true,
  rebuild_preserves_active_binding: true,
  rebuild_preserves_page_version: true,
  cache_generations_unchanged_by_rebuild_command: true,
  activation_commits_before_cache_handler_delivery: true,
  activation_cache_generations_unchanged_immediately_after_owner_commit: true,
  activation_durable_node_updated_loaded_after_commit: true,
  activation_durable_node_published_loaded_after_commit: true,
  durable_lifecycle_registered_schema_validated: true,
  node_updated_rotates_route_and_page_only: true,
  node_published_rotates_route_page_and_artifact: true,
  activation_total_generation_delta_route_2_page_2_artifact_1: true,
  cache_requests_bind_event_and_correlation_ids: true,
  cache_receipts_bind_event_and_correlation_ids: true,
  production_code_changed: false,
  database_schema_changed: false,
  public_transport_changed: false,
  automatic_audit_to_rebuild: false,
  automatic_rebuild_to_activation: false,
  ffa_promoted: false,
  fba_promoted: false,
  tests_run: false,
  source_verifier_run: false,
  cargo_run: false,
  formatting_run: false,
  postgres_run: false,
  cache_handler_run: false,
  cache_generation_observation_run: false,
  workflows_or_ci_run: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`source_contract.${key} must be ${expected}`);
  }
}

if (
  evidence.harness?.path !==
    "crates/rustok-pages/tests/explicit_artifact_repair_cache_postgres.rs" ||
  evidence.harness?.test !==
    "rebuilt_bytes_and_activation_cache_rotate_only_after_committed_events_on_postgres" ||
  evidence.harness?.database_env !== "RUSTOK_PAGES_TEST_DATABASE_URL" ||
  evidence.harness?.fallback_database_env !== "DATABASE_URL"
) {
  failures.push("repair cache harness registration is invalid");
}

for (const marker of [
  'const DATABASE_ENV: &str = "RUSTOK_PAGES_TEST_DATABASE_URL"',
  "CREATE SCHEMA",
  "DROP SCHEMA IF EXISTS",
  "OutboxModule",
  "PagesModule",
  "OutboxTransport::new(db.clone())",
  "PageService::new(db.clone(), event_bus)",
  "struct RecordingCachePort",
  "impl PageCacheInvalidationPort for RecordingCachePort",
  "PageCacheInvalidationEventHandler::new",
  "PagesCacheInvalidationRuntime::new",
]) {
  requireText(harness, marker, "repair cache harness foundation");
}
requireOrder(
  harness,
  [
    "let revision = draft",
    ".body",
    ".as_ref()",
    ".updated_at",
    ".clone();",
    ".publish_reviewed(",
    "revision,",
  ],
  "repair cache reviewed publish revision fixture",
);
for (const forbidden of [
  "Sha256::digest",
  'format!("{}\\0{}", body.format, body.content)',
  "use sha2::{Digest, Sha256};",
]) {
  forbidText(harness, forbidden, "repair cache reviewed publish revision fixture");
}
requireOrder(
  reviewedPublish,
  [
    "fn body_revision_snapshot(bodies: &[page_body::Model]) -> BodyRevisionSnapshot",
    ".map(|body| (body.locale.clone(), body.updated_at.to_string()))",
    "revisions.sort();",
  ],
  "reviewed publish owner revision snapshot",
);

requireOrder(
  harness,
  [
    "let canonical_snapshot = original_artifact.clone();",
    "let events_before_rebuild = outbox_event_ids(&db).await?;",
    'damaged.document_html = Set("<main>damaged canonical document</main>".to_string());',
    ".rebuild_immutable_artifact(",
    "assert_eq!(outbox_event_ids(&db).await?, events_before_rebuild);",
    "assert_eq!(cache_port.generations(), initial_generations);",
    "let rebuilt_record =",
    "let mut expected_rebuilt = canonical_snapshot.clone();",
    "expected_rebuilt.id = rebuilt_record.id;",
    "expected_rebuilt.instance_key = rebuilt_record.instance_key.clone();",
    "expected_rebuilt.created_at = rebuilt_record.created_at;",
    "assert_eq!(rebuilt_record, expected_rebuilt);",
  ],
  "exact rebuild model reproduction ordering",
);

requireOrder(
  harness,
  [
    "let events_before_activation = outbox_event_ids(&db).await?;",
    ".replace_rebuilt_artifact_binding(",
    "assert!(!replaced.replayed);",
    "assert_eq!(cache_port.generations(), initial_generations);",
    "activation_envelopes(",
    "cache_handler.handle(&updated_envelope).await?;",
    "initial_generations.route + 1",
    "initial_generations.page + 1",
    "initial_generations.artifact,",
    "cache_handler.handle(&published_envelope).await?;",
    "initial_generations.route + 2",
    "initial_generations.page + 2",
    "initial_generations.artifact + 1",
    "assert_cache_receipts(",
  ],
  "activation commit then cache delivery ordering",
);

for (const marker of [
  "envelope.validate_registered_schema()?;",
  "PageCacheInvalidationCause::Updated",
  "PageCacheInvalidationCause::Published",
  "requests[0].event_id, updated.id",
  "requests[0].correlation_id, updated.correlation_id",
  "receipts[0].artifact_generation, None",
  "requests[1].event_id, published.id",
  "requests[1].correlation_id, published.correlation_id",
  "receipts[1].artifact_generation,\n        Some(final_generations.artifact)",
]) {
  requireText(harness, marker, "committed activation cache evidence");
}

requireText(rebuildOwner, "append_rebuilt_in_tx(", "rebuild owner");
requireText(rebuildOwner, "txn.commit().await?;", "rebuild owner");
forbidText(rebuildOwner, "PageCacheInvalidationEventHandler", "rebuild owner inline cache boundary");
forbidText(rebuildOwner, "PagesCacheInvalidationRuntime", "rebuild owner inline cache boundary");

requireOrder(
  activationOwner,
  [
    "PageBuilderArtifactService::bind_existing_body_in_tx(",
    "DomainEvent::NodeUpdated",
    "DomainEvent::NodePublished",
    "page_artifact_binding_replacement_operation::ActiveModel",
    "txn.commit().await?;",
  ],
  "activation owner event/receipt/commit ordering",
);
forbidText(activationOwner, "PageCacheInvalidationEventHandler", "activation owner inline cache boundary");
forbidText(activationOwner, "PagesCacheInvalidationRuntime", "activation owner inline cache boundary");

for (const marker of [
  "document_html: Set(compiled.page.document_html.clone())",
  "body_html: Set(compiled.page.body_html.clone())",
  "css: Set(compiled.page.css.clone())",
  "content_hash: Set(compiled.page.content_hash.clone())",
  "instance_key: Set(instance_key.to_string())",
]) {
  requireText(artifactOwner, marker, "artifact owner deterministic model");
}

for (const marker of [
  "Self::Updated => &PAGE_CACHE_MUTABLE_SCOPES",
  "Self::Published | Self::Unpublished | Self::Deleted => &PAGE_CACHE_SCOPES",
  "DomainEvent::NodeUpdated",
  "DomainEvent::NodePublished",
]) {
  requireText(cacheOwner, marker, "cache owner scope mapping");
}

for (const marker of [
  "explicit-artifact-repair-cache-harness-source-ready",
  "pages_explicit_artifact_repair_cache_source_unvalidated",
  "route    +2",
  "page     +2",
  "artifact +1",
  "The source guard and PostgreSQL harness are intentionally not run",
]) {
  requireText(continuation, marker, "repair cache continuation");
}

for (const forbidden of [
  "audit_page_artifacts(",
  "rebuildPageArtifact",
  "activateRebuiltPageArtifact",
]) {
  forbidText(harness, forbidden, "repair cache automatic/transport boundary");
}

if (failures.length > 0) {
  console.error("[verify-pages-explicit-artifact-repair-cache] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "[verify-pages-explicit-artifact-repair-cache] PASS source_ready=true execution=pending",
);
