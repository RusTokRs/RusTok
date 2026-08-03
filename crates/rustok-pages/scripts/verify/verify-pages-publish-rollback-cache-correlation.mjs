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
    "crates/rustok-pages/contracts/evidence/pages-publish-rollback-cache-correlation-source.json",
  ),
);
const regression = read(
  "crates/rustok-pages/tests/publish_rollback_cache_correlation.rs",
);
const reviewedPublish = read(
  "crates/rustok-pages/src/services/page/reviewed_publish.rs",
);
const rollback = read("crates/rustok-pages/src/services/page/rollback.rs");
const cacheOwner = read("crates/rustok-pages/src/cache_invalidation.rs");
const storefront = read(
  "crates/rustok-pages/storefront/src/transport/native_server_adapter.rs",
);
const artifact = read("crates/rustok-pages/src/controllers/mod.rs");
const parityPlan = read("docs/modules/pages-page-builder-parity-continuation-plan.md");
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

if (
  evidence.status !==
  "pages_publish_rollback_cache_correlation_source_unvalidated"
) {
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
  "database_run",
  "storefront_run",
  "artifact_http_run",
  "workflow_checks_run",
  "ci_run",
  "runtime_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`validation.${key} must remain false`);
  }
}

for (const [key, expected] of Object.entries({
  reviewed_publish_emits_node_published_in_owner_transaction: true,
  reviewed_publish_receipt_inserted_before_commit: true,
  rollback_emits_node_published_in_owner_transaction: true,
  rollback_receipt_inserted_before_commit: true,
  publish_and_rollback_share_cache_event_contract: true,
  node_published_rotates_route_page_and_artifact_generations: true,
  handler_request_binds_event_id: true,
  handler_request_binds_correlation_id: true,
  handler_receipt_validated_before_ack: true,
  storefront_key_binds_route_page_and_artifact_generations: true,
  artifact_key_binds_artifact_generation: true,
  old_generation_values_remain_physical_but_unreachable_by_current_keys: true,
  new_generation_storefront_key_misses_before_refill: true,
  new_generation_artifact_key_misses_before_refill: true,
  storefront_refill_becomes_hit: true,
  artifact_refill_becomes_hit: true,
  reader_fail_open_behavior_changed: false,
  publish_behavior_changed: false,
  rollback_behavior_changed: false,
  public_transport_changed: false,
  database_schema_changed: false,
  ffa_promoted: false,
  fba_promoted: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`source_contract.${key} must be ${expected}`);
  }
}

const publishBody = sliceBetween(
  reviewedPublish,
  "pub async fn publish_reviewed(",
  "fn require_builder_sources(",
  "reviewed publish owner transaction",
);
requireOrder(
  publishBody,
  [
    "let txn = self.db.begin().await?;",
    "DomainEvent::NodePublished",
    "insert_publish_operation_in_tx(",
    "txn.commit().await?;",
  ],
  "reviewed publish event receipt commit ordering",
);
for (const forbidden of [
  "CacheService",
  "PagesCacheReadRuntime",
  "page_cache_namespace",
]) {
  forbidText(
    publishBody,
    forbidden,
    "reviewed publish must remain event-driven",
  );
}

const rollbackBody = sliceBetween(
  rollback,
  "pub async fn rollback_to_previous(",
  "async fn find_previous_publish_target_in_tx(",
  "rollback owner transaction",
);
requireOrder(
  rollbackBody,
  [
    "let txn = self.db.begin().await?;",
    "DomainEvent::NodePublished",
    "insert_rollback_operation_in_tx(",
    "txn.commit().await?;",
  ],
  "rollback event receipt commit ordering",
);
for (const forbidden of [
  "CacheService",
  "PagesCacheReadRuntime",
  "page_cache_namespace",
]) {
  forbidText(rollbackBody, forbidden, "rollback must remain event-driven");
}

for (const [value, label] of [
  ["Self::Published | Self::Unpublished | Self::Deleted => &PAGE_CACHE_SCOPES", "published scope set"],
  ["PAGE_CACHE_SCOPES", "route page artifact scopes"],
  ["envelope.id", "event identity binding"],
  ["envelope.correlation_id", "correlation identity binding"],
  ["receipt.validate_for(&request)?", "receipt validation"],
  ["storefront_pages_cache_key", "storefront generation key"],
  ['"rg-{}:pg-{}:ag-{}"', "three-generation storefront identity"],
  ['":g-{generation}:page:{page_id}:{variant_hash}"', "generation-bound artifact identity"],
]) {
  requireText(cacheOwner, value, label);
}

requireOrder(
  storefront,
  [
    "generation_snapshot(tenant_id)",
    "storefront_pages_cache_key(",
    "get_json::<StorefrontPagesData>",
    "get_by_slug_with_locale_fallback(",
    "put_json(cache_key, &data)",
  ],
  "storefront generation miss source refill order",
);
requireOrder(
  artifact,
  [
    "generation_snapshot(tenant_id)",
    "PageCacheScope::Artifact",
    "get_json::<CachedPublishedLandingArtifact>",
    "load_public_bound_artifact_with_fallback(",
    "put_json(cache_key, &artifact)",
  ],
  "artifact generation miss source refill order",
);

for (const [value, label] of [
  [
    "async fn published_event_rotates_generations_and_forces_storefront_and_artifact_miss_refill()",
    "correlation regression",
  ],
  ["struct CorrelatingCachePort", "shared invalidation and read port"],
  ["state.requests.push(request.clone())", "request recording"],
  ["state.receipts.push(receipt.clone())", "receipt recording"],
  ["state.generations.generation(*scope) + 1", "generation rotation"],
  ["DomainEvent::NodePublished", "published event"],
  ["handler.handle(&envelope).await.unwrap()", "handler execution"],
  ["assert_eq!(requests[0].event_id, envelope.id)", "event id assertion"],
  [
    "assert_eq!(requests[0].correlation_id, envelope.correlation_id)",
    "request correlation assertion",
  ],
  ["assert_eq!(receipts[0].event_id, envelope.id)", "receipt event assertion"],
  [
    "assert_eq!(receipts[0].correlation_id, envelope.correlation_id)",
    "receipt correlation assertion",
  ],
  ["assert_ne!(new_storefront_key, old_storefront_key)", "storefront key rotation"],
  ["assert_ne!(new_artifact_key, old_artifact_key)", "artifact key rotation"],
  ["get_json::<Value>(&new_storefront_key)", "storefront miss and hit"],
  ["get_json::<Value>(&new_artifact_key)", "artifact miss and hit"],
  ["put_json(new_storefront_key.clone(), &refilled_storefront)", "storefront refill"],
  ["put_json(new_artifact_key.clone(), &refilled_artifact)", "artifact refill"],
  ["get_json::<Value>(&old_storefront_key)", "old storefront value retained"],
  ["get_json::<Value>(&old_artifact_key)", "old artifact value retained"],
]) {
  requireText(regression, value, label);
}

for (const marker of [
  "Publish/rollback cache correlation source packet: ready, unvalidated",
  "event/correlation-bound receipt",
  "old generation keys remain physically present but unreachable",
  "Execution remains pending",
]) {
  requireText(parityPlan, marker, "parity continuation plan");
}

if (failures.length > 0) {
  console.error("[verify-pages-publish-rollback-cache-correlation] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "[verify-pages-publish-rollback-cache-correlation] PASS source_ready=true execution=pending",
);
