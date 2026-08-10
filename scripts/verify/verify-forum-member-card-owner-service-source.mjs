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

const owner = read("crates/rustok-forum/src/services/member_card.rs");
const userStats = read("crates/rustok-forum/src/services/user_stats.rs");
const graphql = read("crates/rustok-forum/src/graphql/member_card_query.rs");
const storefrontCargo = read("crates/rustok-forum/storefront/Cargo.toml");
const storefrontModel = read("crates/rustok-forum/storefront/src/model.rs");
const storefrontUi = read("crates/rustok-forum/storefront/src/ui/leptos.rs");
const packet = read(
  "docs/modules/forum-15-member-card-owner-service-actualization-2026-08-10.md",
);

for (const marker of [
  "pub const MAX_FORUM_MEMBER_CARD_USER_IDS: usize = 100",
  "pub enum ForumMemberCardAudience",
  "Anonymous",
  "Authenticated { actor_id: Uuid }",
  "TrustedService { actor_id: Option<Uuid> }",
  "pub struct ForumMemberStats",
  "pub struct ForumMemberCard",
  "pub struct ForumMemberCardService",
  "pub fn normalize_user_ids(user_ids: &[Uuid])",
  "pub async fn read_for_audience(",
  "pub(crate) async fn compose_admitted_profiles(",
  "ProfilePresentationService::for_audience(",
  ".find_profile_summaries(",
  "profiles.contains_key(user_id)",
  "forum_user_stat::Entity::find()",
  "forum_user_stat::Column::TenantId.eq(tenant_id)",
  "forum_user_stat::Column::UserId.is_in(visible_user_ids.clone())",
  "stats.remove(&user_id).unwrap_or_default()",
  "error.code()",
  "error.is_retryable()",
]) need(owner, marker, "FORUM-15C owner service");

const presentationIndex = owner.indexOf(".find_profile_summaries(");
const visibleIndex = owner.indexOf("profiles.contains_key(user_id)");
const statsIndex = owner.indexOf("forum_user_stat::Entity::find()");
if (!(presentationIndex >= 0 && visibleIndex > presentationIndex && statsIndex > visibleIndex)) {
  throw new Error("FORUM-15C Profiles admission must precede Forum statistics access");
}

for (const marker of [
  "pub async fn compose_admitted_profiles(",
  "ProfileService::new(",
  "rustok_profiles::entities",
  "SocialGraphService::new",
  "PROFILES_PRESENTATION_FAILED",
]) forbid(owner, marker, "FORUM-15C owner boundary");

need(userStats, 'include!("member_card.rs");', "FORUM-15C user-stats wiring");

for (const marker of [
  "ForumMemberCardService::new(db.clone())",
  "ForumMemberCardService::normalize_user_ids(&user_ids)",
  "ctx.data_opt::<DataLoader<ProfileSummaryLoader>>()",
  ".compose_admitted_profiles(tenant.id, &requested_user_ids, profiles)",
  "ForumMemberCardAudience::Anonymous",
  ".read_for_audience(",
]) need(graphql, marker, "FORUM-15C GraphQL adapter");

for (const marker of [
  "forum_user_stat::Entity::find()",
  "ProfilePresentationService::new(",
  "ProfilePresentationService::for_audience(",
]) forbid(graphql, marker, "FORUM-15C GraphQL must stay thin");

forbid(
  storefrontCargo,
  "rustok-profiles",
  "FORUM-15C storefront must not gain a direct Profiles dependency",
);
for (const marker of [
  "pub struct ForumTopicListItem",
  "pub struct ForumTopicDetail",
  "pub struct ForumReplyDetail",
]) need(storefrontModel, marker, "FORUM-15C storefront baseline");
forbid(storefrontUi, "ForumMemberCard", "FORUM-15C storefront UI integration remains open");

for (const marker of [
  "FORUM-15C",
  "shared-owner-service",
  "pub(crate)",
  "before any Forum statistics are queried",
  "storefront package has no direct `rustok-profiles` dependency",
  "no Cargo command, test, Node verifier",
]) need(packet, marker, "FORUM-15C actualization");

console.log("Forum FORUM-15C member-card owner service source: ok");
