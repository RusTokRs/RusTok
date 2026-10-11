#!/usr/bin/env node

// Forum read-model / Taxonomy Category ownership guard.
//
// The Forum read model renders category presentation, but the Taxonomy owner owns category
// identity, hierarchy and localized copy. This guard therefore pins three things:
//
//   1. Category copy is resolved through the Taxonomy owner's reader port, and the retired
//      forum-side category storage/translation reads stay absent.
//   2. Locale fallback comes from the shared Content helper, never from a Forum re-implementation.
//   3. Every read entry point authorizes before touching data, and the public `services::read_model`
//      path re-exports the owner service instead of a shim.
//
// The 2026-10-11 rewrite replaced the mid-cutover expectations: the guard used to require
// `services/read_model.rs` (a `#[path]`-aliased legacy delegate) and demanded that the owner query
// `forum_category_taxonomy_binding::Entity::find()` directly, while forbidding the shared locale
// helpers the owner legitimately uses for topic and reply translations. That delegate was retired
// once the owner became the single implementation, so requiring its file had made this guard
// decorative (and red in CI); it now asserts that the retired path stays retired.

import fs from "node:fs";

const failures = [];
const read = (path) => fs.readFileSync(path, "utf8");
const requireMarker = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const rejectMarker = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};

const servicesPath = "crates/modules/rustok-forum/src/services/mod.rs";
const ownerPath = "crates/modules/rustok-forum/src/services/read_model_owner.rs";
const retiredLegacyPath = "crates/modules/rustok-forum/src/services/read_model.rs";

for (const path of [servicesPath, ownerPath]) {
  if (!fs.existsSync(path)) failures.push(`${path}: file is required`);
}

// The legacy delegate and its `#[path]` alias are superseded: re-adding either would restore a
// second implementation of the same read model behind the public module path.
if (fs.existsSync(retiredLegacyPath)) {
  failures.push(`${retiredLegacyPath}: retired legacy delegate must stay removed`);
}

if (failures.length === 0) {
  const services = read(servicesPath);
  const owner = read(ownerPath);

  for (const marker of ["pub use super::read_model_owner::ForumReadModelService;"]) {
    requireMarker(services, marker, servicesPath);
  }
  for (const forbidden of ["mod read_model_legacy;", '#[path = "read_model.rs"]']) {
    rejectMarker(services, forbidden, servicesPath);
  }

  // Category presentation data is read through the Taxonomy owner port, bounded by the tree limit.
  for (const marker of [
    "TaxonomyOwnerCategoryReader::new(",
    "MAX_FORUM_CATEGORY_TREE_NODES",
    "row.canonical.position",
    "row.canonical.requested_locale",
    "row.canonical.available_locales",
    "row.canonical.icon_key",
    "row.canonical.color",
  ]) {
    requireMarker(owner, marker, ownerPath);
  }

  // Forum must not re-read taxonomy-owned storage and must not hand-roll locale fallback: the
  // shared Content helper is imported, and the category binding/translation reads belong to the
  // Taxonomy owner. Forum's own category cursor read model (`CategoryCursorPage`,
  // `encode_category_cursor`, …) is deliberately not restricted here — it is Forum-owned.
  for (const forbidden of [
    "forum_category_taxonomy_binding::Entity::find()",
    "forum_category_translation",
    "category_translations_by_id",
    "fn resolve_by_locale_with_fallback",
    "fn available_locales_from",
  ]) {
    rejectMarker(owner, forbidden, ownerPath);
  }

  // The Topic/Reply surface the retired delegate used to expose stays on the owner implementation.
  for (const marker of [
    "pub async fn list_topics(",
    "pub async fn list_topics_with_unread(",
    "pub async fn summarize_topic_ids(",
    "pub async fn list_replies(",
    "topic_translations_by_id",
    "reply_bodies_by_id",
    "use rustok_content::",
  ]) {
    requireMarker(owner, marker, ownerPath);
  }

  // No read path may skip authorization: each entry point scopes its own resource and action.
  for (const marker of [
    "enforce_scope(&security, Resource::ForumCategories, Action::List)?",
    "enforce_scope(&security, Resource::ForumTopics, Action::List)?",
    "enforce_scope(&security, Resource::ForumReplies, Action::List)?",
  ]) {
    requireMarker(owner, marker, ownerPath);
  }
}

if (failures.length > 0) {
  console.error("[forum-read-model-category-taxonomy-copy] verification failed");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "[forum-read-model-category-taxonomy-copy] Taxonomy Category ownership and single owner-backed read model verified",
);
