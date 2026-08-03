#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = (path) => readFileSync(path, "utf8");
const includesAll = (text, markers, label) => {
  for (const marker of markers) {
    assert.ok(text.includes(marker), `${label} is missing marker: ${marker}`);
  }
};

const contract = JSON.parse(
  read("crates/rustok-forum/contracts/forum-topic-canonical-resolution.json"),
);
const cumulative = JSON.parse(
  read("crates/rustok-forum/contracts/forum-topic-merge-owner.json"),
);
const redirect = read("crates/rustok-forum/src/controllers/topic_redirect.rs");
const controller = read("crates/rustok-forum/src/controllers/mod.rs");
const topics = read("crates/rustok-forum/src/controllers/topics.rs");
const publicBoundary = read("crates/rustok-forum/tests/public_boundary_contract.rs");
const openapi = read("crates/rustok-forum/src/openapi.rs");
const cargo = read("crates/rustok-forum/Cargo.toml");
const docs = read("crates/rustok-forum/docs/forum-21i-topic-canonical-resolution.md");
const cumulativeDocs = read("crates/rustok-forum/docs/forum-21b-topic-merge-owner.md");
const readme = read("crates/rustok-forum/README.md");
const docsIndex = read("crates/rustok-forum/docs/README.md");
const routing = read("docs/architecture/routing.md");
const plan = read("crates/rustok-forum/docs/implementation-plan.md");

assert.equal(contract.contract, "forum_topic_canonical_resolution_v1");
assert.equal(contract.latest_transport_slice, "FORUM-21J");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan_status, "planned");
assert.equal(contract.http_redirect.task, "FORUM-21J");
assert.equal(contract.http_redirect.method, "GET");
assert.equal(contract.http_redirect.status, 308);
assert.equal(contract.http_redirect.cache_control, "private, no-store");
assert.equal(
  contract.http_redirect.authorization_is_checked_before_canonical_identity_disclosure,
  true,
);
assert.equal(contract.http_redirect.target_read_is_hydrated_before_redirect, true);
assert.equal(contract.http_redirect.location_uses_hydrated_target_identity, true);
assert.equal(contract.http_redirect.runtime_topic_service_composition_is_reused, true);
assert.equal(contract.http_redirect.missing_and_forbidden_responses_have_no_location, true);
assert.equal(contract.http_redirect.middleware_is_scoped_to_get_only, true);
assert.equal(contract.http_redirect.mutation_commands_follow_canonical_target, false);
assert.equal(contract.http_redirect.put_delete_and_other_mutations_are_unchanged, true);
assert.equal(contract.compatibility.rest_route_shape_changed, false);
assert.equal(contract.compatibility.rest_direct_target_status_changed, false);
assert.equal(contract.compatibility.rest_merged_source_status_changed, true);
assert.equal(cumulative.latest_policy_slice, "FORUM-21J");
assert.equal(cumulative.canonical_resolution.rest_direct_target_returns_200, true);
assert.equal(cumulative.canonical_resolution.rest_merged_source_returns_308, true);
assert.equal(cumulative.canonical_resolution.rest_location_uses_hydrated_target_identity, true);
assert.equal(cumulative.canonical_resolution.rest_runtime_topic_service_composition_is_reused, true);
assert.equal(cumulative.canonical_resolution.rest_redirect_is_get_only, true);

includesAll(
  redirect,
  [
    "#[utoipa::path(",
    'path = "/api/forum/topics/{id}"',
    "status = 308",
    '"Location" = String',
    '"Cache-Control" = String',
    "pub(crate) async fn redirect_merged_topic(",
    "has_any_effective_permission(&auth.permissions, &[Permission::FORUM_TOPICS_READ])",
    "let service = runtime.topic_service();",
    ".resolve_canonical_topic(tenant.id, security.clone(), topic_id)",
    "if !resolution.redirected",
    "let canonical_topic = service",
    ".get_with_locale_fallback(",
    "canonical_topic_location(canonical_topic.id, filter.locale.as_deref())",
    "StatusCode::PERMANENT_REDIRECT",
    "(LOCATION, location)",
    '(CACHE_CONTROL, "private, no-store".to_string())',
    "url::form_urlencoded::Serializer::new",
    'query.append_pair("locale", locale)',
  ],
  "redirect middleware",
);
assert.ok(!redirect.includes("forum_topic_alias"));
assert.ok(!redirect.includes("forum_topic_redirects"));
assert.ok(!redirect.includes("TopicService::new(runtime.db_clone()"));

const permissionCheck = redirect.indexOf("has_any_effective_permission(");
const canonicalResolve = redirect.indexOf(".resolve_canonical_topic(");
const targetHydration = redirect.indexOf("let canonical_topic = service", canonicalResolve);
const hydratedLocation = redirect.indexOf("canonical_topic_location(canonical_topic.id", targetHydration);
const responseRedirect = redirect.indexOf("StatusCode::PERMANENT_REDIRECT", hydratedLocation);
assert.ok(permissionCheck >= 0 && permissionCheck < canonicalResolve);
assert.ok(canonicalResolve < targetHydration && targetHydration < hydratedLocation);
assert.ok(hydratedLocation < responseRedirect);

includesAll(
  redirect,
  [
    "canonical_location_encodes_explicit_locale",
    "merged_source_redirects_privately_while_target_uses_existing_handler",
    "ForumTopicMergeService::new",
    "crate::controllers::topics::get_topic",
    "Some(expected_location.as_str())",
    'Some("private, no-store")',
    "assert_eq!(target_response.status(), StatusCode::OK)",
    "let target: TopicResponse = serde_json::from_slice(&target_body)?;",
    "assert_eq!(missing_response.status(), StatusCode::NOT_FOUND)",
    "assert_eq!(forbidden_response.status(), StatusCode::FORBIDDEN)",
    "Method::PUT",
    "assert_eq!(put_response.status(), StatusCode::NO_CONTENT)",
  ],
  "redirect tests",
);

includesAll(
  controller,
  [
    "pub(crate) mod topic_redirect;",
    "get(topics::get_topic)",
    ".route_layer(axum::middleware::from_fn_with_state(",
    "topic_redirect::redirect_merged_topic",
    ".put(content_commands::update_topic)",
    ".delete(topics::delete_topic)",
  ],
  "Forum route wiring",
);
const getRoute = controller.indexOf("get(topics::get_topic)");
const getLayer = controller.indexOf("topic_redirect::redirect_merged_topic", getRoute);
const putRoute = controller.indexOf(".put(content_commands::update_topic)", getLayer);
const deleteRoute = controller.indexOf(".delete(topics::delete_topic)", putRoute);
assert.ok(getRoute >= 0 && getRoute < getLayer && getLayer < putRoute && putRoute < deleteRoute);

includesAll(
  topics,
  [
    "pub async fn get_topic(",
    "ensure_forum_permission(",
    '"forum_permission_denied"',
    ".get_with_locale_fallback(",
    "Ok(Json(topic))",
  ],
  "existing topic GET handler",
);
assert.ok(!topics.includes("StatusCode::PERMANENT_REDIRECT"));
includesAll(
  publicBoundary,
  ['include_str!("../src/controllers/topic_redirect.rs")'],
  "public boundary controller guard",
);
includesAll(openapi, ["crate::controllers::topic_redirect::redirect_merged_topic"], "OpenAPI");
assert.ok(!openapi.includes("crate::controllers::topics::get_topic,"));
includesAll(cargo, ["[dev-dependencies]", "tower.workspace = true"], "Cargo test support");

includesAll(
  docs,
  [
    "# FORUM-21I/J canonical merged-topic resolution and HTTP redirect",
    "308 Permanent Redirect",
    "Location: /api/forum/topics/{canonical_topic_id}",
    "Cache-Control: private, no-store",
    "forum_permission_denied",
    "FORUM-21A through FORUM-21J",
    "FORUM-24",
    "No command above was run by the implementation agent",
  ],
  "canonical handoff",
);
includesAll(
  cumulativeDocs,
  ["FORUM-21J", "308 Permanent Redirect", "private, no-store", "FORUM-21A through FORUM-21J"],
  "merge handoff",
);
includesAll(
  readme,
  ["authorization-safe `308 Permanent Redirect`", "Slug aliases and localized public routes are not part of the current contract"],
  "Forum README",
);
includesAll(
  docsIndex,
  ["authorization-safe permanent redirect", "FORUM-21I/J canonical resolution and HTTP redirect"],
  "Forum docs index",
);
includesAll(
  routing,
  ["## Owner-authorized canonical redirects", "Cache-Control: private, no-store", "owner module's OpenAPI surface"],
  "routing contract",
);

assert.ok(plan.includes("| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |"));
assert.ok(plan.includes("| `FORUM-24` | `planned` | Localized routes, canonical URLs and aliases. |"));
assert.ok(!plan.includes("| `FORUM-21` | `done` |"));
assert.ok(!plan.includes("| `FORUM-24` | `done` |"));

console.log(
  "FORUM-21J merged-topic HTTP redirect source is ready; FORUM-21 and FORUM-24 remain planned.",
);
