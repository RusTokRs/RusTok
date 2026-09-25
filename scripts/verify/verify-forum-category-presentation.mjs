#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
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

function reject(source, pattern, message) {
  if (pattern.test(source)) failures.push(message);
}

const forumLib = read("crates/modules/rustok-forum/src/lib.rs");
const categoryOwner = read("crates/modules/rustok-forum/src/services/category_projection_owner.rs");
const categoryCommands = read("crates/modules/rustok-forum/src/services/category_command_owner.rs");
const categorySync = read("crates/modules/rustok-forum/src/services/category_taxonomy_sync.rs");
const categoryImport = read("crates/modules/rustok-forum/src/services/category_import.rs");
const categoryLifecycle = read("crates/modules/rustok-forum/src/services/category_lifecycle.rs");
const api = read("crates/modules/rustok-forum/CRATE_API.md");
const plan = read("crates/modules/rustok-forum/docs/implementation-plan.md");
const taxonomyHierarchy = read("crates/modules/rustok-taxonomy/src/owner_category_hierarchy_mutation.rs");
const taxonomyRead = read("crates/modules/rustok-taxonomy/src/owner_category_read.rs");
const taxonomyLib = read("crates/modules/rustok-taxonomy/src/lib.rs");

requireText(forumLib, '["content", "media", "taxonomy"]', "Forum runtime dependency contract is stale");
reject(forumLib, /category_presentation/, "Forum must not register a duplicate Category presentation owner");

requireText(categoryOwner, "taxonomy_sync::load_category_owner_snapshot_in_tx", "Category writes must consume Taxonomy owner projection");
requireText(categoryOwner, "rustok_taxonomy::lock_category_hierarchy_writer_in_tx", "Category write path must use Taxonomy hierarchy serialization");
reject(categoryOwner, /rustok_taxonomy::entities/, "Forum Category projection owner must not access Taxonomy persistence entities");

requireText(categoryCommands, "taxonomy_sync::move_category_in_tx", "Category move must delegate hierarchy mutation to Taxonomy");
requireText(categoryCommands, "taxonomy_sync::reorder_category_siblings_in_tx", "Category reorder must delegate hierarchy mutation to Taxonomy");
reject(categoryCommands, /rustok_taxonomy::entities/, "Forum Category command owner must not access Taxonomy persistence entities");

requireText(categorySync, "rustok_taxonomy::move_module_category_in_tx", "Forum Taxonomy adapter must expose Taxonomy-owned move");
requireText(categorySync, "rustok_taxonomy::shift_module_category_siblings_for_insert_in_tx", "Forum Taxonomy adapter must expose Taxonomy-owned insertion shift");
requireText(categorySync, "TaxonomyOwnerCategoryReader::load_scoped_categories_in_strict", "Forum Taxonomy adapter must expose canonical owner reads");

requireText(categoryImport, "taxonomy_sync::shift_category_siblings_for_insert_in_tx", "Forum import must use Taxonomy-owned sibling insertion");
reject(categoryImport, /rustok_taxonomy::entities/, "Forum Category import must not access Taxonomy persistence entities");

requireText(categoryLifecycle, "TaxonomyOwnerCategoryReader::load_scoped_categories_in_strict", "Forum lifecycle must derive hierarchy from Taxonomy owner read");
reject(categoryLifecycle, /rustok_taxonomy::entities/, "Forum Category lifecycle must not access Taxonomy persistence entities");

requireText(taxonomyHierarchy, "pub async fn move_module_category_in_tx", "Taxonomy must own module Category move");
requireText(taxonomyHierarchy, "pub async fn shift_module_category_siblings_for_insert_in_tx", "Taxonomy must own module Category insertion shift");
requireText(taxonomyRead, "pub async fn load_module_category_sibling_ids_in", "Taxonomy must expose storage-encapsulated Category sibling reads");
requireText(taxonomyLib, "move_module_category_in_tx", "Taxonomy move owner must be exported");
requireText(taxonomyLib, "shift_module_category_siblings_for_insert_in_tx", "Taxonomy insertion owner must be exported");

requireText(api, "### Category presentation ownership", "Forum API contract must describe Taxonomy Category presentation ownership");
reject(api, /CategoryCoverMediaCandidate|resolve_category_cover_for_write|hydrate_category_cover_for_read|normalize_category_icon_key/, "Forum API contract must not describe retired Forum-local Category presentation APIs");

requireText(plan, "Forum-specific Category presentation ownership was superseded", "Forum roadmap must record Category presentation cutover");
reject(plan, /#### Delivered in `FORUM-13A`[\s\S]*CategoryCoverMediaCandidate/, "Forum roadmap must not retain retired Category presentation implementation details");

const retiredPresentation = path.join(repoRoot, "crates/modules/rustok-forum/src/category_presentation.rs");
if (existsSync(retiredPresentation)) failures.push("retired Forum Category presentation source must not exist");

if (failures.length > 0) {
  console.error("forum category presentation ownership verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum category presentation ownership verification passed");
