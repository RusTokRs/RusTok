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
  categoryFacade: "crates/rustok-forum/src/services/category_owner_locale_enumeration.rs",
  topicRaw: "crates/rustok-forum/src/services/topic_locale_enumeration.rs",
  topicFacade: "crates/rustok-forum/src/services/topic_facade_locale_enumeration.rs",
  replyFacade: "crates/rustok-forum/src/services/reply_facade.rs",
  packet: "docs/modules/forum-34-category-topic-locale-enumeration-actualization-2026-08-09.md",
};

const source = Object.fromEntries(Object.entries(files).map(([key, path]) => [key, read(path)]));

for (const marker of [
  'include!("category_owner_locale_enumeration.rs");',
  'include!("topic_locale_enumeration.rs");',
  'include!("topic_facade_locale_enumeration.rs");',
]) need(source.services, marker, "services module graph");
forbid(
  source.services,
  'include!("category_locale_enumeration.rs");',
  "services module graph must retire legacy category locale enumeration",
);

for (const marker of [
  "MAX_FORUM_CATEGORY_LOCALE_ENUMERATION_IDS: usize = 512",
  "pub async fn available_locales_for_categories(",
  "security.is_public_read()",
  "authenticated operator context",
  "Action::Manage",
  "tenant_id.is_nil()",
  "!seen.insert(*category_id)",
  "forum_category::Entity::find()",
  "forum_category_taxonomy_binding::Entity::find()",
  "TaxonomyOwnerCategoryReader::new",
  "TaxonomyScopeType::Module",
  'Some("forum")',
  "projection.available_locales",
  "has no Taxonomy Category binding",
  "Taxonomy Category",
]) need(source.categoryFacade, marker, "category Taxonomy locale API");
for (const marker of [
  "forum_category_translation",
  "load_translations_map_for_categories",
  "available_locales_from(",
  "resolve_by_locale_with_fallback",
  "self.inner.available_locales_for_categories",
]) forbid(source.categoryFacade, marker, "category Taxonomy locale API");

for (const marker of [
  "MAX_FORUM_TOPIC_LOCALE_ENUMERATION_IDS: usize = 512",
  "pub(crate) async fn available_locales_for_topics(",
  "Action::Manage",
  "tenant_id.is_nil()",
  "forum_topic::Entity::find()",
  "load_translations_map_for_topics",
  "available_locales_from(",
  "has no stored locale translation",
]) need(source.topicRaw, marker, "topic raw locale API");
for (const marker of [
  "resolve_by_locale_with_fallback",
  ".get_with_locale_fallback(",
  "VoteService",
  "SubscriptionService",
  "Serialize",
  "Deserialize",
]) forbid(source.topicRaw, marker, "topic raw locale API");

for (const marker of [
  "MAX_FORUM_TOPIC_LOCALE_ENUMERATION_IDS",
  "pub async fn available_locales_for_topics(",
  "security.is_public_read()",
  "authenticated operator context",
  "Action::Manage",
  ".available_locales_for_topics(",
]) need(source.topicFacade, marker, "topic facade locale API");
for (const marker of ["sea_orm", "crate::entities", "::Entity::find()", "Serialize", "Deserialize"]) {
  forbid(source.topicFacade, marker, "topic facade locale API");
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
  "all three current Forum export shapes",
  "no tests, Cargo commands",
]) need(source.packet, marker, "FORUM-34G historical packet");

console.log("Forum category/topic locale enumeration ownership source: ok");
