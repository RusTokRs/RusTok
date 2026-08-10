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

const forumQuery = read("crates/rustok-forum/src/graphql/query_runtime.rs");
const presentation = read("crates/rustok-profiles/src/presentation.rs");
const loader = read("crates/rustok-profiles/src/loader.rs");
const hostPolicy = read("apps/server/src/graphql/profile_summary_policy.rs");
const packet = read(
  "docs/modules/forum-15-profile-presentation-fallback-actualization-2026-08-10.md",
);

for (const marker of [
  "ProfilePresentationService, ProfileSummaryLoader, ProfileSummaryLoaderKey",
  "ctx.data_opt::<DataLoader<ProfileSummaryLoader>>()",
  "loader.load_many(keys).await?",
  "ProfilePresentationService::new(db.clone())",
  ".find_profile_summaries(",
  ".collect::<HashSet<_>>()",
]) need(forumQuery, marker, "FORUM-15A Forum profile composition");

for (const marker of [
  "ProfileService::new(db.clone())\n        .find_profile_summaries(",
  "entities::profile::Entity",
  "ProfilePrivacyService::new",
]) forbid(forumQuery, marker, "FORUM-15A Forum must not bypass Profiles presentation owner");

for (const marker of [
  "pub struct ProfilePresentationService",
  "ProfilePrivacyService::new(self.db.clone())",
  ".evaluate_access_batch(tenant_id, user_ids, self.audience)",
  "ProfilePrivacyDecision::Allow",
  "ProfileError::PresentationUnavailable",
  "impl ProfilesReader for ProfilePresentationService",
]) need(presentation, marker, "Profiles presentation owner baseline");

for (const marker of [
  "ProfilePresentationService::for_audience(db, audience)",
  ".find_profile_summaries(",
  "ProfileSummaryBatchKey",
]) need(loader, marker, "Profiles request loader baseline");

for (const marker of [
  "ProfileAccessAudience::Anonymous",
  "ProfileAccessAudience::Authenticated",
  "ProfileAccessAudience::TrustedService",
  "ProfileSummaryLoader::for_audience(db, audience)",
  "request.data.insert(DataLoader::new(",
]) need(hostPolicy, marker, "Host profile audience composition baseline");

for (const marker of [
  "FORUM-15A",
  "fallback-fail-closed",
  "anonymous/fail-closed",
  "request-scoped loader as the preferred path",
  "does not claim complete member cards",
  "Forum-owned statistics",
  "no Cargo command, test, Node verifier",
]) need(packet, marker, "FORUM-15A actualization");

console.log("Forum FORUM-15A profile presentation fallback source: ok");
