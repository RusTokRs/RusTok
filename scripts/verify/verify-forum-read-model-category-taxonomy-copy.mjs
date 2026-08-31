#!/usr/bin/env node

import fs from "node:fs";

const failures = [];
const read = (path) => fs.readFileSync(path, "utf8");
const requireMarker = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const rejectMarker = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};

const servicesPath = "crates/rustok-forum/src/services/mod.rs";
const ownerPath = "crates/rustok-forum/src/services/read_model_owner.rs";
const legacyPath = "crates/rustok-forum/src/services/read_model.rs";
for (const path of [servicesPath, ownerPath, legacyPath]) {
  if (!fs.existsSync(path)) failures.push(`${path}: file is required`);
}

if (failures.length === 0) {
  const services = read(servicesPath);
  const owner = read(ownerPath);
  const legacy = read(legacyPath);

  for (const marker of [
    '#[path = "read_model.rs"]',
    "mod read_model_legacy;",
    "mod read_model_owner;",
    "pub use super::read_model_owner::ForumReadModelService;",
  ]) {
    requireMarker(services, marker, servicesPath);
  }

  for (const marker of [
    "TaxonomyOwnerCategoryReader",
    "forum_category_taxonomy_binding::Entity::find()",
    "MAX_FORUM_CATEGORY_TREE_NODES",
    "row.canonical.position",
    "row.canonical.requested_locale",
    "row.canonical.available_locales",
    "row.canonical.icon_key",
    "row.canonical.color",
    ".list_topics(tenant_id, security, query)",
    ".list_topics_with_unread(tenant_id, security, query)",
    ".summarize_topic_ids(tenant_id, security, topic_ids)",
    ".list_replies(tenant_id, security, topic_id, query)",
  ]) {
    requireMarker(owner, marker, ownerPath);
  }

  for (const forbidden of [
    "forum_category_translation",
    "category_translations_by_id",
    "available_locales_from",
    "resolve_by_locale_with_fallback",
  ]) {
    rejectMarker(owner, forbidden, ownerPath);
  }

  for (const marker of [
    "pub async fn list_topics(",
    "pub async fn list_topics_with_unread(",
    "pub async fn summarize_topic_ids(",
    "pub async fn list_replies(",
    "topic_translations_by_id",
    "reply_bodies_by_id",
  ]) {
    requireMarker(legacy, marker, legacyPath);
  }

  for (const forbidden of [
    "pub async fn list_categories(",
    "CategoryCursorPage",
    "CategoryCursorQuery",
    "CategoryReadModel",
    "CATEGORY_CURSOR_VERSION",
    "struct CategoryCursor",
    "encode_category_cursor",
    "decode_category_cursor",
    "forum_category::",
    "forum_category_translation",
    "category_translations_by_id",
    "Resource::ForumCategories",
  ]) {
    rejectMarker(legacy, forbidden, legacyPath);
  }
}

if (failures.length > 0) {
  console.error("[forum-read-model-category-taxonomy-copy] verification failed");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "[forum-read-model-category-taxonomy-copy] Taxonomy Category ownership and Topic/Reply-only legacy delegate verified",
);
