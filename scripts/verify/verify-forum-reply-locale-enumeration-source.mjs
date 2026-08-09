#!/usr/bin/env node

import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

function requireText(text, marker, message) {
  if (!text.includes(marker)) throw new Error(message);
}

function requireAbsent(text, marker, message) {
  if (text.includes(marker)) throw new Error(message);
}

const sourcePath = "crates/rustok-forum/src/services/reply_inline.rs";
const packetPath = "docs/modules/forum-34-reply-locale-enumeration-actualization-2026-08-09.md";

const source = read(sourcePath);
const packet = read(packetPath);

for (const marker of [
  "pub const MAX_FORUM_REPLY_LOCALE_ENUMERATION_IDS: usize = 512;",
  "pub async fn available_locales_for_replies(",
  "enforce_scope(&security, Resource::ForumReplies, Action::List)?;",
  "tenant_id.is_nil()",
  "reply_ids.len() > Self::MAX_FORUM_REPLY_LOCALE_ENUMERATION_IDS",
  "let mut seen = BTreeSet::new();",
  "!seen.insert(*reply_id)",
  "forum_reply::Entity::find()",
  ".filter(forum_reply::Column::TenantId.eq(tenant_id))",
  ".filter(forum_reply::Column::Id.is_in(reply_ids.to_vec()))",
  "ForumError::ReplyNotFound(*reply_id)",
  "self.load_bodies_map(tenant_id, reply_ids).await?",
  "available_locales_from(&bodies, |body| body.locale.as_str())",
  "result.push((*reply_id, locales));",
]) {
  requireText(source, marker, `${sourcePath}: missing ${marker}`);
}

const methodStart = source.indexOf("    pub async fn available_locales_for_replies(");
const methodEnd = source.indexOf("    pub(crate) async fn update_with_inline_relations(", methodStart);
if (methodStart < 0 || methodEnd <= methodStart) {
  throw new Error(`${sourcePath}: locale enumeration method boundary is invalid`);
}
const method = source.slice(methodStart, methodEnd);

for (const forbidden of [
  "resolve_by_locale_with_fallback",
  "fallback_locale",
  "requested_locale",
  ".get(tenant_id",
  "find_reply(tenant_id",
  "VoteService",
  "SubscriptionService",
  "TransactionalEventBus",
]) {
  requireAbsent(
    method,
    forbidden,
    `${sourcePath}: locale enumeration must remain exact/batched and side-effect free: ${forbidden}`,
  );
}

const awaitCount = method.split(".await?").length - 1;
if (awaitCount !== 2) {
  throw new Error(
    `${sourcePath}: locale enumeration should keep the bounded two-query shape, found ${awaitCount} awaited owner queries`,
  );
}

for (const marker of [
  "FORUM-34E",
  "34A-34D",
  "canonical Forum ledger still says `FORUM-34` is `planned`",
  "exact stored locale enumeration",
  "MAX_FORUM_REPLY_LOCALE_ENUMERATION_IDS = 512",
  "requires `forum_replies:list`",
  "rejects nil reply IDs and duplicate reply IDs",
  "one tenant-scoped reply existence query",
  "existing batched `load_bodies_map` path",
  "does not call `resolve_by_locale_with_fallback`",
  "does not introduce an N+1 enumeration path",
  "shared or Forum-only import/export runner",
  "no test, Cargo command",
]) {
  requireText(packet, marker, `${packetPath}: missing ${marker}`);
}

console.log("Forum FORUM-34E reply locale enumeration source: ok");
