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

const query = read("crates/rustok-forum/src/graphql/member_card_query.rs");
const gqlMod = read("crates/rustok-forum/src/graphql/mod.rs");
const profilePresentation = read("crates/rustok-profiles/src/presentation.rs");
const profileLoader = read("crates/rustok-profiles/src/loader.rs");
const packet = read("docs/modules/forum-15-member-card-stats-actualization-2026-08-10.md");

for (const marker of [
  "pub const MAX_FORUM_MEMBER_CARD_USER_IDS: usize = 100",
  "async fn forum_member_cards(",
  "Permission::FORUM_TOPICS_READ",
  "if user_ids.len() > MAX_FORUM_MEMBER_CARD_USER_IDS",
  "if user_id.is_nil()",
  "seen.insert(user_id)",
  "load_visible_profiles(",
  "profiles.contains_key(user_id)",
  "load_forum_stats(db, tenant.id, &visible_user_ids)",
  "ProfilePresentationService::new(db.clone())",
  "ctx.data_opt::<DataLoader<ProfileSummaryLoader>>()",
  "loader.load_many(keys).await?",
  "forum_user_stat::Entity::find()",
  "forum_user_stat::Column::TenantId.eq(tenant_id)",
  "forum_user_stat::Column::UserId.is_in(visible_user_ids.to_vec())",
]) need(query, marker, "FORUM-15B member-card query");

const profileIndex = query.indexOf("load_visible_profiles(");
const visibleFilterIndex = query.indexOf("profiles.contains_key(user_id)");
const statsIndex = query.indexOf("load_forum_stats(db, tenant.id, &visible_user_ids)");
if (!(profileIndex >= 0 && visibleFilterIndex > profileIndex && statsIndex > visibleFilterIndex)) {
  throw new Error("FORUM-15B privacy admission must precede Forum stats loading");
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

console.log("Forum FORUM-15B member-card statistics source: ok");
