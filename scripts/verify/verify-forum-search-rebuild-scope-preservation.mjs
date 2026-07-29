#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];

function read(relativePath) {
  const target = path.join(root, relativePath);
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

const activePath = "crates/rustok-search/src/projector.rs";
const legacyPath = "crates/rustok-search/src/projector_legacy.rs";
const libPath = "crates/rustok-search/src/lib.rs";
const ingestionPath = "crates/rustok-search/src/ingestion.rs";
const blogPath = "crates/rustok-search/src/blog_projector.rs";
const forumPath = "crates/rustok-search/src/forum_projector.rs";
const rustTestPath = "crates/rustok-search/tests/search_scope_preservation_contract.rs";
const contractPath = "crates/rustok-forum/contracts/forum-search-rebuild-scope-preservation.json";
const searchContractPath = "crates/rustok-forum/contracts/forum-search-projection.json";
const invalidationPath = "crates/rustok-forum/contracts/forum-projection-invalidation.json";
const visibilityPath = "crates/rustok-forum/contracts/forum-visibility-scoped-bulk-read.json";
const notePath = "crates/rustok-forum/docs/forum-20bm-search-rebuild-scope-preservation.md";

const active = read(activePath);
const legacy = read(legacyPath);
const lib = read(libPath);
const ingestion = read(ingestionPath);
const blog = read(blogPath);
const forum = read(forumPath);
const rustTest = read(rustTestPath);
const note = read(notePath);

let contract = null;
let searchContract = null;
let invalidation = null;
let visibility = null;
for (const [label, source, assign] of [
  [contractPath, read(contractPath), (value) => { contract = value; }],
  [searchContractPath, read(searchContractPath), (value) => { searchContract = value; }],
  [invalidationPath, read(invalidationPath), (value) => { invalidation = value; }],
  [visibilityPath, read(visibilityPath), (value) => { visibility = value; }],
]) {
  try {
    assign(JSON.parse(source));
  } catch (error) {
    failures.push(`${label}: invalid JSON: ${error.message}`);
  }
}

for (const marker of [
  "use crate::projector_legacy;",
  "const CORE_SCOPE_COUNT_SQL",
  "entity_type IN ('node', 'product')",
  "self.legacy.rebuild_content_scope(tenant_id).await?",
  "self.legacy.rebuild_product_scope(tenant_id).await",
  '"rebuild_tenant"',
]) {
  requireMarker(active, marker, activePath);
}
for (const forbidden of [
  "self.legacy.rebuild_tenant",
  '"DELETE FROM search_documents WHERE tenant_id = $1"',
]) {
  rejectMarker(active, forbidden, activePath);
}
const coreCountSection = active.split("const CORE_SCOPE_COUNT_SQL")[1] ?? "";
const coreCountSql = coreCountSection.split('"#;')[0] ?? "";
for (const forbidden of ["blog_post", "forum_category", "forum_topic"]) {
  rejectMarker(coreCountSql, forbidden, activePath);
}

for (const marker of [
  "pub struct SearchProjector",
  "pub async fn rebuild_tenant",
  '"DELETE FROM search_documents WHERE tenant_id = $1"',
]) {
  requireMarker(legacy, marker, legacyPath);
}
for (const marker of [
  "#[allow(dead_code)]",
  '#[path = "projector_legacy.rs"]',
  "mod projector_legacy;",
  "pub mod projector;",
  "pub use projector::SearchProjector;",
]) {
  requireMarker(lib, marker, libPath);
}
rejectMarker(lib, "pub mod projector_legacy;", libPath);
rejectMarker(lib, "pub use projector_legacy", libPath);

const rebuildSection = ingestion.split("async fn rebuild_tenant")[1] ?? "";
const rebuild = rebuildSection.split("async fn handle_reindex_request")[0] ?? "";
const coreIndex = rebuild.indexOf("self.projector.rebuild_tenant");
const blogIndex = rebuild.indexOf("self.blog_projector.rebuild_tenant");
const forumIndex = rebuild.lastIndexOf("projector.rebuild_tenant");
if (!(coreIndex >= 0 && blogIndex > coreIndex && forumIndex > blogIndex)) {
  failures.push(`${ingestionPath}: full rebuild order must remain core -> Blog -> Forum`);
}

for (const marker of [
  "let tx = self.begin_transaction().await?",
  "self.delete_tenant_documents_in(&tx, tenant_id).await?",
  "self.commit_transaction(tx).await",
]) {
  requireMarker(blog, marker, blogPath);
}
for (const marker of [
  "let tx = self.db.begin().await.map_err(Error::Database)?",
  "self.create_stage(&tx).await?",
  "delete_forum_scope(&tx, tenant_id).await?",
  "tx.commit().await.map_err(Error::Database)",
]) {
  requireMarker(forum, marker, forumPath);
}
for (const marker of [
  "active_tenant_rebuild_never_calls_the_destructive_legacy_tenant_rebuild",
  "full_ingestion_rebuild_keeps_source_order_and_atomic_external_replacements",
  "bootstrap_presence_check_ignores_external_only_documents",
  ".rfind(\"projector.rebuild_tenant\")",
]) {
  requireMarker(rustTest, marker, rustTestPath);
}
for (const marker of [
  "FORUM-20BM",
  "source-scoped preservation",
  "not a new cross-source transaction",
  "previous Forum documents remain",
  "Global all-source atomicity",
  "FORUM-20BN",
]) {
  requireMarker(note, marker, notePath);
}

if (contract) {
  if (contract.task !== "FORUM-20BM") failures.push(`${contractPath}: unexpected task`);
  if (contract.upstream_task !== "FORUM-20BL") {
    failures.push(`${contractPath}: unexpected upstream task`);
  }
  if (contract.downstream_task !== "FORUM-20BN") {
    failures.push(`${contractPath}: unexpected downstream task`);
  }
  for (const key of [
    "public_search_projector_is_scope_preserving_facade",
    "legacy_projector_module_is_private",
  ]) {
    if (contract.ownership_boundary?.[key] !== true) {
      failures.push(`${contractPath}: ownership boundary ${key} drift`);
    }
  }
  for (const key of [
    "legacy_projector_reexported",
    "active_tenant_rebuild_calls_legacy_all_tenant_delete",
    "core_rebuild_deletes_blog_scope",
    "core_rebuild_deletes_forum_scope",
    "core_rebuild_deletes_unknown_future_external_scope",
  ]) {
    if (contract.ownership_boundary?.[key] !== false) {
      failures.push(`${contractPath}: ownership boundary ${key} must remain false`);
    }
  }
  for (const key of [
    "content_scope_replacement_transactional",
    "product_scope_replacement_transactional",
    "blog_scope_replacement_transactional",
    "forum_scope_replacement_uses_temporary_stage",
    "forum_scope_replacement_transactional",
    "failed_blog_rebuild_keeps_previous_blog_scope",
    "failed_forum_rebuild_keeps_previous_forum_scope",
    "later_source_failure_cannot_remove_untouched_external_scope",
    "earlier_successful_scope_may_commit_before_later_failure",
  ]) {
    if (contract.replacement_boundary?.[key] !== true) {
      failures.push(`${contractPath}: replacement boundary ${key} drift`);
    }
  }
  for (const key of [
    "global_cross_source_transaction_added",
    "global_cross_source_atomicity_claimed",
  ]) {
    if (contract.replacement_boundary?.[key] !== false) {
      failures.push(`${contractPath}: replacement boundary ${key} must remain false`);
    }
  }
  if (contract.bootstrap_boundary?.external_only_documents_suppress_core_bootstrap !== false) {
    failures.push(`${contractPath}: external-only documents must not suppress core bootstrap`);
  }
}

for (const [label, upstream] of [
  [searchContractPath, searchContract],
  [invalidationPath, invalidation],
  [visibilityPath, visibility],
]) {
  if (!upstream) continue;
  if (upstream.rebuild_scope_preservation_contract !== contractPath) {
    failures.push(`${label}: rebuild preservation contract handoff drift`);
  }
  if (upstream.downstream_task !== "FORUM-20BN") {
    failures.push(`${label}: downstream task must advance to FORUM-20BN`);
  }
}
if (searchContract) {
  if (searchContract.persistence_boundary?.full_search_rebuild_source_failure_keeps_previous_forum_scope !== true) {
    failures.push(`${searchContractPath}: previous Forum scope must survive full rebuild source failure`);
  }
  if (searchContract.persistence_boundary?.direct_search_rebuild_deletes_external_scopes !== false) {
    failures.push(`${searchContractPath}: direct Search rebuild must not delete external scopes`);
  }
  if (searchContract.persistence_boundary?.global_cross_source_atomicity_added !== false) {
    failures.push(`${searchContractPath}: global atomicity must remain explicitly unclaimed`);
  }
}

if (failures.length > 0) {
  console.error("forum Search rebuild scope preservation verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum Search rebuild scope preservation verified");
