#!/usr/bin/env node

/**
 * Blog Search projection is owner-produced.
 *
 * The documents are built by `rustok-blog`'s `SearchProjectionSource` and stored by
 * Search's neutral `SearchProjectionDriver`. Search no longer reads the Blog tables
 * itself, so this guard pins the *owner* invariants that used to be pinned as SQL in
 * `blog_projector.rs`:
 *
 *   - tags come from `blog_post_tags` joined to Taxonomy, never from
 *     `blog_posts.metadata.tags`;
 *   - the author name comes from the Profiles owner port, never from a `users` join;
 *   - the indexed body is the article plain text, not the excerpt;
 *   - only the public corpus is projected: `published` **and** stamped.
 *
 * Search side invariants stay too: the ingestion path must reach Blog only through the
 * neutral driver helpers, and the retired private projector must not come back.
 *
 * Run: `node scripts/verify/verify-search-blog-projection.mjs`
 */

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function read(relativePath) {
  const target = repoPath(relativePath);
  if (!existsSync(target)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(target, "utf8");
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function rejectMarker(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

const ownerSourcePath = "crates/modules/rustok-blog/src/services/search_projection.rs";
const driverPath = "crates/modules/rustok-search/src/projection_driver.rs";
const ingestionPath = "crates/modules/rustok-search/src/ingestion.rs";
const routingTestPath =
  "crates/modules/rustok-search/tests/blog_ingestion_contract_test.rs";
const postgresTestPath =
  "crates/modules/rustok-search/tests/blog_projection_postgres_test.rs";
const evidencePath =
  "crates/modules/rustok-search/contracts/evidence/search-blog-projection-postgres-harness.json";
const planPath = "crates/modules/rustok-search/docs/implementation-plan.md";

const ownerSource = read(ownerSourcePath);
const driver = read(driverPath);
const ingestion = read(ingestionPath);
const routingTest = read(routingTestPath);
const postgresTest = read(postgresTestPath);
const plan = read(planPath);
let evidence = null;
try {
  evidence = JSON.parse(read(evidencePath));
} catch (error) {
  failures.push(`${evidencePath}: invalid JSON: ${error.message}`);
}

// --- Blog owner source -----------------------------------------------------------
for (const marker of [
  "pub struct BlogSearchProjectionSourceFactory",
  "fn build_with_extensions(",
  ".get::<Arc<dyn ProfileSummaryReader>>()",
  "find_profile_summaries(",
  "ProfileSummaryAudience::Anonymous",
  "Blog Search projection has no Profiles owner port; public author names are omitted",
  "load_post_tags_map(",
  "load_category_projections_map(",
  "crate::richtext::project_stored_article(",
  'blog_post::Column::Status.eq("published")',
  "blog_post::Column::PublishedAt.is_not_null()",
  "if entity_type != BLOG_POST_ENTITY_TYPE",
  "MAX_SEARCH_PROJECTION_PAGE_SIZE",
  'format!("blog_post:{}:{}", post.id, translation.locale)',
]) {
  requireMarker(ownerSource, marker, ownerSourcePath);
}
for (const marker of [
  "search_documents",
  "INSERT INTO",
  "DELETE FROM",
  "to_regclass(",
  "LEFT JOIN users",
  "FROM users",
  "jsonb_array_elements_text",
  "p.metadata -> 'tags'",
  "metadata.get(\"tags\")",
  "UPDATE search_documents",
]) {
  rejectMarker(ownerSource, marker, ownerSourcePath);
}

// --- Search neutral driver -------------------------------------------------------
for (const marker of [
  "pub(crate) async fn rebuild_tenant(",
  "pub(crate) async fn refresh_entity(",
  "pub(crate) async fn delete_entity(",
  "pub(crate) async fn delete_tenant(",
  "DELETE FROM search_documents WHERE tenant_id = $1 AND source_module = $2",
  "self.source.source_module().into()",
]) {
  requireMarker(driver, marker, driverPath);
}

// --- Search ingestion routes Blog through the driver -----------------------------
for (const marker of [
  "DomainEvent::BlogPostCreated",
  "DomainEvent::BlogPostPublished",
  "DomainEvent::BlogPostUnpublished",
  "DomainEvent::BlogPostUpdated",
  "DomainEvent::BlogPostArchived",
  "DomainEvent::BlogPostDeleted",
  "DomainEvent::TenantModuleToggled",
  'module_slug == "blog"',
  "self.blog_driver",
  "refresh_owner_entity(",
  "delete_owner_entity(",
  "toggle_owner_scope(",
  "refresh_blog_author_scope(",
  '"blog_post"',
  '"delete_blog_scope"',
  '"rebuild_blog_scope"',
]) {
  requireMarker(ingestion, marker, ingestionPath);
}
for (const marker of [
  "blog_projector",
  "BlogSearchProjector",
  "handle_blog_module_toggle",
  "refresh_author_projection",
]) {
  rejectMarker(ingestion, marker, ingestionPath);
}

// --- Event routing contract ------------------------------------------------------
for (const marker of [
  "DomainEvent::BlogPostCreated",
  "DomainEvent::BlogPostPublished",
  "DomainEvent::BlogPostUnpublished",
  "DomainEvent::BlogPostUpdated",
  "DomainEvent::BlogPostArchived",
  "DomainEvent::BlogPostDeleted",
  "DomainEvent::TenantModuleToggled",
  'target_type: "blog".to_string()',
]) {
  requireMarker(routingTest, marker, routingTestPath);
}

// --- PostgreSQL lifecycle harness ------------------------------------------------
for (const marker of [
  "RUSTOK_SEARCH_TEST_DATABASE_URL",
  "SearchModule.migrations()",
  "SearchIngestionHandler::with_all_projection_sources",
  "ContractEventEnvelope::new",
  'SET search_path TO "{schema_name}", public',
  "options=-c%20search_path%3D",
  "full_blog_reindex_replaces_only_current_tenant_blog_documents",
  "blog_events_upsert_publish_archive_and_delete_search_document",
  "blog_reindex_projects_attached_taxonomy_tags_not_metadata_snapshot",
  "blog_author_profile_change_rebuilds_the_public_corpus",
  "blog_projection_uses_the_profiles_port_not_the_account_row",
  "blog_projection_degrades_without_the_profiles_port",
  "stale-metadata-only",
  "Published body",
  "CREATE TABLE taxonomy_terms",
  "CREATE TABLE taxonomy_term_translations",
  "CREATE TABLE blog_post_tags",
  "blog_module_disable_cleans_scope_and_enable_rebuilds_it",
  "targeted_reindex_removes_stale_document_when_source_post_is_missing",
]) {
  requireMarker(postgresTest, marker, postgresTestPath);
}
for (const marker of ["LEFT JOIN users", "users u"]) {
  rejectMarker(postgresTest, marker, postgresTestPath);
}

if (evidence) {
  if (evidence.schema_version !== 1) failures.push(`${evidencePath}: schema_version must be 1`);
  if (evidence.module !== "search" || evidence.surface !== "blog_post_projection") {
    failures.push(`${evidencePath}: module/surface identity drift`);
  }
  if (evidence.status !== "executable_no_run" || evidence.compile_policy !== "not_run_by_request") {
    failures.push(`${evidencePath}: execution status drift`);
  }
  const targets = evidence.test_targets ?? [];
  for (const target of [routingTestPath, postgresTestPath]) {
    if (!targets.includes(target)) failures.push(`${evidencePath}: missing test target ${target}`);
  }
  const contract = evidence.production_contract ?? {};
  if (contract.event_handler !== ingestionPath) {
    failures.push(`${evidencePath}: event_handler must point at the Search ingestion owner`);
  }
  if (
    contract.projector !== ownerSourcePath ||
    contract.projection_driver !== driverPath
  ) {
    failures.push(`${evidencePath}: the projection owner moved; evidence paths are stale`);
  }
  if (
    contract.blog_tag_attachment_source !== "blog_post_tags" ||
    contract.tag_dictionary_source !== "taxonomy_terms + taxonomy_term_translations" ||
    contract.blog_tag_relation_tenant_constrained !== true ||
    contract.legacy_metadata_tags_are_projection_source !== false
  ) {
    failures.push(`${evidencePath}: canonical Blog tag projection source drift`);
  }
  if (
    contract.blog_author_source !== "rustok-profiles ProfileSummaryReader port" ||
    contract.blog_author_account_table_join !== false ||
    contract.blog_author_audience !== "anonymous public summary"
  ) {
    failures.push(`${evidencePath}: Blog author projection source drift`);
  }
  if (contract.public_corpus_status !== "published and stamped") {
    failures.push(`${evidencePath}: the indexed corpus must stay the public Blog corpus`);
  }
  const caseNames = new Set((evidence.cases ?? []).map((item) => item.name));
  for (const caseName of [
    "module_toggle_cleanup_rebuild",
    "targeted_missing_post_cleanup",
    "canonical_taxonomy_tag_projection",
    "author_profile_refresh",
    "profiles_port_absent_degrades",
  ]) {
    if (!caseNames.has(caseName)) failures.push(`${evidencePath}: missing case ${caseName}`);
  }
}

for (const marker of [
  "search-blog-projection-postgres-harness.json",
  "RUSTOK_SEARCH_TEST_DATABASE_URL",
  "search_path",
  "module-disabled cleanup",
  "targeted missing-post",
]) {
  requireMarker(plan, marker, planPath);
}

if (failures.length > 0) {
  console.error("search Blog projection verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("search Blog projection verification passed");
