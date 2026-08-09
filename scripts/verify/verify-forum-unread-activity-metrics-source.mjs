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
const packetPath =
  "docs/modules/forum-33-unread-activity-metrics-actualization-2026-08-09.md";

const graphql = read(graphqlPath);
const packet = read(packetPath);

for (const marker of [
  "async fn forum_unread_topics(",
  ".list_topics_with_unread(",
  ".await?;\n        observe_unread_topic_locale_resolution(&page.items);\n        observe_unread_topic_activity(&page.items);",
  "rustok_forum_graphql_unread_topic_state_total",
  '&["state"]',
  'const UNREAD_TOPIC_STATE_IMPLICIT: &str = "implicit";',
  'const UNREAD_TOPIC_STATE_REPLY: &str = "reply";',
  'const UNREAD_TOPIC_STATE_REVISION: &str = "revision";',
  'const UNREAD_TOPIC_STATE_REPLY_AND_REVISION: &str = "reply_and_revision";',
  'const UNREAD_TOPIC_STATE_READ: &str = "read";',
  "if !read_state_explicit",
  "unread_count > 0 && has_unread_topic_revision",
  "else if unread_count > 0",
  "else if has_unread_topic_revision",
  "rustok_telemetry::register_runtime_collector",
  "counter.with_label_values(&[state]).inc();",
]) {
  requireText(graphql, marker, `${graphqlPath}: missing ${marker}`);
}

const observerStart = graphql.indexOf("fn unread_topic_activity_state(");
const mapperStart = graphql.indexOf("fn map_topic(");
if (observerStart < 0 || mapperStart <= observerStart) {
  throw new Error(`${graphqlPath}: unread activity observer source boundary is invalid`);
}
const observer = graphql.slice(observerStart, mapperStart);

for (const forbidden of [
  "tenant_id",
  "user_id",
  "topic_id",
  "category_id",
  "requested_locale",
  "effective_locale",
  "last_read_position",
  "last_read_revision",
  "DatabaseConnection",
  "ForumReadModelService::new",
  "Entity::find",
  "UPDATE ",
  "INSERT ",
  "DELETE ",
  "ActiveModel",
]) {
  requireAbsent(
    observer,
    forbidden,
    `${graphqlPath}: unread activity observer must not contain ${forbidden}`,
  );
}

for (const forbiddenLabel of [
  "with_label_values(&[item.",
  "with_label_values(&[unread_count",
  "unread_count.to_string",
  "read_state_explicit.to_string",
  "has_unread_topic_revision.to_string",
]) {
  requireAbsent(
    observer,
    forbiddenLabel,
    `${graphqlPath}: unread activity metric labels must remain fixed`,
  );
}

for (const marker of [
  "FORUM-33I",
  "FORUM-33H",
  "Attachments remain blocked on FORUM-14",
  "rustok_forum_graphql_unread_topic_state_total",
  "observation counter",
  "unreadOnly = true",
  "not described as",
  "Spam-outcome telemetry remains premature",
  "DuplicateContent",
  "ExternalSpamScore",
  "No additional database query",
  "no Cargo command",
]) {
  requireText(packet, marker, `${packetPath}: missing ${marker}`);
}

console.log("Forum FORUM-33I unread activity metric source: ok");
