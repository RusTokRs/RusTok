#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), "utf8");
const failures = [];

const evidence = JSON.parse(
  read("crates/rustok-pages/contracts/evidence/pages-public-list-locale-fallback-source.json"),
);
const owner = read("crates/rustok-pages/src/services/page/read.rs");
const nativeAdapter = read(
  "crates/rustok-pages/storefront/src/transport/native_server_adapter.rs",
);
const graphql = read("crates/rustok-pages/src/graphql/query.rs");
const regression = read("crates/rustok-pages/tests/page_locale_fallback.rs");
const cacheGuard = read(
  "crates/rustok-pages/scripts/verify/verify-pages-native-storefront-cache.mjs",
);
const serverFnGuard = read(
  "crates/rustok-pages/scripts/verify/verify-pages-native-storefront-server-fn.mjs",
);
const channelGuard = read(
  "crates/rustok-pages/scripts/verify/verify-pages-native-storefront-channel-admission.mjs",
);
const plan = read("docs/modules/pages-page-builder-parity-continuation-plan.md");
const localPlan = read("crates/rustok-pages/docs/implementation-plan.md");
const packet = read(
  "docs/modules/pages-page-builder-public-list-locale-fallback-packet-2026-08-05.md",
);

const need = (text, marker, label) => {
  if (!text.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (text, marker, label) => {
  if (text.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const ordered = (text, markers, label) => {
  let previous = -1;
  for (const marker of markers) {
    const index = text.indexOf(marker, previous + 1);
    if (index < 0) {
      failures.push(`${label}: missing or out of order ${marker}`);
      return;
    }
    previous = index;
  }
};
const between = (text, start, end, label) => {
  const from = text.indexOf(start);
  if (from < 0) {
    failures.push(`${label}: missing start ${start}`);
    return "";
  }
  const to = text.indexOf(end, from + start.length);
  if (to < 0) {
    failures.push(`${label}: missing end ${end}`);
    return "";
  }
  return text.slice(from, to);
};

if (evidence.format !== "pages_public_list_locale_fallback_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_public_list_locale_fallback_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("source evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
for (const key of [
  "owner_explicit_fallback_method_added",
  "explicit_fallback_locale_normalized",
  "public_list_translation_uses_requested_tenant_platform_chain",
  "legacy_public_list_method_preserved",
  "authenticated_list_behavior_preserved",
  "native_selected_detail_and_list_share_tenant_fallback",
  "native_tenant_context_uses_tenant_default_locale",
  "native_tenant_slug_lookup_uses_loaded_tenant_default_locale",
  "blank_tenant_default_uses_platform_fallback",
  "graphql_public_detail_and_list_share_tenant_fallback",
  "channel_visibility_filter_preserved",
  "native_cache_variant_already_binds_fallback_locale",
  "focused_sqlite_regression_added",
  "retained_native_cache_guard_updated",
  "retained_native_server_fn_guard_updated",
  "retained_channel_admission_guard_updated",
  "production_pages_read_behavior_changed",
  "production_native_storefront_behavior_changed",
  "production_graphql_public_list_behavior_changed"
]) {
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`source_contract.${key} must be true`);
  }
}
for (const key of [
  "production_page_builder_behavior_changed",
  "database_schema_changed",
  "migration_changed",
  "dto_changed",
  "graphql_schema_changed",
  "public_route_changed",
  "cache_key_policy_changed",
  "cache_ttl_changed",
  "event_delivery_changed",
  "optional_event_infrastructure_changed",
  "ffa_promoted",
  "fba_promoted"
]) {
  if (evidence.source_contract?.[key] !== false) {
    failures.push(`source_contract.${key} must be false`);
  }
}

const legacyPublicList = between(
  owner,
  "pub async fn list_public_visible(",
  "#[instrument(skip(self))]\n    pub async fn list_public_visible_with_locale_fallback(",
  "legacy public list wrapper",
);
ordered(
  legacyPublicList,
  [
    "self.list_public_visible_with_locale_fallback(tenant_id, filter, None, channel_slug)",
    ".await"
  ],
  "legacy public list compatibility",
);

const fallbackPublicList = between(
  owner,
  "pub async fn list_public_visible_with_locale_fallback(",
  "async fn page_list_from_select(",
  "fallback-aware public list owner",
);
ordered(
  fallbackPublicList,
  [
    "let locale = filter",
    "let locale = normalize_locale(&locale)?",
    "let fallback_locale = fallback_locale.map(normalize_locale).transpose()?",
    "apply_public_page_channel_filter(select, tenant_id, channel_slug)",
    "self.page_list_from_select(",
    "fallback_locale"
  ],
  "fallback-aware public list ordering",
);

const listMapper = between(
  owner,
  "async fn page_list_from_select(",
  "pub(super) async fn find_page(",
  "page list mapping",
);
ordered(
  listMapper,
  [
    "fallback_locale: Option<String>",
    "resolve_translation_record(",
    "&locale",
    "fallback_locale.as_deref()",
    "title: resolved.translation.map",
    "slug: resolved.translation.map"
  ],
  "requested tenant platform locale resolution",
);

const nativeBody = between(
  nativeAdapter,
  "async fn storefront_pages_native(",
  '#[cfg(not(feature = "ssr"))]',
  "native storefront body",
);
const tenantFallbackUses =
  nativeBody.match(
    /normalize_tenant_fallback_locale\(tenant\.default_locale\.as_str\(\)\)/g,
  )?.length ?? 0;
if (tenantFallbackUses !== 2) {
  failures.push(
    `native tenant fallback must cover TenantContext and tenant_slug lookup; found ${tenantFallbackUses}`,
  );
}
ordered(
  nativeBody,
  [
    "let (tenant_id, fallback_locale)",
    "let requested_locale = locale",
    "storefront_cache_variant(",
    "fallback_locale.as_str()",
    ".get_by_slug_with_locale_fallback(",
    "Some(fallback_locale.as_str())",
    ".list_public_visible_with_locale_fallback(",
    "Some(fallback_locale.as_str())",
    "cache_runtime.put_json(cache_key, &data).await"
  ],
  "native detail list fallback parity",
);
for (const marker of [
  "fn normalize_tenant_fallback_locale(value: &str) -> String",
  "PLATFORM_FALLBACK_LOCALE.to_string()",
  "fn tenant_fallback_locale_uses_platform_only_when_owner_value_is_blank()",
  'assert_eq!(normalize_tenant_fallback_locale(" ru "), "ru")',
  'normalize_tenant_fallback_locale("   ")'
]) {
  need(nativeAdapter, marker, "native tenant fallback normalization");
}

const graphqlPublicList = between(
  graphql,
  "async fn list_public_visible_pages(",
  "fn resolve_graphql_locale_fallback(",
  "GraphQL public list helper",
);
ordered(
  graphqlPublicList,
  [
    "let locale = resolve_graphql_locale_fallback(filter.locale.as_deref(), default_locale)",
    ".list_public_visible_with_locale_fallback(",
    "locale: Some(locale)",
    "Some(default_locale)",
    "public_channel_slug"
  ],
  "GraphQL public list tenant fallback",
);
need(
  graphql,
  "get_by_slug_with_locale_fallback(\n                tenant_id,\n                security,\n                &locale,\n                &slug,\n                Some(tenant.default_locale.as_str()),",
  "GraphQL public detail tenant fallback",
);

for (const marker of [
  "async fn public_list_respects_explicit_tenant_fallback_locale()",
  'locale: "ru".to_string()',
  'locale: Some("FR".to_string())',
  'Some("RU")',
  'assert_eq!(items[0].title.as_deref(), Some("Только русский"))',
  'assert_eq!(items[0].slug.as_deref(), Some("tolko-russkiy"))'
]) {
  need(regression, marker, "focused SQLite regression");
}

for (const guard of [cacheGuard, serverFnGuard, channelGuard]) {
  need(
    guard,
    "list_public_visible_with_locale_fallback(",
    "retained native ordering guard",
  );
}

for (const marker of [
  "public-list-locale-fallback-source-ready",
  "Public list tenant locale fallback: source-ready",
  "native and GraphQL public detail/list reads",
  "cache variant already binds the fallback locale"
]) {
  need(plan, marker, "canonical Pages/Page Builder plan");
}
for (const marker of [
  "Native and unauthenticated GraphQL public detail and list reads use the same",
  "tenant fallback chain: requested locale",
  "Delete tombstones and historical backfill remain open"
]) {
  need(localPlan, marker, "Pages local plan");
}
for (const marker of [
  "source-ready / execution-pending",
  "requested locale `fr`",
  "tenant default locale `ru`",
  "selected detail and public list now resolve the same translation",
  "Execution evidence remains pending"
]) {
  need(packet, marker, "locale fallback packet");
}

forbid(owner, "Iggy", "Pages owner read path");
forbid(nativeAdapter, "Iggy", "native Pages storefront");
forbid(graphqlPublicList, "Iggy", "GraphQL public list");

if (failures.length > 0) {
  console.error("[verify-pages-public-list-locale-fallback] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log(
  "[verify-pages-public-list-locale-fallback] PASS source_ready=true execution=pending requested=fr tenant_fallback=ru",
);
