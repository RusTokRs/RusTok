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

const graphqlPath = "crates/modules/rustok-forum/src/graphql/storefront_audience_topics.rs";
const packetPath =
  "docs/modules/forum-33-storefront-topic-list-locale-metrics-actualization-2026-08-09.md";

const graphql = read(graphqlPath);
const packet = read(packetPath);

for (const marker of [
  "async fn forum_storefront_audience_topics(",
  ".list_public_storefront_visible_with_locale_fallback(",
  ".await?;",
  "observe_storefront_topic_list_locale_resolution(&page.items);",
  "rustok_forum_graphql_storefront_topic_list_locale_resolution_total",
  '&["outcome"]',
  'const STOREFRONT_TOPIC_LIST_LOCALE_OUTCOME_EXACT: &str = "exact";',
  'const STOREFRONT_TOPIC_LIST_LOCALE_OUTCOME_FALLBACK: &str = "fallback";',
  'const STOREFRONT_TOPIC_LIST_LOCALE_OUTCOME_MISSING: &str = "missing";',
  "available_locale_count == 0",
  "requested_locale == effective_locale",
  "rustok_telemetry::register_runtime_collector",
  "counter.with_label_values(&[outcome]).inc();",
  'metrics::record_read_path_query(',
  'metrics::record_read_path_budget(',
]) {
  requireText(graphql, marker, `${graphqlPath}: missing ${marker}`);
}

// Both viewer branches must await the exact owner before the *single* locale
// observation and response mapping. Observing within either branch would let
// authenticated/public metrics diverge or double-count after a refactor.
const authenticatedCall = graphql.indexOf(
  ".list_authenticated_storefront_visible_with_audience_context(",
);
const publicCall = graphql.indexOf(
  ".list_public_storefront_visible_with_locale_fallback(",
);
const observation = graphql.indexOf(
  "observe_storefront_topic_list_locale_resolution(&page.items);",
);
const mapping = graphql.indexOf("let items = page", observation);
const response = graphql.indexOf("Ok(ForumTopicPage {", mapping);
if (
  authenticatedCall < 0 ||
  publicCall <= authenticatedCall ||
  !graphql.slice(authenticatedCall, publicCall).includes(".await?") ||
  !graphql.slice(publicCall, observation).includes(".await?") ||
  observation <= publicCall ||
  mapping <= observation ||
  response <= mapping
) {
  throw new Error(
    `${graphqlPath}: expected authenticated/public owners -> await -> one locale observation -> mapping -> response ordering`,
  );
}

const observerStart = graphql.indexOf(
  "fn storefront_topic_list_locale_resolution_outcome(",
);
const mapperStart = graphql.indexOf("fn map_topic_list_item(", observerStart);
if (observerStart < 0 || mapperStart <= observerStart) {
  throw new Error(`${graphqlPath}: storefront locale observer source boundary is invalid`);
}
const observer = graphql.slice(observerStart, mapperStart);

for (const forbidden of [
  "tenant_id",
  "user_id",
  "topic.id",
  "category_id",
  "author_id",
  "title",
  "slug",
  "channel_slugs",
  "tags",
  "status",
  "reply_count",
  "vote_score",
  "is_subscribed",
  "solution_reply_id",
  "metadata",
  "DatabaseConnection",
  "ForumTopicAudienceListService::new",
  "Entity::find",
  "UPDATE ",
  "INSERT ",
  "DELETE ",
  "ActiveModel",
]) {
  requireAbsent(
    observer,
    forbidden,
    `${graphqlPath}: storefront locale observer must not contain ${forbidden}`,
  );
}

for (const forbiddenLabel of [
  "with_label_values(&[requested_locale",
  "with_label_values(&[effective_locale",
  "with_label_values(&[item.",
  "requested_locale.to_string",
  "effective_locale.to_string",
]) {
  requireAbsent(
    observer,
    forbiddenLabel,
    `${graphqlPath}: locale or DTO values must not become metric labels`,
  );
}

for (const marker of [
  "FORUM-33K",
  "FORUM-33J",
  "locale-baseline-complete",
  "Attachments therefore remain blocked on FORUM-14",
  "forumStorefrontAudienceTopics",
  "rustok_forum_graphql_storefront_topic_list_locale_resolution_total",
  "observation counter",
  "no database query",
  "tenant-controlled locale string",
  "Spam-outcome telemetry therefore remains blocked",
  "public Moderation application-operation read/status contract",
  "stop locale expansion",
  "no Cargo command",
]) {
  requireText(packet, marker, `${packetPath}: missing ${marker}`);
}

console.log("Forum FORUM-33K storefront topic-list locale metric source: ok");
