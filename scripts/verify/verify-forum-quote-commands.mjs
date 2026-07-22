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
const relationTests = read("crates/rustok-forum/src/services/mention_relation_tests.rs");
const controller = read("crates/rustok-forum/src/controllers/quote_commands.rs");
const routes = read("crates/rustok-forum/src/controllers/mod.rs");
const graphql = read("crates/rustok-forum/src/graphql/quote_commands.rs");
const graphqlRoot = read("crates/rustok-forum/src/graphql/mod.rs");
const openapi = read("crates/rustok-forum/src/openapi.rs");
const d1Record = read("crates/rustok-forum/docs/forum-12d1-quote-commands.md");

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
reject(dto, /serde\s*\(\s*default/, "omitting D1 quotes must not silently clear relations");
reject(graphql, /graphql\s*\(\s*default/, "omitting D1 GraphQL quotes must not silently clear relations");

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
  requireText(relationTests, marker, `quote owner runtime scenario is missing ${marker}`);
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

const inline = contract.inline_quote_commands ?? {};
for (const [field, expected] of [
  ["create_omitted_quotes", "empty_initial_set"],
  ["update_omitted_quotes", "preserve_latest_exact_locale_set"],
  ["update_explicit_empty_quotes", "clear"],
  ["preserve_expected_revision_cas", true],
  ["conflict_code", "FORUM_RELATION_REVISION_CONFLICT"],
  ["conflict_retryable", true],
  ["legacy_rust_dtos_unchanged", true],
  ["legacy_facade_updates_preserve_quotes", true],
]) {
  if (inline[field] !== expected) {
    failures.push(`machine boundary must set inline_quote_commands.${field}=${expected}`);
  }
}

const topicDto = read(inline.topic_dto ?? "");
const replyDto = read(inline.reply_dto ?? "");
const resolver = read(inline.resolver ?? "");
const moduleComposition = read(inline.module_composition ?? "");
const topicOwner = read(contract.owner_entrypoints?.topic_update?.owner ?? "");
const topicImplementation = read(contract.owner_entrypoints?.topic_update?.implementation ?? "");
const replyOwner = read(contract.owner_entrypoints?.reply_create?.owner ?? "");
const replyImplementation = read(contract.owner_entrypoints?.reply_update?.implementation ?? "");
const topicFacade = read("crates/rustok-forum/src/services/topic_facade.rs");
const replyFacade = read("crates/rustok-forum/src/services/reply_facade.rs");
const contentController = read("crates/rustok-forum/src/controllers/content_commands.rs");
const contentGraphql = read("crates/rustok-forum/src/graphql/content_commands.rs");
const inlineTests = read("crates/rustok-forum/src/services/relation_quote_input_tests.rs");
const d2Record = read("crates/rustok-forum/docs/forum-12d2-inline-quote-commands.md");

for (const marker of [
  "CreateTopicCommandInput",
  "UpdateTopicCommandInput",
  "#[serde(default)]",
  "pub quotes: Vec<ForumQuoteReferenceInput>",
  "pub quotes: Option<Vec<ForumQuoteReferenceInput>>",
  "update_command_distinguishes_omitted_quotes_from_explicit_clear",
]) {
  requireText(topicDto, marker, `topic inline quote DTO is missing ${marker}`);
}
for (const marker of [
  "CreateReplyCommandInput",
  "UpdateReplyCommandInput",
  "#[serde(default)]",
  "pub quotes: Vec<ForumQuoteReferenceInput>",
  "pub quotes: Option<Vec<ForumQuoteReferenceInput>>",
  "update_command_distinguishes_omitted_quotes_from_explicit_clear",
]) {
  requireText(replyDto, marker, `reply inline quote DTO is missing ${marker}`);
}

for (const marker of [
  "InlineQuoteExpectation",
  "Exact(Option<i64>)",
  "resolve_inline_update_quotes",
  "lock_source_and_assert_latest_in_tx",
  "deleted_at IS NULL",
  "ForumError::RelationRevisionConflict",
  "FORUM_MAX_QUOTE_REFERENCES_PER_REVISION",
]) {
  requireText(resolver, marker, `inline quote resolver is missing ${marker}`);
}
requireOrder(
  resolver,
  [
    "lock_active_source_in_tx",
    "InlineQuoteExpectation::Exact",
    "forum_relation_revision::Entity::find()",
    "ForumError::RelationRevisionConflict",
  ],
  "preserved quotes must lock the source and compare the expected relation revision",
);

for (const [source, label] of [
  [topicImplementation, "topic implementation"],
  [replyImplementation, "reply implementation"],
]) {
  for (const marker of [
    "resolve_inline_update_quotes",
    "lock_source_and_assert_latest_in_tx",
    ".persist_in_tx",
    "txn.commit().await?",
  ]) {
    requireText(source, marker, `${label} is missing ${marker}`);
  }
  requireOrder(
    source,
    [
      "resolve_inline_update_quotes",
      ".prepare(",
      "let txn = self.db.begin().await?;",
      "lock_source_and_assert_latest_in_tx",
      ".persist_in_tx",
      "txn.commit().await?",
    ],
    `${label} must preserve quote resolution and persistence order`,
  );
}
for (const marker of ["create_command", "update_command", "create_with_inline_relations", "update_with_inline_relations"]) {
  requireText(topicOwner + replyOwner, marker, `owner command extension is missing ${marker}`);
}
for (const marker of [
  "self.create_command",
  "self.update_command",
  "input.into()",
]) {
  requireText(topicFacade + replyFacade, marker, `legacy facade compatibility is missing ${marker}`);
}
for (const marker of [
  'include!("topic.rs")',
  'include!("topic_inline.rs")',
  'include!("reply.rs")',
  'include!("reply_inline.rs")',
  'include!("topic_owner_inline.rs")',
  'include!("reply_owner_inline.rs")',
]) {
  requireText(moduleComposition, marker, `module composition is missing ${marker}`);
}

for (const marker of [
  "content_commands::create_topic",
  "content_commands::update_topic",
  "content_commands::create_reply",
  "content_commands::update_reply",
  "CreateTopicCommandInput",
  "UpdateReplyCommandInput",
  "StatusCode::CONFLICT",
]) {
  requireText(routes + contentController, marker, `REST inline quote boundary is missing ${marker}`);
}
for (const marker of [
  "create_forum_topic_with_quotes",
  "update_forum_topic_with_quotes",
  "create_forum_reply_with_quotes",
  "update_forum_reply_with_quotes",
  "ForumContentCommandMutation",
]) {
  requireText(contentGraphql + graphqlRoot, marker, `GraphQL inline quote boundary is missing ${marker}`);
}
for (const marker of [
  "content_commands::create_topic",
  "content_commands::update_topic",
  "content_commands::create_reply",
  "content_commands::update_reply",
  "CreateTopicCommandInput",
  "UpdateReplyCommandInput",
]) {
  requireText(openapi, marker, `OpenAPI inline quote boundary is missing ${marker}`);
}

for (const marker of [
  "inline_quote_preserve_detects_concurrent_relation_replacement",
  "stale omitted snapshot must conflict",
  "FORUM_RELATION_REVISION_CONFLICT",
  "InlineQuoteExpectation::Any",
]) {
  requireText(inlineTests, marker, `inline quote runtime scenario is missing ${marker}`);
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
  requireText(d1Record, marker, `FORUM-12D1 implementation record is missing ${marker}`);
}
for (const marker of [
  "FORUM-12D2",
  "omitted updates preserve",
  "explicit empty list clears",
  "FORUM_RELATION_REVISION_CONFLICT",
  "legacy Rust DTOs remain unchanged",
  "Maintainer verification was not executed",
]) {
  requireText(d2Record, marker, `FORUM-12D2 implementation record is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("forum quote command verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum quote command verification passed");
