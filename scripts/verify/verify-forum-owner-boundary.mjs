#!/usr/bin/env node

import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function readRepo(relativePath) {
  return readFileSync(repoPath(relativePath), "utf8");
}

function fail(message) {
  failures.push(message);
}

function assertExists(relativePath, description) {
  if (!existsSync(repoPath(relativePath))) fail(description);
}

function assertContains(text, marker, description) {
  if (!text.includes(marker)) fail(description);
}

function assertNotMatch(text, pattern, description) {
  if (pattern.test(text)) fail(description);
}

function collectRustFiles(root, relative = "") {
  const absolute = path.join(root, relative);
  if (!existsSync(absolute)) return [];
  const files = [];
  for (const entry of readdirSync(absolute)) {
    if ([".git", "node_modules", "target"].includes(entry)) continue;
    const childRelative = path.join(relative, entry);
    const childAbsolute = path.join(root, childRelative);
    const stat = statSync(childAbsolute);
    if (stat.isDirectory()) files.push(...collectRustFiles(root, childRelative));
    else if (entry.endsWith(".rs")) files.push(childRelative.replaceAll(path.sep, "/"));
  }
  return files;
}

const servicesModPath = "crates/rustok-forum/src/services/mod.rs";
const categoryOwnerPath = "crates/rustok-forum/src/services/category_owner.rs";
const topicFacadePath = "crates/rustok-forum/src/services/topic_facade.rs";
const replyFacadePath = "crates/rustok-forum/src/services/reply_facade.rs";
const libPath = "crates/rustok-forum/src/lib.rs";

for (const filePath of [
  servicesModPath,
  categoryOwnerPath,
  topicFacadePath,
  replyFacadePath,
  libPath,
]) {
  assertExists(filePath, `${filePath}: required forum owner-boundary file is missing`);
}

const servicesMod = readRepo(servicesModPath);
const categoryOwner = readRepo(categoryOwnerPath);
const topicFacade = readRepo(topicFacadePath);
const replyFacade = readRepo(replyFacadePath);
const lib = readRepo(libPath);

assertContains(servicesMod, "mod topic;", `${servicesModPath}: raw topic persistence module must be crate-private`);
assertContains(servicesMod, "mod reply;", `${servicesModPath}: raw reply persistence module must be crate-private`);
assertContains(servicesMod, "pub use topic_facade::TopicService;", `${servicesModPath}: public TopicService must come from the explicit facade`);
assertContains(servicesMod, "pub use reply_facade::ReplyService;", `${servicesModPath}: public ReplyService must come from the explicit facade`);
assertNotMatch(servicesMod, /(^|\n)\s*pub\s+mod\s+(topic|reply|topic_owner|reply_owner)\s*;/, `${servicesModPath}: raw lifecycle modules must not be public`);
assertNotMatch(servicesMod, /pub\s+use\s+(topic_owner|reply_owner)::/, `${servicesModPath}: internal owner implementations must not be re-exported`);

for (const [filePath, source] of [
  [categoryOwnerPath, categoryOwner],
  [topicFacadePath, topicFacade],
  [replyFacadePath, replyFacade],
]) {
  assertNotMatch(source, /std::ops::Deref|impl\s+Deref\s+for/, `${filePath}: public facade must not dereference into an implementation service`);
}

for (const method of [
  "pub async fn create(",
  "pub async fn get(",
  "pub async fn get_with_locale_fallback(",
  "pub async fn update(",
  "pub async fn delete(",
  "pub async fn list(",
  "pub async fn list_with_locale_fallback(",
  "pub async fn list_paginated_with_locale_fallback(",
  "pub async fn tree(",
  "pub async fn move_category(",
  "pub async fn reorder_siblings(",
  "pub async fn archive_subtree(",
  "pub async fn restore_subtree(",
]) {
  assertContains(categoryOwner, method, `${categoryOwnerPath}: explicit category owner method missing: ${method}`);
}

for (const method of [
  "pub async fn create(",
  "pub async fn get(",
  "pub async fn get_with_locale_fallback(",
  "pub async fn update(",
  "pub async fn delete(",
  "pub async fn list(",
  "pub async fn list_with_locale_fallback(",
  "pub async fn list_storefront_visible_with_locale_fallback(",
]) {
  assertContains(topicFacade, method, `${topicFacadePath}: explicit topic facade method missing: ${method}`);
}

for (const method of [
  "pub async fn create(",
  "pub async fn get(",
  "pub async fn get_with_locale_fallback(",
  "pub async fn update(",
  "pub async fn delete(",
  "pub async fn list_for_topic(",
  "pub async fn list_for_topic_with_locale_fallback(",
  "pub async fn list_response_for_topic_with_locale_fallback(",
  "pub async fn list_response_for_topic_by_statuses_with_locale_fallback(",
]) {
  assertContains(replyFacade, method, `${replyFacadePath}: explicit reply facade method missing: ${method}`);
}

assertContains(lib, "CategoryService", `${libPath}: root CategoryService export must remain available`);
assertContains(lib, "ReplyService", `${libPath}: root ReplyService export must remain available`);
assertContains(lib, "TopicService", `${libPath}: root TopicService export must remain available`);

const forbiddenExternalPatterns = [
  /rustok_forum::services::topic(?:::|\b)/,
  /rustok_forum::services::reply(?:::|\b)/,
  /rustok_forum::services::topic_owner(?:::|\b)/,
  /rustok_forum::services::reply_owner(?:::|\b)/,
  /services::topic::TopicService/,
  /services::reply::ReplyService/,
  /services::topic_owner::TopicService/,
  /services::reply_owner::ReplyService/,
];
for (const relativePath of collectRustFiles(repoRoot)) {
  if (relativePath.startsWith("crates/rustok-forum/src/services/")) continue;
  const source = readRepo(relativePath);
  for (const pattern of forbiddenExternalPatterns) {
    if (pattern.test(source)) {
      fail(`${relativePath}: imports a non-public forum topic/reply implementation service`);
    }
  }
}

if (failures.length > 0) {
  console.error("forum owner boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum owner boundary verification passed");
