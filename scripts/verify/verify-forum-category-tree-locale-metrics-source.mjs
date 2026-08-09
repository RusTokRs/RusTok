#!/usr/bin/env node

import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

function requireText(text, marker, message) {
  if (!text.includes(marker)) throw new Error(message);
}

function requireAbsent(text, marker, message) {
  if (text.includes(marker)) throw new Error(message);
}

const graphqlPath = "crates/rustok-forum/src/graphql/category_tree_query.rs";
const packetPath =
  "docs/modules/forum-33-category-tree-locale-metrics-actualization-2026-08-09.md";

const graphql = read(graphqlPath);
const packet = read(packetPath);

for (const marker of [
  "async fn forum_category_tree(",
  ".tree_authenticated_owner_visible_with_audience_context(",
  ".await?;",
  "observe_category_tree_locale_resolution(&tree.roots);",
  "rustok_forum_graphql_category_tree_locale_resolution_total",
  '&["outcome"]',
  'const CATEGORY_TREE_LOCALE_OUTCOME_EXACT: &str = "exact";',
  'const CATEGORY_TREE_LOCALE_OUTCOME_FALLBACK: &str = "fallback";',
  'const CATEGORY_TREE_LOCALE_OUTCOME_MISSING: &str = "missing";',
  "available_locale_count == 0",
  "requested_locale == effective_locale",
  "rustok_telemetry::register_runtime_collector",
  "counter.with_label_values(&[outcome]).inc();",
  "observe_category_tree_locale_resolution_with_counter(counter, &node.children);",
]) {
  requireText(graphql, marker, `${graphqlPath}: missing ${marker}`);
}

const ownerCall = graphql.indexOf(
  ".tree_authenticated_owner_visible_with_audience_context(",
);
const ownerAwait = graphql.indexOf(".await?;", ownerCall);
const observation = graphql.indexOf(
  "observe_category_tree_locale_resolution(&tree.roots);",
  ownerAwait,
);
const response = graphql.indexOf("Ok(tree.into())", observation);
if (
  ownerCall < 0 ||
  ownerAwait <= ownerCall ||
  observation <= ownerAwait ||
  response <= observation
) {
  throw new Error(
    `${graphqlPath}: expected owner call -> await -> locale observation -> response ordering`,
  );
}

const observerStart = graphql.indexOf("fn category_tree_locale_resolution_outcome(");
const gqlTypeStart = graphql.indexOf("#[derive(SimpleObject)]", observerStart);
if (observerStart < 0 || gqlTypeStart <= observerStart) {
  throw new Error(`${graphqlPath}: category-tree locale observer source boundary is invalid`);
}
const observer = graphql.slice(observerStart, gqlTypeStart);

for (const forbidden of [
  "tenant_id",
  "user_id",
  "category_id",
  "parent_id",
  "name",
  "slug",
  "description",
  "icon",
  "color",
  "topic_count",
  "reply_count",
  "DatabaseConnection",
  "category_audience_read_service",
  "Entity::find",
  "UPDATE ",
  "INSERT ",
  "DELETE ",
  "ActiveModel",
]) {
  requireAbsent(
    observer,
    forbidden,
    `${graphqlPath}: category-tree locale observer must not contain ${forbidden}`,
  );
}

for (const forbiddenLabel of [
  "with_label_values(&[requested_locale",
  "with_label_values(&[effective_locale",
  "with_label_values(&[node.",
  "requested_locale.to_string",
  "effective_locale.to_string",
]) {
  requireAbsent(
    observer,
    forbiddenLabel,
    `${graphqlPath}: locale values must not become metric labels`,
  );
}

for (const marker of [
  "FORUM-33J",
  "FORUM-33I",
  "Attachments therefore remain blocked on FORUM-14",
  "forumCategoryTree",
  "rustok_forum_graphql_category_tree_locale_resolution_total",
  "MAX_FORUM_CATEGORY_TREE_NODES",
  "no additional database query",
  "Tenant-controlled locale strings",
  "Spam-outcome telemetry remains blocked",
  "Moderation recovery now has public owner commands",
  "no Cargo command",
]) {
  requireText(packet, marker, `${packetPath}: missing ${marker}`);
}

console.log("Forum FORUM-33J category-tree locale metric source: ok");
