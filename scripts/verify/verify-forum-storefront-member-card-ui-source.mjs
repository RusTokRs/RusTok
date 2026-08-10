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

function count(text, marker) {
  return text.split(marker).length - 1;
}

const leptos = read("crates/rustok-forum/storefront/src/ui/leptos.rs");
const memberCard = read("crates/rustok-forum/storefront/src/ui/member_card.rs");
const uiMod = read("crates/rustok-forum/storefront/src/ui/mod.rs");
const model = read("crates/rustok-forum/storefront/src/model.rs");
const graphql = read("crates/rustok-forum/storefront/src/transport/graphql_adapter.rs");
const native = read("crates/rustok-forum/storefront/src/transport/native_server_adapter.rs");
const cargo = read("crates/rustok-forum/storefront/Cargo.toml");
const en = read("crates/rustok-forum/storefront/locales/en.json");
const ru = read("crates/rustok-forum/storefront/locales/ru.json");
const packet = read(
  "docs/modules/forum-15-storefront-member-card-ui-actualization-2026-08-10.md",
);

for (const marker of [
  "pub member_cards: Vec<ForumMemberCard>",
  'rename = "authorId"',
  "pub struct ForumMemberCard",
]) need(model, marker, "FORUM-15E transport baseline");

for (const marker of [
  "member_cards,",
  "provide_context(member_card_context(member_cards));",
  "use super::member_card::{ForumAuthorBadge, member_card_context};",
]) need(leptos, marker, "FORUM-15E storefront composition");
if (count(leptos, "<ForumAuthorBadge author_id />") !== 3) {
  throw new Error("FORUM-15E must render the shared author badge on feed, opening post and reply surfaces");
}
forbid(leptos, "member_cards: _", "FORUM-15E member-card payload must be consumed");

for (const marker of [
  "pub type ForumMemberCardContext = Arc<HashMap<String, ForumMemberCard>>",
  "pub fn member_card_context(",
  "pub fn ForumAuthorBadge(",
  "cards.get(user_id)",
  "card.map(|card|",
  'class="forum-member-card ',
  '"forum.member.topics"',
  '"forum.member.replies"',
  '"forum.member.solutions"',
  "fn profile_initials(",
]) need(memberCard, marker, "FORUM-15E member-card presentation helper");

for (const source of [leptos, memberCard]) {
  for (const marker of [
    "forumMemberCards",
    "ForumMemberCardService",
    "rustok_profiles",
    "rustok_media",
    "avatar_media_id",
    "author_id.unwrap",
    "author_id.expect",
  ]) forbid(source, marker, "FORUM-15E UI must remain owner-read-free and fail closed");
}

for (const marker of [
  '"forum.member.topics"',
  '"forum.member.replies"',
  '"forum.member.solutions"',
]) {
  need(en, marker, "FORUM-15E English locale");
  need(ru, marker, "FORUM-15E Russian locale");
}

need(uiMod, "mod member_card;", "FORUM-15E UI module wiring");
forbid(cargo, "rustok-profiles", "FORUM-15E storefront must not add Profiles dependency");
forbid(cargo, "rustok-media", "FORUM-15E storefront must not add Media dependency");

for (const marker of [
  "FORUM-15E",
  "privacy-filtered-lookup",
  "no-new-owner-reads",
  "Media-backed avatar rendering remains open",
  "browser/runtime evidence",
  "no Cargo command, test, Node verifier",
]) need(packet, marker, "FORUM-15E actualization");

for (const marker of [
  "forumMemberCards(userIds: $userIds, locale: $locale)",
  "Err(error) if personalization_unavailable(&error) => Ok(Vec::new())",
]) need(graphql, marker, "FORUM-15E GraphQL transport baseline");
for (const marker of [
  "Permission::FORUM_TOPICS_READ",
  "ForumMemberCardService::new(db.clone())",
]) need(native, marker, "FORUM-15E native transport baseline");

console.log("Forum FORUM-15E storefront member-card UI source: ok");
