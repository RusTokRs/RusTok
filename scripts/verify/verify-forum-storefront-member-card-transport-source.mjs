#!/usr/bin/env node

import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

// Collect every drift instead of aborting on the first one: a guard that throws hides the rest of
// the contract behind whichever marker happened to move first.
const failures = [];

function need(text, marker, label) {
  if (!text.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function forbid(text, marker, label) {
  if (text.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

const memberCardQuery = read("crates/modules/rustok-forum/src/graphql/member_card_query.rs");
const unreadQuery = read("crates/modules/rustok-forum/src/graphql/storefront_read_state.rs");
const owner = read("crates/modules/rustok-forum/src/services/member_card.rs");
const model = read("crates/modules/rustok-forum/storefront/src/model.rs");
const graphql = read("crates/modules/rustok-forum/storefront/src/transport/graphql_adapter.rs");
const native = read("crates/modules/rustok-forum/storefront/src/transport/native_server_adapter.rs");
const ui = read("crates/modules/rustok-forum/storefront/src/ui/leptos.rs");
const cargo = read("crates/modules/rustok-forum/storefront/Cargo.toml");
for (const marker of [
  "async fn forum_member_cards(",
  "require_member_card_permission(ctx)?",
  "Permission::FORUM_TOPICS_READ",
]) need(memberCardQuery, marker, "FORUM-15D existing authenticated member-card contract");
forbid(
  memberCardQuery,
  "forum_storefront_member_cards",
  "FORUM-15D must not widen Forum statistics to a public storefront field",
);

for (const marker of [
  "pub struct GqlForumStorefrontUnreadTopic",
  "pub author_id: Option<Uuid>",
  "author_id: item.topic.author_id",
  "Permission::FORUM_TOPICS_LIST",
]) need(unreadQuery, marker, "FORUM-15D unread topic author schema parity");

for (const marker of [
  "pub async fn read_for_audience(",
  "ProfilePresentationService::for_audience(",
  "forum_user_stat::Entity::find()",
]) need(owner, marker, "FORUM-15D shared owner baseline");

for (const marker of [
  "pub member_cards: Vec<ForumMemberCard>",
  "pub struct ForumMemberCard",
  "pub struct ForumMemberProfileSummary",
  "pub struct ForumMemberStats",
  'rename = "authorId"',
]) need(model, marker, "FORUM-15D storefront transport model");

for (const marker of [
  "authorId title slug",
  "topicId authorId content",
  "forumMemberCards(userIds: $userIds, locale: $locale)",
  "fn storefront_author_ids(",
  "let mut seen = HashSet::new()",
  "request_raw::<_, StorefrontForumMemberCardsResponse>(",
  "Err(error) if personalization_unavailable(&error) => Ok(Vec::new())",
  "member_cards,",
]) need(graphql, marker, "FORUM-15D GraphQL storefront path");
forbid(
  graphql,
  "forumStorefrontMemberCards",
  "FORUM-15D GraphQL storefront must reuse auth-gated member cards",
);


if (failures.length > 0) {
  console.error("Forum FORUM-15D storefront member-card transport source failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum FORUM-15D storefront member-card transport source: ok");
