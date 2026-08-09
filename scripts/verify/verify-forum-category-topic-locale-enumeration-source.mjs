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

const files = {
  services: "crates/rustok-forum/src/services/mod.rs",
  categoryRaw: "crates/rustok-forum/src/services/category_locale_enumeration.rs",
  categoryFacade: "crates/rustok-forum/src/services/category_owner_locale_enumeration.rs",
  topicRaw: "crates/rustok-forum/src/services/topic_locale_enumeration.rs",
  topicFacade: "crates/rustok-forum/src/services/topic_facade_locale_enumeration.rs",
  replyFacade: "crates/rustok-forum/src/services/reply_facade.rs",
  packet: "docs/modules/forum-34-category-topic-locale-enumeration-actualization-2026-08-09.md",
};

const source = Object.fromEntries(Object.entries(files).map(([key, path]) => [key, read(path)]));

for (const marker of [
  'include!("category_locale_enumeration.rs");',
  'include!("category_owner_locale_enumeration.rs");',
  'include!("topic_locale_enumeration.rs");',
  'include!("topic_facade_locale_enumeration.rs");',
]) need(source.services, marker, "services module graph");

for (const [label, text, kind, entity, translationLoader] of [
  ["category raw", source.categoryRaw, "CATEGORY", "forum_category", "load_translations_map_for_categories"],
  ["topic raw", source.topicRaw, "TOPIC", "forum_topic", "load_translations_map_for_topics"],
]) {
  for (const marker of [
    `MAX_FORUM_${kind}_LOCALE_ENUMERATION_IDS: usize = 512`,
    "pub(crate) async fn available_locales_for_",
    "Action::Manage",
    "tenant_id.is_nil()",
    ".is_nil()",
    "!seen.insert(*",
    `${entity}::Entity::find()`,
    `${entity}::Column::TenantId.eq(tenant_id)`,
    `${entity}::Column::Id.is_in(",
    translationLoader,
    "available_locales_from(",
    "has no stored locale translation",
    "result.push((*",
  ]) need(text, marker, label);

  for (const marker of [
    "resolve_by_locale_with_fallback",
    ".get_with_locale_fallback(",
    ".get(",
    "VoteService",
    "SubscriptionService",
    "TaxonomyService",
    "Serialize",
    "Deserialize",
  ]) forbid(text, marker, label);

  const existenceQueries = text.split("::Entity::find()").length - 1;
  if (existenceQueries !== 1) {
    throw new Error(`${label}: expected one direct existence query, found ${existenceQueries}`);
  }
}

for (const [label, text, kind, method] of [
  ["category facade", source.categoryFacade, "CATEGORY", "available_locales_for_categories"],
  ["topic facade", source.topicFacade, "TOPIC", "available_locales_for_topics"],
]) {
  for (const marker of [
    `MAX_FORUM_${kind}_LOCALE_ENUMERATION_IDS`,
    `pub async fn ${method}(`,
    "security.is_public_read()",
    "authenticated operator context",
    "Action::Manage",
    `.${method}(`,
  ]) need(text, marker, label);
  for (const marker of ["sea_orm", "crate::entities", "::Entity::find()", "Serialize", "Deserialize"]) {
    forbid(text, marker, label);
  }
}

for (const marker of [
  "pub async fn available_locales_for_replies(",
  "MAX_FORUM_REPLY_LOCALE_ENUMERATION_IDS",
  "Action::Manage",
]) need(source.replyFacade, marker, "reply parity baseline");

for (const marker of [
  "FORUM-34G",
  "FORUM-34A through FORUM-34F",
  "effective `Manage` permission satisfies narrower actions",
  "still labels `FORUM-34` as `planned`",
  "more than 512 IDs",
  "one tenant-scoped bounded existence query",
  "one existing batched translation loader",
  "does not claim that an archived/deleted/otherwise non-presentable owner row is export-readable",
  "no per-ID owner `get`",
  "all three current Forum export shapes",
  "next safe Forum-owned slice can compose",
  "no tests, Cargo commands",
]) need(source.packet, marker, "FORUM-34G packet");

console.log("Forum FORUM-34G category/topic locale enumeration source: ok");
