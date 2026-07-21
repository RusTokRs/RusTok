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
const topicOwnerPath = "crates/rustok-forum/src/services/topic_owner.rs";
const replyOwnerPath = "crates/rustok-forum/src/services/reply_owner.rs";
const libPath = "crates/rustok-forum/src/lib.rs";
const crateApiPath = "crates/rustok-forum/CRATE_API.md";
const planPath = "crates/rustok-forum/docs/implementation-plan.md";

for (const filePath of [
  servicesModPath,
  topicOwnerPath,
  replyOwnerPath,
  libPath,
  crateApiPath,
  planPath,
]) {
  assertExists(filePath, `${filePath}: required forum owner-boundary file is missing`);
}

const servicesMod = readRepo(servicesModPath);
const topicOwner = readRepo(topicOwnerPath);
const replyOwner = readRepo(replyOwnerPath);
const lib = readRepo(libPath);
const crateApi = readRepo(crateApiPath);
const plan = readRepo(planPath);

assertContains(servicesMod, "mod topic;", `${servicesModPath}: raw topic persistence module must be crate-private`);
assertContains(servicesMod, "mod reply;", `${servicesModPath}: raw reply persistence module must be crate-private`);
assertNotMatch(servicesMod, /(^|\n)\s*pub\s+mod\s+(topic|reply)\s*;/, `${servicesModPath}: raw lifecycle modules must not be public`);

for (const [filePath, source] of [[topicOwnerPath, topicOwner], [replyOwnerPath, replyOwner]]) {
  assertNotMatch(source, /std::ops::Deref|impl\s+Deref\s+for/, `${filePath}: public owner service must not dereference into persistence service`);
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
  assertContains(topicOwner, method, `${topicOwnerPath}: explicit owner method missing: ${method}`);
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
  assertContains(replyOwner, method, `${replyOwnerPath}: explicit owner method missing: ${method}`);
}

assertContains(lib, "ReplyService", `${libPath}: root ReplyService export must remain available`);
assertContains(lib, "TopicService", `${libPath}: root TopicService export must remain available`);
assertContains(crateApi, "verify-forum-owner-boundary.mjs", `${crateApiPath}: owner-boundary verifier must be documented`);
assertContains(plan, "verify-forum-owner-boundary.mjs", `${planPath}: implementation plan must record the owner-boundary verifier`);

const forbiddenExternalPatterns = [
  /rustok_forum::services::topic(?:::|\b)/,
  /rustok_forum::services::reply(?:::|\b)/,
  /services::topic::TopicService/,
  /services::reply::ReplyService/,
];
for (const relativePath of collectRustFiles(repoRoot)) {
  if (relativePath.startsWith("crates/rustok-forum/src/services/")) continue;
  const source = readRepo(relativePath);
  for (const pattern of forbiddenExternalPatterns) {
    if (pattern.test(source)) {
      fail(`${relativePath}: imports a raw forum topic/reply persistence service`);
    }
  }
}

if (failures.length > 0) {
  console.error("forum owner boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum owner boundary verification passed");
