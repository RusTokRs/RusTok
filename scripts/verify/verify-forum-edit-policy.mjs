#!/usr/bin/env node
// Bounded source-order regression guard, not a substitute for the DB tests.
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spaced } from "./lib/forum-source-verification.mjs";

export const SETTINGS_COMPOSITION_SITES = [
  ["src/controllers/mod.rs", "REST", 1],
  ["src/graphql/runtime_data.rs", "GraphQL", 1],
  ["storefront/src/transport/composer_native_server.rs", "native composer", 2],
  ["storefront/src/transport/native_server_adapter.rs", "native initial read", 1],
  ["storefront/src/transport/native_server_adapter_pages.rs", "native continuation", 2],
];

export const GRAPHQL_SETTINGS_DEFAULT_SITES = [
  ["src/graphql/query_runtime.rs", 4],
  ["src/graphql/storefront_audience_topics.rs", 2],
  ["src/graphql/storefront_read_state.rs", 2],
];

export function inspectGraphqlSettingsErrors(source, label, expectedCount) {
  const calls = [...source.matchAll(/\.default_(?:topics_per_page|replies_per_page|topic_order)\(tenant_id\)\s*\.await\s*\?/g)];
  return calls.length === expectedCount ? [] : [
    `${label}: expected ${expectedCount} settings defaults propagating typed errors unchanged, found ${calls.length}`,
  ];
}

export function inspectSettingsComposition(source, label, expectedCount) {
  const matches = [...source.matchAll(/let settings_providers = ForumSettingsProviders::from_optional_readers\(\s*([\s\S]*?)\n\s*\);/g)];
  const errors = [];
  if (matches.length !== expectedCount) errors.push(`${label}: expected ${expectedCount} canonical settings compositions, found ${matches.length}`);
  for (const [index, [, argumentsSource]] of matches.entries()) {
    // Only direct, unfiltered capability lookups may reach the common constructor.
    // A missing static or transactional reader must remain visible to the owner.
    const argumentsPattern = /^\w+\.shared_get::<SharedStaticModuleSettingsReader>\(\),\s*\w+\.shared_get::<SharedStaticModuleSettingsTransactionReader>\(\),?\s*$/;
    if (!argumentsPattern.test(argumentsSource.trim())) errors.push(`${label}[${index}]: both optional settings capabilities must reach the owner unchanged`);
  }
  return errors;
}

export function inspectTransactionalSettingsPolicy(source) {
  const bounded = signature => {
    const start = source.indexOf(signature);
    if (start < 0) return "";
    return spaced(source.slice(start, source.indexOf("\n    }", start)));
  };
  const staticCapability = bounded("fn static_settings_reader(");
  const ordinaryRead = bounded("pub(crate) async fn module_settings(");
  const engagementRead = bounded("pub async fn resolve(");
  const constructor = bounded("pub fn from_optional_readers(");
  const capability = bounded("fn transactional_settings_reader(");
  const read = bounded("pub(crate) async fn module_settings_in_tx(");
  const errors = [];
  if (!/Self\s*\{\s*static_reader,\s*transactional_reader,?\s*\}/.test(constructor)) errors.push("settings constructor must retain both optional capabilities");
  if (!ordinaryRead.includes("self.static_settings_reader()?")) errors.push("ordinary settings read must propagate static capability admission");
  if (!engagementRead.includes("providers.static_settings_reader()?")) errors.push("engagement mode read must propagate static capability admission");
  if (!capability.includes("self.static_settings_reader()?;")) errors.push("transactional settings must reject a missing static reader too");
  for (const marker of [
    "self.static_reader.as_ref()", "self.transactional_reader.is_some()",
    "(Some(reader), _) => Ok(Some(reader))",
    "(None, true) => Err(ForumError::capability_unavailable(",
    '\"FORUM_STATIC_SETTINGS_UNAVAILABLE\"',
    "(None, false) => Ok(None)",
  ]) {
    if (!staticCapability.includes(spaced(marker))) errors.push(`static settings must reject transaction-only configuration: ${marker}`);
  }
  for (const marker of [
    "self.transactional_reader.as_ref()", "self.static_reader.is_some()",
    "(Some(reader), _) => Ok(Some(reader))",
    "(None, true) => Err(ForumError::capability_unavailable(",
    '"FORUM_TRANSACTIONAL_SETTINGS_UNAVAILABLE"',
    "(None, false) => Ok(None)",
  ]) {
    if (!capability.includes(spaced(marker))) errors.push(`transactional settings must distinguish missing capability from defaults: ${marker}`);
  }
  if (!read.includes("self.transactional_settings_reader()?")) errors.push("transactional settings read must propagate capability admission");
  return errors;
}

export function inspectTopicEditPolicy(source) {
  const start = source.indexOf("    pub(crate) async fn update_with_inline_relations(");
  const end = source.indexOf("    pub(crate) async fn ", start + 1);
  const body = source.slice(start, end < 0 ? undefined : end);
  const markers = [
    "let txn = self.db.begin().await?;",
    "lock_category_tree_in_tx(&txn, tenant_id)",
    "Self::claim_topic_update_in_tx(",
    "Self::find_topic_in_tx(",
    "enforce_owned_scope(",
    "topic.status == TopicStatus::Archived",
    "enforce_author_edit_window(",
    ".module_settings_in_tx(",
    "Utc::now()",
  ];
  let previous = -1;
  for (const marker of markers) {
    const index = body.indexOf(marker, previous + 1);
    if (index < 0) return [`topic edit policy must follow source claim: missing/out-of-order ${marker}`];
    previous = index;
  }
  return [];
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const root = process.env.RUSTOK_VERIFY_REPO_ROOT || path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
  const source = readFileSync(path.join(root, "crates/modules/rustok-forum/src/services/topic.rs"), "utf8");
  const settings = readFileSync(path.join(root, "crates/modules/rustok-forum/src/services/engagement_mode.rs"), "utf8");
  const errors = [...inspectTopicEditPolicy(source), ...inspectTransactionalSettingsPolicy(settings)];
  for (const [file, label, count] of SETTINGS_COMPOSITION_SITES) {
    errors.push(...inspectSettingsComposition(readFileSync(path.join(root, "crates/modules/rustok-forum", file), "utf8"), label, count));
  }
  for (const [file, count] of GRAPHQL_SETTINGS_DEFAULT_SITES) {
    errors.push(...inspectGraphqlSettingsErrors(readFileSync(path.join(root, "crates/modules/rustok-forum", file), "utf8"), file, count));
  }
  if (errors.length) {
    console.error(errors.join("\n"));
    process.exitCode = 1;
  } else console.log("Forum topic edit policy follows the transactional source claim (static evidence only).");
}
