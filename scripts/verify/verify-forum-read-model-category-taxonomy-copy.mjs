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
    "self.legacy.list_topics",
    "self.legacy.list_topics_with_unread",
    "self.legacy.summarize_topic_ids",
    "self.legacy",
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

  requireMarker(
    legacy,
    "category_translations_by_id",
    "private legacy delegate remains available for non-Category methods during bounded cutover",
  );
}

if (failures.length > 0) {
  console.error("[forum-read-model-category-taxonomy-copy] verification failed");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("[forum-read-model-category-taxonomy-copy] Taxonomy Category ownership verified");
