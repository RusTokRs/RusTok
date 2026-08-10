#!/usr/bin/env node

import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

function need(text, marker, label) {
  if (!text.includes(marker)) throw new Error(`${label}: missing ${marker}`);
}

function forbid(text, marker, label) {
  if (text.includes(marker)) throw new Error(`${label}: forbidden ${marker}`);
}

const memberCardQuery = read("crates/rustok-forum/src/graphql/member_card_query.rs");
const unreadQuery = read("crates/rustok-forum/src/graphql/storefront_read_state.rs");
const owner = read("crates/rustok-forum/src/services/member_card.rs");
const model = read("crates/rustok-forum/storefront/src/model.rs");
const graphql = read("crates/rustok-forum/storefront/src/transport/graphql_adapter.rs");
const native = read("crates/rustok-forum/storefront/src/transport/native_server_adapter.rs");
const ui = read("crates/rustok-forum/storefront/src/ui/leptos.rs");
const cargo = read("crates/rustok-forum/storefront/Cargo.toml");
const packet = read(
  "docs/modules/forum-15-storefront-member-card-transport-actualization-2026-08-10.md",
);

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

for (const marker of [
  "ForumMemberCardService",
  "Permission::FORUM_TOPICS_READ",
  "let may_read_member_cards =",
  "fn storefront_author_ids(",
  ".read_for_audience(",
  "ForumMemberCardAudience::Authenticated",
  "ForumMemberCardAudience::TrustedService { actor_id: None }",
  "fn map_member_card(",
  "member_cards,",
]) need(native, marker, "FORUM-15D native storefront path");

need(ui, "member_cards: _,", "FORUM-15D UI compile-shape acceptance");
forbid(ui, "ForumMemberCard", "FORUM-15D visual member-card rendering remains open");
forbid(cargo, "rustok-profiles", "FORUM-15D storefront must not depend directly on Profiles");

for (const marker of [
  "FORUM-15D",
  "authenticated-stats",
  "anonymous-content-preserved",
  "does **not** add a public member-card statistics GraphQL field",
  "personalized unread GraphQL DTO",
  "at most one `forumMemberCards` request",
  "at most one `ForumMemberCardService` call",
  "no Cargo command, test, Node verifier",
]) need(packet, marker, "FORUM-15D actualization");

console.log("Forum FORUM-15D storefront member-card transport source: ok");
