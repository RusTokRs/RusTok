#!/usr/bin/env node

import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  return readFileSync(absolute, "utf8");
}

function requireText(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

function requireOrder(source, markers, message) {
  let previous = -1;
  for (const marker of markers) {
    const index = source.indexOf(marker, previous + 1);
    if (index < 0 || index <= previous) {
      failures.push(`${message}: missing or out-of-order marker ${marker}`);
      return;
    }
    previous = index;
  }
}

function reject(source, pattern, message) {
  if (pattern.test(source)) failures.push(message);
}

function collectRustFiles(root, relative = "") {
  const absolute = path.join(root, relative);
  if (!existsSync(absolute)) return [];
  const files = [];
  for (const entry of readdirSync(absolute)) {
    const childRelative = path.join(relative, entry);
    const childAbsolute = path.join(root, childRelative);
    const stat = statSync(childAbsolute);
    if (stat.isDirectory()) files.push(...collectRustFiles(root, childRelative));
    else if (entry.endsWith(".rs")) files.push(childRelative.replaceAll(path.sep, "/"));
  }
  return files;
}

const contract = JSON.parse(
  read("crates/rustok-forum/contracts/forum-mention-write-boundary.json") || "{}",
);
const dto = read(contract.quote_command?.dto ?? "");
const service = read(contract.quote_command?.service ?? "");
const tests = read("crates/rustok-forum/src/services/mention_relation_tests.rs");
const controller = read("crates/rustok-forum/src/controllers/quote_commands.rs");
const routes = read("crates/rustok-forum/src/controllers/mod.rs");
const graphql = read("crates/rustok-forum/src/graphql/quote_commands.rs");
const graphqlRoot = read("crates/rustok-forum/src/graphql/mod.rs");
const openapi = read("crates/rustok-forum/src/openapi.rs");
const record = read("crates/rustok-forum/docs/forum-12d1-quote-commands.md");

for (const [field, expected] of [
  ["quotes_field_required", true],
  ["empty_set_clears", true],
  ["reject_deleted_source", true],
  ["response_before_commit", true],
]) {
  if (contract.quote_command?.[field] !== expected) {
    failures.push(`machine boundary must set quote_command.${field}=${expected}`);
  }
}

for (const marker of [
  "ForumQuoteTargetKindInput",
  "ForumQuoteReferenceInput",
  "SetForumQuotesInput",
  "pub quotes: Vec<ForumQuoteReferenceInput>",
]) {
  requireText(dto, marker, `quote command DTO is missing ${marker}`);
}
reject(dto, /serde\s*\(\s*default/, "omitting quotes must not silently clear relations");
reject(graphql, /graphql\s*\(\s*default/, "omitting GraphQL quotes must not silently clear relations");

for (const marker of [
  "pub struct ForumQuoteCommandService",
  "set_topic_quotes",
  "set_reply_quotes",
  "enforce_owned_scope",
  "normalize_locale_tag",
  "FORUM_MAX_QUOTE_REFERENCES_PER_REVISION",
  "FORUM_MAX_MENTION_TARGETS_PER_REVISION",
  "BTreeSet",
  "MentionRelationService::new",
  "load_snapshot_in_tx",
  "deleted_at IS NULL",
  "ForumError::TopicDeleted",
  "ForumError::ReplyDeleted",
]) {
  requireText(service, marker, `quote owner service is missing ${marker}`);
}
requireOrder(
  service,
  [
    ".load_source(",
    "enforce_owned_scope",
    ".prepare(",
    "let txn = self.db.begin().await?;",
    ".persist_in_tx(&txn, prepared)",
    "load_snapshot_in_tx(",
    "txn.commit().await?;",
  ],
  "quote replacement and its response must stay inside one owner transaction",
);
requireText(service, "input.quotes", "an explicit empty quote list must reach owner replacement");
requireText(service, "quote_inputs_are_deduplicated_and_bounded", "quote bounds need owner unit coverage");
reject(service, /rustok_notifications|NotificationService/, "quote commands must not call Notifications");
reject(service, /handle_snapshot|projection_fingerprint/, "quote command DTO/service must not expose private relation fields");

for (const marker of [
  "quote_owner_replace_replay_clear_and_cross_tenant_rejection_are_atomic",
  "identical replacement should replay",
  "explicit empty list should clear quotes",
  "cross-tenant quoted revision must fail closed",
  "checked_sub(3)",
]) {
  requireText(tests, marker, `quote owner runtime scenario is missing ${marker}`);
}

for (const marker of [
  "/api/forum/topics/{id}/quotes",
  "/api/forum/replies/{id}/quotes",
  "ForumQuoteCommandService",
]) {
  requireText(controller + routes, marker, `REST quote boundary is missing ${marker}`);
}
for (const marker of [
  "set_forum_topic_quotes",
  "set_forum_reply_quotes",
  "GqlForumQuoteTargetKind",
  "ForumQuoteCommandMutation",
]) {
  requireText(graphql + graphqlRoot, marker, `GraphQL quote boundary is missing ${marker}`);
}
for (const marker of [
  "set_topic_quotes",
  "set_reply_quotes",
  "ForumQuoteReferenceInput",
  "SetForumQuotesInput",
]) {
  requireText(openapi, marker, `OpenAPI quote boundary is missing ${marker}`);
}

for (const root of contract.transport_roots ?? []) {
  for (const relative of collectRustFiles(path.join(repoRoot, root))) {
    const source = read(path.join(root, relative).replaceAll(path.sep, "/"));
    for (const symbol of contract.forbidden_transport_symbols ?? []) {
      if (source.includes(symbol)) {
        failures.push(`${root}/${relative}: transport must not access private symbol ${symbol}`);
      }
    }
  }
}

for (const marker of [
  "FORUM-12D1",
  "full replacement",
  "exact locale",
  "empty list clears",
  "omitting the list is rejected",
  "soft-deleted sources reject",
  "Maintainer verification was not executed",
]) {
  requireText(record, marker, `FORUM-12D1 implementation record is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("forum quote command verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum quote command verification passed");
