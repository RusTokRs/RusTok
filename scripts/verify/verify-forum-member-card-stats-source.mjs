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

const query = read("crates/modules/rustok-forum/src/graphql/member_card_query.rs");
const owner = read("crates/modules/rustok-forum/src/services/member_card.rs");
const gqlMod = read("crates/modules/rustok-forum/src/graphql/mod.rs");
const profilePresentation = read("crates/modules/rustok-profiles/src/presentation.rs");
const profileLoader = read("crates/modules/rustok-profiles/src/loader.rs");
const packet = read("docs/modules/forum-15-member-card-stats-actualization-2026-08-10.md");

// The resolver is now a thin transport: it admits the permission, normalises the request and hands
// it to the owner service, which owns privacy admission and the single statistics read.
for (const marker of [
  "async fn forum_member_cards(",
  "require_module_enabled(ctx, MODULE_SLUG).await?",
  "Permission::FORUM_TOPICS_READ",
  "ForumMemberCardService::new(db.clone())",
  "ForumMemberCardService::normalize_user_ids(&user_ids)",
  "ctx.data_opt::<DataLoader<ProfileSummaryLoader>>()",
  "ProfileSummaryLoaderKey {",
  ".load_many(keys)",
  ".compose_admitted_profiles(tenant.id, &requested_user_ids, profiles)",
  ".read_for_audience(",
]) need(query, marker, "FORUM-15B member-card resolver");

for (const marker of [
  "pub const MAX_FORUM_MEMBER_CARD_USER_IDS: usize = 100",
  "if user_ids.len() > MAX_FORUM_MEMBER_CARD_USER_IDS",
  "if user_id.is_nil()",
  "seen.insert(user_id)",
  "pub fn normalize_user_ids(user_ids: &[Uuid])",
  "pub async fn read_for_audience(",
  "pub(crate) async fn compose_admitted_profiles(",
  ".filter(|user_id| profiles.contains_key(user_id))",
  "forum_user_stat::Entity::find()",
  "forum_user_stat::Column::TenantId.eq(tenant_id)",
  "forum_user_stat::Column::UserId.is_in(visible_user_ids.clone())",
]) need(owner, marker, "FORUM-15B member-card owner");

// Only identities admitted by the Profiles owner may reach the Forum statistics read.
const visibleFilterIndex = owner.indexOf(".filter(|user_id| profiles.contains_key(user_id))");
const statsIndex = owner.indexOf("forum_user_stat::Entity::find()");
if (!(visibleFilterIndex >= 0 && statsIndex > visibleFilterIndex)) {
  failures.push("FORUM-15B privacy admission must precede Forum stats loading");
}

for (const marker of [
  "Permission::FORUM_TOPICS_LIST",
  "Permission::FORUM_REPLIES_LIST",
  "ProfileService::new(",
  "ProfilePrivacyService",
  "rustok_profiles::entities",
  "UserStatsService::new",
  ".get(tenant_id",
]) forbid(query, marker, "FORUM-15B must not broaden or bypass batch owner composition");

for (const marker of [
  "mod member_card_query;",
  "GqlForumMemberCard, GqlForumMemberStats, MAX_FORUM_MEMBER_CARD_USER_IDS",
  "member_card_query::ForumMemberCardQuery",
]) need(gqlMod, marker, "FORUM-15B GraphQL wiring");

for (const marker of [
  "ProfilePrivacyService::new(self.db.clone())",
  ".evaluate_access_batch(tenant_id, user_ids, self.audience)",
  "ProfilePrivacyDecision::Allow",
  "ProfileError::PresentationUnavailable",
]) need(profilePresentation, marker, "Profiles presentation owner baseline");

for (const marker of [
  "ProfilePresentationService::for_audience(db, audience)",
  "ProfileSummaryBatchKey",
  ".find_profile_summaries(",
]) need(profileLoader, marker, "Profiles batch loader baseline");

for (const marker of [
  "FORUM-15B",
  "profile-privacy-authoritative",
  "MAX_FORUM_MEMBER_CARD_USER_IDS = 100",
  "one `forum_user_stats` query",
  "is not claimed here",
  "no Cargo command, test, Node verifier",
]) need(packet, marker, "FORUM-15B actualization");

if (failures.length > 0) {
  console.error("Forum FORUM-15B member-card statistics source failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum FORUM-15B member-card statistics source: ok");
