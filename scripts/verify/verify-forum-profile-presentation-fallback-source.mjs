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

const forumQuery = read("crates/modules/rustok-forum/src/graphql/query_runtime.rs");
const presentation = read("crates/modules/rustok-profiles/src/presentation.rs");
const loader = read("crates/modules/rustok-profiles/src/loader.rs");
const hostPolicy = read("apps/server/src/graphql/profile_summary_policy.rs");
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


console.log("Forum FORUM-15A profile presentation fallback source: ok");
