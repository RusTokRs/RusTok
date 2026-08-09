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

const storagePath = "crates/rustok-forum/src/services/reply_inline.rs";
const ownerPath = "crates/rustok-forum/src/services/reply_owner_inline.rs";
const facadePath = "crates/rustok-forum/src/services/reply_facade.rs";
const packetPath = "docs/modules/forum-34-reply-locale-enumeration-actualization-2026-08-09.md";

const storage = read(storagePath);
const owner = read(ownerPath);
const facade = read(facadePath);
const packet = read(packetPath);

for (const marker of [
  "pub(crate) const MAX_FORUM_REPLY_LOCALE_ENUMERATION_IDS: usize = 512;",
  "pub(crate) async fn available_locales_for_replies(",
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
  requireText(storage, marker, `${storagePath}: missing ${marker}`);
}

const storageMethodStart = storage.indexOf(
  "    pub(crate) async fn available_locales_for_replies(",
);
const storageMethodEnd = storage.indexOf(
  "    pub(crate) async fn update_with_inline_relations(",
  storageMethodStart,
);
if (storageMethodStart < 0 || storageMethodEnd <= storageMethodStart) {
  throw new Error(`${storagePath}: locale enumeration method boundary is invalid`);
}
const storageMethod = storage.slice(storageMethodStart, storageMethodEnd);

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
    storageMethod,
    forbidden,
    `${storagePath}: locale enumeration must remain exact/batched and side-effect free: ${forbidden}`,
  );
}

const awaitCount = storageMethod.split(".await?").length - 1;
if (awaitCount !== 2) {
  throw new Error(
    `${storagePath}: locale enumeration should keep the bounded two-query shape, found ${awaitCount} awaited owner queries`,
  );
}

for (const marker of [
  "pub(crate) const MAX_FORUM_REPLY_LOCALE_ENUMERATION_IDS: usize =",
  "reply::ReplyService::MAX_FORUM_REPLY_LOCALE_ENUMERATION_IDS;",
  "pub(crate) async fn available_locales_for_replies(",
  ".available_locales_for_replies(tenant_id, security, reply_ids)",
]) {
  requireText(owner, marker, `${ownerPath}: missing owner delegation marker ${marker}`);
}

for (const marker of [
  "pub const MAX_FORUM_REPLY_LOCALE_ENUMERATION_IDS: usize =",
  "reply_owner::ReplyService::MAX_FORUM_REPLY_LOCALE_ENUMERATION_IDS;",
  "pub async fn available_locales_for_replies(",
  "enforce_scope(&security, Resource::ForumReplies, Action::List)?;",
  "if security.is_public_read()",
  "Forum reply locale enumeration requires an authenticated operator context",
  ".available_locales_for_replies(tenant_id, security, reply_ids)",
]) {
  requireText(facade, marker, `${facadePath}: missing public owner marker ${marker}`);
}

const facadeMethodStart = facade.indexOf("    pub async fn available_locales_for_replies(");
const facadeMethodEnd = facade.indexOf("    pub async fn create(", facadeMethodStart);
if (facadeMethodStart < 0 || facadeMethodEnd <= facadeMethodStart) {
  throw new Error(`${facadePath}: public locale enumeration method boundary is invalid`);
}
const facadeMethod = facade.slice(facadeMethodStart, facadeMethodEnd);
for (const forbidden of [
  "topic_category_is_visible",
  "for reply_id in reply_ids",
  "find_reply(",
  "forum_reply::Entity",
  "forum_reply_body::Entity",
]) {
  requireAbsent(
    facadeMethod,
    forbidden,
    `${facadePath}: public facade must reject public-read and delegate without per-reply probes: ${forbidden}`,
  );
}

for (const marker of [
  "FORUM-34E",
  "34A-34D",
  "canonical Forum ledger still says `FORUM-34` is `planned`",
  "exact stored locale enumeration",
  "MAX_FORUM_REPLY_LOCALE_ENUMERATION_IDS = 512",
  "requires `forum_replies:list`",
  "rejects `SecurityContext::is_public_read()`",
  "anonymous reply-existence or locale oracle",
  "raw batched storage method and its bound remain crate-private",
  "one tenant-scoped reply existence query",
  "existing batched `load_bodies_map` loader",
  "does not call `resolve_by_locale_with_fallback`",
  "does not introduce an N+1 enumeration path",
  "shared or Forum-only import/export runner",
  "no test, Cargo command",
]) {
  requireText(packet, marker, `${packetPath}: missing ${marker}`);
}

console.log("Forum FORUM-34E reply locale enumeration source: ok");
