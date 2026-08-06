#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];

function read(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!existsSync(absolutePath)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(absolutePath, "utf8");
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function rejectMarker(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

const paths = {
  cargo: "crates/rustok-forum/storefront/Cargo.toml",
  test: "crates/rustok-forum/storefront/tests/native_host_route_decision_sqlite.rs",
  categoryAdapter:
    "crates/rustok-forum/storefront/src/transport/native_server_adapter_category_route.rs",
  topicAdapter:
    "crates/rustok-forum/storefront/src/transport/native_server_adapter_topic_route.rs",
  contract: "crates/rustok-forum/contracts/forum-native-host-route-evidence.json",
  docs: "crates/rustok-forum/docs/forum-24s-native-host-route-evidence.md",
  readme: "crates/rustok-forum/docs/README.md",
};

const source = Object.fromEntries(
  Object.entries(paths).map(([key, relativePath]) => [key, read(relativePath)]),
);

for (const marker of [
  "axum.workspace = true",
  "rustok-taxonomy.workspace = true",
  "sea-orm.workspace = true",
  "sea-orm-migration.workspace = true",
  "tokio.workspace = true",
  "tower.workspace = true",
]) {
  requireMarker(source.cargo, marker, paths.cargo);
}

for (const marker of [
  '#![cfg(feature = "ssr")]',
  "handle_server_fns_with_context",
  "provide_context(host.clone())",
  "HostRuntimeContext::new",
  "TenantContextExtension(tenant.clone())",
  'const CATEGORY_ENDPOINT: &str = "/api/fn/forum/storefront-category-route"',
  'const TOPIC_ENDPOINT: &str = "/api/fn/forum/storefront-topic-route"',
  "TaxonomyModule.migrations()",
  "ForumModule.migrations()",
  "CategoryService::new",
  "CreateCategoryInput",
  "UpdateCategoryInput",
  "TopicService::new",
  "RenameForumTopicSlugInput",
  "registered_native_host_resolves_forum_canonical_alias_and_missing_routes",
  '"canonical"',
  '"redirect"',
  '"null"',
  '"/en/forum/c/platform-engineering"',
  '"/en/forum/t/{short_id}/registered-native-host"',
]) {
  requireMarker(source.test, marker, paths.test);
}
for (const marker of [
  "ForumCategoryRouteService::new",
  "ForumTopicRouteService::new",
  "forum_category_route_alias",
  "forum_topic_route_alias",
  "INSERT INTO forum_category_route",
  "INSERT INTO forum_topic_route",
]) {
  rejectMarker(source.test, marker, paths.test);
}

for (const marker of [
  '#[server(prefix = "/api/fn", endpoint = "forum/storefront-category-route")]',
  "expect_context::<HostRuntimeContext>()",
  "extract::<TenantContext>()",
  "extract::<OptionalAuthContext>()",
  "extract::<RequestContext>()",
  "ForumCategoryRouteService::new(db.clone())",
  "get_public_storefront_visible_with_locale_fallback",
]) {
  requireMarker(source.categoryAdapter, marker, paths.categoryAdapter);
}

for (const marker of [
  '#[server(prefix = "/api/fn", endpoint = "forum/storefront-topic-route")]',
  "expect_context::<HostRuntimeContext>()",
  "extract::<TenantContext>()",
  "extract::<OptionalAuthContext>()",
  "extract::<RequestContext>()",
  "ForumTopicRouteService::new(db.clone())",
  "TransactionalEventBus",
  "get_public_storefront_visible_with_locale_fallback",
]) {
  requireMarker(source.topicAdapter, marker, paths.topicAdapter);
}

let contract = null;
try {
  contract = JSON.parse(source.contract);
} catch (error) {
  failures.push(`${paths.contract}: invalid JSON: ${error.message}`);
}
if (contract) {
  if (
    contract.schema_version !== 1 ||
    contract.module !== "forum" ||
    contract.surface !== "native_host_canonical_route_decision" ||
    contract.task !== "FORUM-24S"
  ) {
    failures.push(`${paths.contract}: identity drift`);
  }
  if (
    contract.status !== "executable_no_run" ||
    contract.compile_policy !== "not_run_by_request"
  ) {
    failures.push(`${paths.contract}: execution status drift`);
  }
  if (contract.test_target !== paths.test) {
    failures.push(`${paths.contract}: test target drift`);
  }
  if (
    contract.registered_host?.category_endpoint !==
      "/api/fn/forum/storefront-category-route" ||
    contract.registered_host?.topic_endpoint !==
      "/api/fn/forum/storefront-topic-route"
  ) {
    failures.push(`${paths.contract}: registered endpoint drift`);
  }
  if (contract.fixture_owner_writes?.direct_route_alias_inserts !== false) {
    failures.push(`${paths.contract}: direct alias insertion must remain false`);
  }
  const cases = new Set((contract.cases ?? []).map((item) => item.name));
  for (const required of [
    "registered_category_canonical",
    "registered_category_redirect",
    "registered_topic_canonical",
    "registered_topic_redirect",
    "registered_missing_fail_closed",
    "trusted_tenant_context",
  ]) {
    if (!cases.has(required)) failures.push(`${paths.contract}: missing case ${required}`);
  }
  for (const key of [
    "production_runtime_code_changed",
    "route_owner_changed",
    "visibility_policy_changed",
    "channel_policy_changed",
    "graphql_contract_changed",
    "storefront_dto_changed",
    "storage_schema_changed",
    "event_schema_changed",
    "migration_added",
    "browser_evidence_claimed",
  ]) {
    if (contract.preserved_boundaries?.[key] !== false) {
      failures.push(`${paths.contract}: ${key} must remain false`);
    }
  }
  if (contract.verification?.executed_by_implementation_agent !== false) {
    failures.push(`${paths.contract}: execution must not be claimed`);
  }
}

for (const marker of [
  "FORUM-24S",
  "executable SQLite source / maintainer execution pending",
  "/api/fn/forum/storefront-category-route",
  "/api/fn/forum/storefront-topic-route",
  "TenantContextExtension",
  "No route alias row is inserted directly",
  "No tests, Node verifiers, Cargo commands",
  "browser navigation evidence",
]) {
  requireMarker(source.docs, marker, paths.docs);
}
for (const marker of [
  "FORUM-24S",
  "forum-24s-native-host-route-evidence.md",
  "verify-forum-native-host-route-evidence.mjs",
]) {
  requireMarker(source.readme, marker, paths.readme);
}

if (failures.length > 0) {
  console.error("Forum registered native-host route evidence verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum registered native-host route evidence verified");
