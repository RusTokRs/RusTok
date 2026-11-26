#!/usr/bin/env node

import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { containsMarker } from "./lib/forum-source-verification.mjs";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  if (typeof relativePath !== "string" || relativePath.length === 0) {
    failures.push("<missing path>: required file is missing");
    return "";
  }
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  return readFileSync(absolute, "utf8");
}

function requireText(source, marker, message) {
  if (!containsMarker(source, marker)) failures.push(message);
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
  read("crates/modules/rustok-forum/contracts/forum-mention-write-boundary.json") || "{}",
);
for (const [name, entry] of Object.entries(contract.owner_entrypoints ?? {})) {
  const owner = read(entry.owner);
  const implementation = read(entry.implementation);
  if (typeof entry.delegate !== "string" || entry.delegate.length === 0) {
    failures.push(`${name}: owner delegate must be declared`);
  } else {
    requireText(owner, entry.delegate, `${name}: declared owner delegate is missing`);
    requireText(implementation, entry.delegate, `${name}: declared implementation is missing`);
  }
}
const dto = read(contract.quote_command?.dto ?? "");
const service = read(contract.quote_command?.service ?? "");
const engagementGuard = read("crates/modules/rustok-forum/src/services/topic_engagement_guard.rs");
const relationTests = read("crates/modules/rustok-forum/src/services/mention_relation_tests.rs");
const controller = read("crates/modules/rustok-forum/src/controllers/quote_commands.rs");
const routes = read("crates/modules/rustok-forum/src/controllers/mod.rs");
const graphql = read("crates/modules/rustok-forum/src/graphql/quote_commands.rs");
const graphqlRoot = read("crates/modules/rustok-forum/src/graphql/mod.rs");
const openapi = read("crates/modules/rustok-forum/src/openapi.rs");
const api = read("crates/modules/rustok-forum/CRATE_API.md");
const ownerReadme = read("crates/modules/rustok-forum/README.md");
const graphqlRuntime = read("crates/modules/rustok-forum/src/graphql/runtime_data.rs");

for (const [field, expected] of [
  ["quotes_field_required", true],
  ["empty_set_clears", true],
  ["reject_deleted_source", true],
  ["reject_archived_parent", true],
  ["reject_locked_reply_parent", true],
  ["recheck_author_window_in_tx", true],
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
    "lock_category_tree_in_tx(&txn, tenant_id)",
    ".enforce_source_edit_in_tx(&txn, tenant_id, source, &security)",
    ".persist_in_tx(&txn, prepared)",
    "load_snapshot_in_tx(",
    "txn.commit().await?;",
  ],
  "quote replacement and its response must stay inside one owner transaction",
);
const transactionalAdmission = service.slice(
  service.indexOf("    async fn enforce_source_edit_in_tx("),
  service.indexOf("    async fn load_source("),
);
requireOrder(transactionalAdmission, [
  "find_topic_for_update_in_tx(",
  "find_reply_for_update_in_tx(",
  "enforce_owned_scope(",
  "module_settings_in_tx(",
  "enforce_author_edit_window(",
], "quote admission must lock parent and reply before checking policy/time");
requireText(service, "input.quotes", "an explicit empty quote list must reach owner replacement");
requireText(service, "quote_inputs_are_deduplicated_and_bounded", "quote bounds need owner unit coverage");
reject(service, /rustok_notifications|NotificationService/, "quote commands must not call Notifications");
reject(service, /handle_snapshot|projection_fingerprint/, "quote command DTO/service must not expose private relation fields");

for (const marker of [
  "quote_owner_replace_replay_clear_and_cross_tenant_rejection_are_atomic",
  "identical replacement should replay",
  "explicit empty list should clear quotes",
  "cross-tenant quoted revision must fail closed",
  "before_foreign",
  "failed quote replacement must not append a relation revision",
  "standalone_quote_commands_recheck_transactional_policy_before_relation_writes",
  "must read the transactional policy",
  "rejected transactional admission must append no relation revision",
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
const topicFacade = read("crates/modules/rustok-forum/src/services/topic_facade.rs");
const replyFacade = read("crates/modules/rustok-forum/src/services/reply_facade.rs");
const contentController = read("crates/modules/rustok-forum/src/controllers/content_commands.rs");
const contentGraphql = read("crates/modules/rustok-forum/src/graphql/content_commands.rs");
const inlineTests = read(inline.runtime_tests ?? "");

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
// Ordinary modules replaced the removed inline fragments. Check registration,
// never require restoring source stitching merely to satisfy a stale pin.
for (const name of ["topic", "reply", "topic_owner", "reply_owner", "relation_quote_input"]) {
  requireText(moduleComposition, `mod ${name};`, `module composition is missing ${name}`);
}
reject(moduleComposition, /include!\s*\(/, "owner modules must not use source stitching");

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

// The live owner contract supersedes removed per-slice handoff records.
for (const marker of [
  "### Quote owner commands", "exact source locale", "full replacement",
  "An empty list explicitly clears quotes", "FORUM_QUOTE_TARGET_UNAVAILABLE",
  "Identical replacement replays", "Standalone quote commands recheck",
  "### Inline quote create and edit commands", "quotes: []", "FORUM_RELATION_REVISION_CONFLICT",
  "Existing `TopicService::create/update` and `ReplyService::create/update`",
]) {
  requireText(api, marker, `canonical quote API contract is missing ${marker}`);
}
for (const marker of ["author edit window", "Archived topics are immutable", "locked topics reject reply quote edits"]) {
  requireText(ownerReadme, marker, `quote lifecycle contract is missing ${marker}`);
}
for (const [source, label] of [[routes, "REST runtime"], [graphqlRuntime, "GraphQL runtime"]]) {
  const start = source.indexOf("fn quote_command_service(");
  const factory = source.slice(start, source.indexOf("\n    }", start) + 6);
  requireText(factory, "fn quote_command_service(", `${label} must compose a quote owner`);
  requireText(factory, ".with_settings_providers(self.settings_providers.clone())", `${label} must pass configured settings`);
}
requireText(controller, ".quote_command_service()", "REST must use the configured quote owner");
requireText(graphql, "super::forum_graphql_runtime(ctx)", "GraphQL must use its routed runtime");
requireText(graphql, ".quote_command_service(db.clone())", "GraphQL must use the configured quote owner");
reject(controller + graphql, /ForumQuoteCommandService::new\s*\(/, "quote transports must not bypass the configured owner");
const admission = service.slice(service.indexOf("async fn enforce_source_edit_in_tx("), service.indexOf("async fn load_source("));
for (const marker of [
  "enforce_source_edit_in_tx", "find_topic_for_update_in_tx", "enforce_topic_engagement(",
  "TopicEngagementKind::UpdateTopic", "TopicEngagementKind::UpdateReply",
  "module_settings_in_tx", "enforce_author_edit_window",
]) {
  requireText(admission, marker, `transactional quote admission is missing ${marker}`);
}

for (const marker of [
  "TopicStatus::Archived => return Err(ForumError::TopicArchived)",
  "topic.is_locked && !may_write_locked_topic(security, resource)",
  "return Err(ForumError::TopicLocked)",
]) {
  requireText(engagementGuard, marker, `canonical engagement admission is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("forum quote command verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum quote command verification passed");
