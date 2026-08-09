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

const graphqlPath = "crates/rustok-forum/src/graphql/read_state.rs";
const cargoPath = "crates/rustok-forum/Cargo.toml";
const packetPath =
  "docs/modules/forum-33-locale-fallback-metrics-actualization-2026-08-09.md";

const graphql = read(graphqlPath);
const cargo = read(cargoPath);
const packet = read(packetPath);

requireText(cargo, 'prometheus = "0.14"', `${cargoPath}: prometheus dependency is missing`);

for (const marker of [
  "async fn forum_unread_topics(",
  ".list_topics_with_unread(",
  ".await?;\n        observe_unread_topic_locale_resolution(&page.items);",
  "rustok_forum_graphql_locale_resolution_total",
  '&["resource", "outcome"]',
  'const LOCALE_RESOURCE_UNREAD_TOPIC: &str = "unread_topic";',
  'const LOCALE_OUTCOME_EXACT: &str = "exact";',
  'const LOCALE_OUTCOME_FALLBACK: &str = "fallback";',
  'const LOCALE_OUTCOME_MISSING: &str = "missing";',
  "available_locale_count == 0",
  "requested_locale == effective_locale",
  "rustok_telemetry::register_runtime_collector",
  "with_label_values(&[LOCALE_RESOURCE_UNREAD_TOPIC, outcome])",
]) {
  requireText(graphql, marker, `${graphqlPath}: missing ${marker}`);
}

const observerStart = graphql.indexOf("fn locale_resolution_outcome(");
const mapperStart = graphql.indexOf("fn map_topic(");
if (observerStart < 0 || mapperStart <= observerStart) {
  throw new Error(`${graphqlPath}: locale observer source boundary is invalid`);
}
const observer = graphql.slice(observerStart, mapperStart);

for (const forbidden of [
  "tenant_id",
  "user_id",
  "topic_id",
  "category_id",
  "title",
  "slug",
  "DatabaseConnection",
  "ForumReadModelService::new",
  "Entity::find",
  "UPDATE ",
  "INSERT ",
  "DELETE ",
  "ActiveModel",
]) {
  requireAbsent(observer, forbidden, `${graphqlPath}: locale metric observer must not contain ${forbidden}`);
}

requireAbsent(
  observer,
  "with_label_values(&[requested_locale",
  `${graphqlPath}: requested locale must never become a metric label`,
);
requireAbsent(
  observer,
  "with_label_values(&[effective_locale",
  `${graphqlPath}: effective locale must never become a metric label`,
);

for (const marker of [
  "FORUM-33H",
  "FORUM-33G",
  "Attachments remain blocked on FORUM-14",
  "forumUnreadTopics",
  "rustok_forum_graphql_locale_resolution_total",
  "resource=\"unread_topic\"",
  "exact",
  "fallback",
  "missing",
  "No locale values are metric labels",
  "no additional database query",
  "no Cargo command",
]) {
  requireText(packet, marker, `${packetPath}: missing ${marker}`);
}

console.log("Forum FORUM-33H locale fallback metric source: ok");
