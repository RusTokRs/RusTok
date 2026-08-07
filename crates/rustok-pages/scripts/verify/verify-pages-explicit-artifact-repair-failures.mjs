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
    "crates/rustok-pages/contracts/evidence/pages-explicit-artifact-repair-failures-source.json",
  ),
);
const harness = read(
  "crates/rustok-pages/tests/explicit_artifact_repair_failures_sqlite.rs",
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
const continuation = read(
  "docs/modules/pages-page-builder-repair-failures-continuation-2026-08-07.md",
);
const postgresContinuation = read(
  "docs/modules/pages-page-builder-repair-postgres-continuation-2026-08-07.md",
);
const actualization = read(
  "docs/modules/page-builder-parity-actualization-2026-08-05.md",
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
const sliceBetween = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  if (startIndex < 0) {
    failures.push(`${label}: missing ${start}`);
    return "";
  }
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (endIndex < 0) {
    failures.push(`${label}: missing ${end}`);
    return "";
  }
  return content.slice(startIndex, endIndex);
};
const countText = (content, value) => content.split(value).length - 1;

if (evidence.status !== "pages_explicit_artifact_repair_failures_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const key of [
  "source_guard",
  "sqlite_harness",
  "corrupt_provenance",
  "reviewed_runtime_mismatch",
  "stale_version",
  "invalid_replacement",
  "unpublished_page",
  "zero_side_effect_snapshots",
  "rebuild_no_event_boundary",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`validation.${key} must remain false`);
  }
}

for (const [key, expected] of Object.entries({
  sqlite_isolated_database_per_test: true,
  real_outbox_sys_events_migration_used: true,
  real_channel_module_migrations_used: true,
  real_pages_module_migrations_used: true,
  reviewed_publish_revision_matches_owner_updated_at_snapshot: true,
  corrupt_provenance_rejected: true,
  corrupt_provenance_error_code_bound: true,
  corrupt_provenance_adds_no_rebuild_receipt: true,
  corrupt_provenance_adds_no_artifact: true,
  corrupt_provenance_preserves_binding_version_status_events: true,
  reviewed_runtime_mismatch_rejected: true,
  reviewed_runtime_mismatch_adds_no_rebuild_receipt: true,
  reviewed_runtime_mismatch_adds_no_artifact: true,
  reviewed_runtime_mismatch_preserves_binding_version_status_events: true,
  activation_stale_version_rejected: true,
  activation_stale_version_adds_no_receipt: true,
  activation_stale_version_preserves_binding_version_status_events: true,
  activation_invalid_replacement_rejected: true,
  activation_invalid_replacement_error_code_bound: true,
  activation_invalid_replacement_adds_no_receipt: true,
  activation_invalid_replacement_preserves_binding_version_status_events: true,
  activation_unpublished_page_rejected: true,
  activation_unpublished_error_code_bound: true,
  activation_unpublished_adds_no_receipt: true,
  activation_unpublished_preserves_binding_version_status_events: true,
  activation_failure_fixture_rebuild_emits_no_events: true,
  automatic_audit_to_rebuild: false,
  automatic_rebuild_to_activation: false,
  production_code_changed: false,
  database_schema_changed: false,
  public_transport_changed: false,
  ffa_promoted: false,
  fba_promoted: false,
  tests_run: false,
  source_verifier_run: false,
  cargo_run: false,
  formatting_run: false,
  sqlite_run: false,
  workflows_or_ci_run: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`source_contract.${key} must be ${expected}`);
  }
}

if (
  evidence.harness?.path !==
  "crates/rustok-pages/tests/explicit_artifact_repair_failures_sqlite.rs"
) {
  failures.push("failure harness path is invalid");
}
for (const test of [
  "rebuild_rejects_corrupt_provenance_atomically",
  "rebuild_rejects_reviewed_runtime_mismatch_atomically",
  "activation_rejects_stale_version_atomically",
  "activation_rejects_invalid_replacement_atomically",
  "activation_rejects_unpublished_page_atomically",
]) {
  if (!evidence.harness?.tests?.includes(test)) {
    failures.push(`failure harness registration is missing ${test}`);
  }
  requireText(harness, `async fn ${test}()`, "failure harness");
}

for (const marker of [
  "SysEventsMigration.up(&manager).await?",
  "for migration in ChannelModule.migrations()",
  "for migration in PagesModule.migrations()",
  "body.updated_at",
  "struct RepairState",
  "rebuild_receipts: u64",
  "activation_receipts: u64",
  "artifact_count: u64",
  "binding_artifact_id: Uuid",
  "page_version: i32",
  "page_status: String",
  "event_count: u64",
]) {
  requireText(harness, marker, "failure harness foundation");
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
  "failure harness reviewed publish revision fixture",
);
for (const forbidden of [
  "Sha256::digest",
  'format!("{}\\0{}", body.format, body.content)',
  "use sha2::{Digest, Sha256};",
]) {
  forbidText(harness, forbidden, "failure harness reviewed publish revision fixture");
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
if (countText(harness, "assert_eq!(after, before);") !== 5) {
  failures.push("each of the five rejected commands must preserve the complete RepairState snapshot");
}

const corruptProvenance = sliceBetween(
  harness,
  "async fn rebuild_rejects_corrupt_provenance_atomically()",
  "async fn rebuild_rejects_reviewed_runtime_mismatch_atomically()",
  "corrupt provenance test",
);
requireOrder(
  corruptProvenance,
  [
    'source_active.provenance_hash = Set("0".repeat(64));',
    "let before = repair_state",
    ".rebuild_immutable_artifact(",
    "PagesError::PublishOperationIntegrity(message)",
    "PAGE_ARTIFACT_REBUILD_SOURCE_INVALID",
    "let after = repair_state",
    "assert_eq!(after, before);",
  ],
  "corrupt provenance rejection ordering",
);

const runtimeMismatch = sliceBetween(
  harness,
  "async fn rebuild_rejects_reviewed_runtime_mismatch_atomically()",
  "async fn activation_rejects_stale_version_atomically()",
  "runtime mismatch test",
);
requireOrder(
  runtimeMismatch,
  [
    "PageBuilderReviewedPublishRuntime::new(",
    "let before = repair_state",
    ".rebuild_immutable_artifact(",
    "PagesError::PublishRuntimeReviewInvalid",
    "let after = repair_state",
    "assert_eq!(after, before);",
  ],
  "runtime mismatch rejection ordering",
);

const staleVersion = sliceBetween(
  harness,
  "async fn activation_rejects_stale_version_atomically()",
  "async fn activation_rejects_invalid_replacement_atomically()",
  "stale version test",
);
requireOrder(
  staleVersion,
  [
    "let before = repair_state",
    "let stale_version = before.page_version + 1;",
    ".replace_rebuilt_artifact_binding(",
    "PagesError::VersionConflict",
    "let after = repair_state",
    "assert_eq!(after, before);",
  ],
  "stale version rejection ordering",
);

const invalidReplacement = sliceBetween(
  harness,
  "async fn activation_rejects_invalid_replacement_atomically()",
  "async fn activation_rejects_unpublished_page_atomically()",
  "invalid replacement test",
);
requireOrder(
  invalidReplacement,
  [
    'replacement_active.artifact_hash = Set("0".repeat(64));',
    "let before = repair_state",
    ".replace_rebuilt_artifact_binding(",
    "PAGE_ARTIFACT_BINDING_REPLACEMENT_TARGET_INVALID",
    "let after = repair_state",
    "assert_eq!(after, before);",
  ],
  "invalid replacement rejection ordering",
);

const unpublished = sliceBetween(
  harness,
  "async fn activation_rejects_unpublished_page_atomically()",
  "fn page_service(",
  "unpublished activation test",
);
requireOrder(
  unpublished,
  [
    ".unpublish_if_current(",
    "let before = repair_state",
    'assert_ne!(before.page_status, "published");',
    ".replace_rebuilt_artifact_binding(",
    "PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT",
    "let after = repair_state",
    "assert_eq!(after, before);",
  ],
  "unpublished activation rejection ordering",
);

const rebuildFixture = sliceBetween(
  harness,
  "async fn rebuild_fixture(",
  "fn reviewed_input(",
  "activation rebuild fixture",
);
requireOrder(
  rebuildFixture,
  [
    "let before = repair_state",
    ".rebuild_immutable_artifact(",
    "let after = repair_state",
    "after.rebuild_receipts, before.rebuild_receipts + 1",
    "after.artifact_count, before.artifact_count + 1",
    "after.binding_artifact_id, before.binding_artifact_id",
    "after.page_version, before.page_version",
    "after.event_count, before.event_count",
  ],
  "rebuild fixture no-side-effect boundary",
);

for (const marker of [
  'pub const PAGE_ARTIFACT_REBUILD_SOURCE_INVALID: &str = "PAGE_ARTIFACT_REBUILD_SOURCE_INVALID"',
  "verify_source(&source)?;",
  "if source.review_hash != reviewed.review_hash",
  "PagesError::publish_runtime_review_invalid(",
]) {
  requireText(rebuildOwner, marker, "rebuild owner");
}
requireOrder(
  rebuildOwner,
  [
    "verify_source(&source)?;",
    "if source.provenance_hash != input.expected_provenance_hash",
    "if source.review_hash != reviewed.review_hash",
    "let compiled = compile_exact_rebuild(&source, &reviewed)?;",
  ],
  "rebuild owner rejection-before-append ordering",
);

for (const marker of [
  'pub const PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT: &str =',
  'pub const PAGE_ARTIFACT_BINDING_REPLACEMENT_TARGET_INVALID: &str =',
  "enforce_expected_version(Some(input.expected_version), existing_page.version)?;",
  'if existing_page.status != "published"',
  "if replacement.instance_key != rebuild.artifact_instance_key",
  "PageBuilderArtifactService::bind_existing_body_in_tx(",
]) {
  requireText(activationOwner, marker, "activation owner");
}
requireOrder(
  activationOwner,
  [
    "enforce_expected_version(Some(input.expected_version), existing_page.version)?;",
    'if existing_page.status != "published"',
    "let replacement = load_replacement_artifact_in_tx(",
    "if replacement.instance_key != rebuild.artifact_instance_key",
    "PageBuilderArtifactService::bind_existing_body_in_tx(",
  ],
  "activation owner rejection-before-binding ordering",
);

for (const marker of [
  "explicit-artifact-repair-failure-harness-source-ready",
  "pages_explicit_artifact_repair_failures_source_unvalidated",
  "five isolated SQLite regressions",
  "complete snapshot to remain unchanged",
  "Cache handler execution",
]) {
  requireText(continuation, marker, "failure continuation");
}
for (const marker of [
  "explicit-artifact-repair-failure-harness-source-ready",
  "Rebuild provenance/runtime failure matrix",
  "Activation stale-version/invalid-target/unpublished matrix",
  "negative SQLite repair harnesses",
]) {
  requireText(postgresContinuation, marker, "PostgreSQL continuation cursor");
}
for (const marker of [
  "repair-failure-harness-source-ready",
  "Rebuild provenance/runtime negative harness",
  "Activation stale-version/invalid-target/unpublished harness",
  "negative provenance/runtime failures are now harness-ready",
]) {
  requireText(actualization, marker, "canonical parity actualization");
}

for (const forbidden of [
  "audit_page_artifacts(",
  "rebuildPageArtifact",
  "activateRebuiltPageArtifact",
]) {
  forbidText(harness, forbidden, "failure harness automatic/transport boundary");
}

if (failures.length > 0) {
  console.error("[verify-pages-explicit-artifact-repair-failures] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "[verify-pages-explicit-artifact-repair-failures] PASS source_ready=true execution=pending",
);
