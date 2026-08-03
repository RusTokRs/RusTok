#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const paths = {
  contract: "crates/rustok-forum/contracts/forum-topic-canonical-resolution.json",
  cumulativeContract: "crates/rustok-forum/contracts/forum-topic-merge-owner.json",
  docs: "crates/rustok-forum/docs/forum-21i-topic-canonical-resolution.md",
  cumulativeDocs: "crates/rustok-forum/docs/forum-21b-topic-merge-owner.md",
  error: "crates/rustok-forum/src/error.rs",
  migration:
    "crates/rustok-forum/src/migrations/m20260803_000017_add_forum_topic_canonical_resolution.rs",
  migrationsMod: "crates/rustok-forum/src/migrations/mod.rs",
  service: "crates/rustok-forum/src/services/topic_canonical_resolution.rs",
  facade: "crates/rustok-forum/src/services/topic_facade.rs",
  servicesMod: "crates/rustok-forum/src/services/mod.rs",
  lib: "crates/rustok-forum/src/lib.rs",
  controller: "crates/rustok-forum/src/controllers/mod.rs",
  redirect: "crates/rustok-forum/src/controllers/topic_redirect.rs",
  restTopics: "crates/rustok-forum/src/controllers/topics.rs",
  openapi: "crates/rustok-forum/src/openapi.rs",
  graphql: "crates/rustok-forum/src/graphql/query_runtime.rs",
  seo: "crates/rustok-forum/src/seo_targets.rs",
  test: "crates/rustok-forum/tests/topic_canonical_resolution_sqlite.rs",
  readme: "crates/rustok-forum/README.md",
  docsIndex: "crates/rustok-forum/docs/README.md",
  routing: "docs/architecture/routing.md",
  plan: "crates/rustok-forum/docs/implementation-plan.md",
  verifier: "scripts/verify/verify-forum-topic-canonical-resolution.mjs",
};

const read = (path) => readFileSync(path, "utf8");
function includesAll(text, markers, label) {
  for (const marker of markers) {
    assert.ok(text.includes(marker), `${label} is missing marker: ${marker}`);
  }
}

const contract = JSON.parse(read(paths.contract));
const cumulativeContract = JSON.parse(read(paths.cumulativeContract));
const docs = read(paths.docs);
const cumulativeDocs = read(paths.cumulativeDocs);
const error = read(paths.error);
const migration = read(paths.migration);
const migrationsMod = read(paths.migrationsMod);
const service = read(paths.service);
const facade = read(paths.facade);
const servicesMod = read(paths.servicesMod);
const lib = read(paths.lib);
const controller = read(paths.controller);
const redirect = read(paths.redirect);
const restTopics = read(paths.restTopics);
const openapi = read(paths.openapi);
const graphql = read(paths.graphql);
const seo = read(paths.seo);
const test = read(paths.test);
const readme = read(paths.readme);
const docsIndex = read(paths.docsIndex);
const routing = read(paths.routing);
const plan = read(paths.plan);
const verifier = read(paths.verifier);

assert.equal(contract.contract, "forum_topic_canonical_resolution_v1");
assert.equal(contract.task, "FORUM-21I");
assert.equal(contract.latest_transport_slice, "FORUM-21J");
assert.equal(contract.parent_task, "FORUM-21");
assert.equal(contract.extends, "FORUM-21B");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan_status, "planned");
assert.equal(contract.owner_service, "TopicService");
assert.equal(contract.source_of_truth, "forum_topic_merge_operations");
assert.equal(contract.resolution.maximum_hops, 32);
assert.equal(
  contract.resolution.each_hop_topic_and_edges_share_one_statement_snapshot,
  true,
);
assert.equal(contract.database_guards.one_source_edge_per_tenant_topic, true);
assert.equal(contract.database_guards.parallel_alias_table_added, false);
assert.equal(contract.selected_read_cutover.rest_get_direct_target.includes("200"), true);
assert.equal(contract.selected_read_cutover.rest_get_merged_source.includes("308"), true);
assert.equal(contract.selected_read_cutover.list_reads_changed, false);
assert.equal(contract.selected_read_cutover.mutation_target_resolution_changed, false);
assert.equal(contract.http_redirect.task, "FORUM-21J");
assert.equal(contract.http_redirect.status, 308);
assert.equal(contract.http_redirect.cache_control, "private, no-store");
assert.equal(contract.http_redirect.middleware_is_scoped_to_get_only, true);
assert.equal(contract.error.stable_code, "FORUM_TOPIC_CANONICAL_RESOLUTION_CONFLICT");
assert.equal(contract.error.http_status, 500);
assert.equal(contract.error.retryable, false);
assert.equal(contract.compatibility.topic_response_shape_changed, false);
assert.equal(contract.compatibility.rest_route_shape_changed, false);
assert.equal(contract.compatibility.rest_direct_target_status_changed, false);
assert.equal(contract.compatibility.rest_merged_source_status_changed, true);
assert.equal(cumulativeContract.latest_policy_slice, "FORUM-21J");
assert.equal(cumulativeContract.canonical_resolution.parallel_alias_store, false);
assert.equal(cumulativeContract.canonical_resolution.rest_merged_source_returns_308, true);
assert.equal(cumulativeContract.bounds.canonical_resolution_hops_max, 32);

includesAll(
  migrationsMod,
  [
    "mod m20260803_000017_add_forum_topic_canonical_resolution;",
    "Box::new(m20260803_000017_add_forum_topic_canonical_resolution::Migration)",
  ],
  "migration registry",
);
includesAll(
  migration,
  [
    "uq_forum_topic_merge_operations_source",
    "ON forum_topic_merge_operations (tenant_id, source_topic_id)",
    "forum_validate_topic_merge_redirect_edge",
    "forum_05_topic_merge_redirect_edge",
    "source.status::text = 'archived'",
    "source.is_locked = TRUE",
    "source.reply_count = 0",
    "target.status::text <> 'archived'",
    "source.status = 'archived'",
    "target.status <> 'archived'",
    "DROP INDEX IF EXISTS uq_forum_topic_merge_operations_source",
  ],
  "canonical resolution migration",
);

includesAll(
  error,
  [
    "TopicCanonicalResolutionConflict(Uuid)",
    '"FORUM_TOPIC_CANONICAL_RESOLUTION_CONFLICT"',
  ],
  "ForumError",
);
assert.ok(!error.includes("Self::TopicCanonicalResolutionConflict(_) => true"));

includesAll(
  service,
  [
    "pub const MAX_FORUM_TOPIC_CANONICAL_REDIRECT_HOPS: usize = 32;",
    "pub struct ForumTopicCanonicalResolution",
    "pub requested_topic_id: Uuid",
    "pub canonical_topic_id: Uuid",
    "pub merge_operation_ids: Vec<Uuid>",
    "struct ForumTopicCanonicalStep",
    "pub(crate) async fn resolve_unchecked(",
    "load_resolution_step(",
    "match step.edges.as_slice()",
    "!visited.insert(edge.target_topic_id)",
    "merge_operation_ids.len() >= MAX_FORUM_TOPIC_CANONICAL_REDIRECT_HOPS",
    "TopicCanonicalResolutionConflict(requested_topic_id)",
    "EXISTS (",
    "AS topic_exists",
    "FROM forum_topic_merge_operations",
    "LIMIT 2",
    "LEFT JOIN (",
    "topic.deleted_at IS NULL",
  ],
  "canonical resolution owner",
);
assert.ok(!service.includes("forum_topic_alias"));
assert.ok(!service.includes("forum_topic_redirects"));
assert.ok(!service.includes("bestEffort"));

includesAll(
  facade,
  [
    "pub async fn resolve_canonical_topic(",
    "resolve_canonical_topic_for_security(",
    "ForumTopicCanonicalResolutionService::new(self.db.clone())",
    "resolution.canonical_topic_id",
    "pub async fn get_with_canonical_resolution_and_locale_fallback(",
    ".is_topic_visible(tenant_id, resolution.canonical_topic_id, &scope)",
  ],
  "TopicService facade",
);
const canonicalResolve = facade.indexOf("resolve_canonical_topic_for_security(");
const selectedHydration = facade.indexOf(
  ".inner\n            .get_with_locale_fallback(",
  canonicalResolve,
);
assert.ok(canonicalResolve >= 0 && canonicalResolve < selectedHydration);

includesAll(
  servicesMod,
  [
    "mod topic_canonical_resolution;",
    "ForumTopicCanonicalResolution, MAX_FORUM_TOPIC_CANONICAL_REDIRECT_HOPS",
  ],
  "service exports",
);
includesAll(
  lib,
  [
    "ForumTopicCanonicalResolution",
    "MAX_FORUM_TOPIC_CANONICAL_REDIRECT_HOPS",
    "mod contract_tests;",
  ],
  "crate exports",
);
includesAll(
  controller,
  [
    "ForumError::TopicCanonicalResolutionConflict(_)",
    "StatusCode::INTERNAL_SERVER_ERROR",
    "mod topic_redirect;",
    "topic_redirect::redirect_merged_topic",
  ],
  "HTTP composition",
);
includesAll(
  redirect,
  [
    "pub(crate) async fn redirect_merged_topic(",
    "StatusCode::PERMANENT_REDIRECT",
    '(CACHE_CONTROL, "private, no-store".to_string())',
    "has_any_effective_permission(&auth.permissions, &[Permission::FORUM_TOPICS_READ])",
    ".get_with_locale_fallback(",
    "url::form_urlencoded::Serializer::new",
  ],
  "HTTP redirect owner",
);
assert.ok(!redirect.includes("forum_topic_alias"));
assert.ok(!redirect.includes("forum_topic_redirects"));

includesAll(
  restTopics,
  ["pub async fn get_topic(", ".get_with_locale_fallback(", "Ok(Json(topic))"],
  "REST target hydration path",
);
assert.ok(!restTopics.includes("PERMANENT_REDIRECT"));
includesAll(
  openapi,
  ["crate::controllers::topic_redirect::redirect_merged_topic"],
  "Forum OpenAPI path",
);
assert.ok(!openapi.includes("crate::controllers::topics::get_topic,"));
includesAll(
  graphql,
  [
    "async fn forum_topic(",
    "let service = TopicService::new(db.clone(), event_bus.clone());",
    ".get_with_locale_fallback(",
    "Ok(Some(map_topic_response(topic, author_profile)))",
  ],
  "GraphQL selected-topic path",
);
includesAll(
  seo,
  [
    "impl SeoTargetProvider for ForumTopicSeoTargetProvider",
    "let service = TopicService::new(runtime.db.clone(), runtime.event_bus.clone());",
    ".get_with_locale_fallback(",
    "target_id: record.target_id",
  ],
  "Forum SEO topic path",
);

includesAll(
  test,
  [
    "merged_topic_ids_resolve_to_one_visible_canonical_target",
    "let operation_ab = Uuid::new_v4();",
    "let operation_bc = Uuid::new_v4();",
    "vec![operation_ab, operation_bc]",
    "assert_eq!(selected.id, topic_c)",
    "assert_eq!(storefront.id, topic_c)",
    "Err(ForumError::TopicNotFound(id)) if id == missing_id",
    "insert_direct_merge_receipt",
    "active_topic",
  ],
  "SQLite canonical regression",
);
includesAll(
  redirect,
  [
    "merged_source_redirects_privately_while_target_uses_existing_handler",
    "assert_eq!(target_response.status(), StatusCode::OK)",
    "assert_eq!(missing_response.status(), StatusCode::NOT_FOUND)",
    "assert_eq!(forbidden_response.status(), StatusCode::FORBIDDEN)",
    "assert_eq!(put_response.status(), StatusCode::NO_CONTENT)",
  ],
  "Axum redirect regression",
);

includesAll(
  docs,
  [
    "# FORUM-21I/J canonical merged-topic resolution and HTTP redirect",
    "`source_ready_maintainer_execution_pending`",
    "One source of truth",
    "at most 32 edges",
    "FORUM_TOPIC_CANONICAL_RESOLUTION_CONFLICT",
    "308 Permanent Redirect",
    "Cache-Control: private, no-store",
    "FORUM-21A through FORUM-21J",
    "No command above was run by the implementation agent",
  ],
  "FORUM-21I/J handoff",
);
includesAll(
  cumulativeDocs,
  [
    "FORUM-21J",
    "the only source-to-target canonical edge",
    "FORUM_TOPIC_CANONICAL_RESOLUTION_CONFLICT",
    "FORUM-21A through FORUM-21J",
  ],
  "cumulative merge handoff",
);
includesAll(
  readme,
  [
    "immutable `forum_topic_merge_operations` chain",
    "authorization-safe `308 Permanent Redirect`",
    "Canonical merged-topic resolution and HTTP redirect",
  ],
  "crate README",
);
includesAll(
  docsIndex,
  [
    "authorization-safe permanent redirect",
    "FORUM-21I/J canonical resolution and HTTP redirect",
  ],
  "forum docs index",
);
includesAll(
  routing,
  ["## Owner-authorized canonical redirects", "Cache-Control: private, no-store"],
  "routing contract",
);

assert.ok(plan.includes("| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |"));
assert.ok(plan.includes("| `FORUM-24` | `planned` | Localized routes, canonical URLs and aliases. |"));
assert.ok(!plan.includes("| `FORUM-21` | `done` |"));
assert.ok(!plan.includes("| `FORUM-24` | `done` |"));
assert.ok(verifier.includes("source_ready_maintainer_execution_pending"));

console.log(
  "FORUM-21I/J canonical resolution and HTTP redirect source is ready; FORUM-21 and FORUM-24 remain planned.",
);
