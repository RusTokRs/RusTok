#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const failures = [];
const files = {
  evidence: 'crates/rustok-blog/contracts/evidence/blog-tag-mutation-reindex-source.json',
  taxonomyMutation: 'crates/rustok-taxonomy/src/module_term_mutation.rs',
  taxonomyLib: 'crates/rustok-taxonomy/src/lib.rs',
  tagService: 'crates/rustok-blog/src/services/tag.rs',
  relationMigration: 'crates/rustok-blog/src/migrations/m20260328_000002_create_blog_taxonomy_tables.rs',
  harness: 'crates/rustok-blog/tests/taxonomy_tags.rs',
  slice: 'crates/rustok-blog/docs/implementation-plan-slice-104.md',
  current: 'crates/rustok-blog/docs/implementation-plan-current.md',
};
function read(relativePath) {
  const target = path.join(repoRoot, relativePath);
  if (!fs.existsSync(target)) { failures.push(`${relativePath}: missing file`); return ''; }
  return fs.readFileSync(target, 'utf8');
}
function json(relativePath) {
  try { return JSON.parse(read(relativePath)); }
  catch (error) { failures.push(`${relativePath}: invalid JSON: ${error.message}`); return null; }
}
function need(source, marker, label) { if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`); }
function forbid(source, marker, label) { if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`); }
function between(source, startMarker, endMarker) {
  const start = source.indexOf(startMarker);
  if (start < 0) return '';
  const end = source.indexOf(endMarker, start + startMarker.length);
  return source.slice(start, end < 0 ? source.length : end);
}

const evidence = json(files.evidence);
const taxonomyMutation = read(files.taxonomyMutation);
const taxonomyLib = read(files.taxonomyLib);
const tagService = read(files.tagService);
const relationMigration = read(files.relationMigration);
const harness = read(files.harness);
const slice = read(files.slice);
const current = read(files.current);

if (evidence) {
  if (
    evidence.schema_version !== 1 || evidence.module !== 'blog' ||
    evidence.surface !== 'tag_mutation_atomic_reindex' ||
    evidence.status !== 'source_verified_no_compile' ||
    evidence.compile_policy !== 'not_run_by_request' || evidence.runtime_status !== 'not_run'
  ) failures.push(`${files.evidence}: identity/status drift`);
  const contract = evidence.production_contract ?? {};
  for (const [key, expected] of Object.entries({
    update_uses_supplied_transaction: true,
    delete_uses_supplied_transaction: true,
    taxonomy_update_permission_preserved: true,
    taxonomy_read_permission_preserved_for_update_response: true,
    taxonomy_delete_permission_preserved: true,
    module_scope_rechecked_by_taxonomy_owner: true,
    term_kind_rechecked_by_taxonomy_owner: true,
    translation_revision_cas_preserved: true,
    term_revision_cas_preserved: true,
    translation_change_evidence_same_transaction: true,
    blog_reindex_same_transaction: true,
    manual_predelete_relation_cleanup_removed: true,
    relation_cleanup_uses_declared_fk_cascade: true,
    tag_service_constructor_changed: false,
    database_schema_changed: false,
    search_projection_source_changed: false,
    ffa_promoted: false,
    fba_promoted: false,
  })) {
    if (contract[key] !== expected) failures.push(`${files.evidence}: ${key} drift`);
  }
  if (contract.blog_reindex_target_type !== 'blog' || contract.blog_reindex_target_id !== null) {
    failures.push(`${files.evidence}: Blog reindex target drift`);
  }
  if (
    evidence.source_harness?.path !== files.harness ||
    evidence.source_harness?.status !== 'executable_no_run' ||
    evidence.source_harness?.runtime_status !== 'not_run' ||
    !Array.isArray(evidence.execution) || evidence.execution.length !== 0 ||
    evidence.planning_result?.tag_mutation_source_complete !== true ||
    evidence.planning_result?.additional_tag_mutation_scaffolding_required !== false ||
    evidence.planning_result?.next_autonomous_source_work_requires_fresh_audit !== true
  ) failures.push(`${files.evidence}: harness/planning/execution drift`);
}

for (const marker of [
  'pub async fn update_module_term_in_tx(',
  'pub async fn delete_module_term_in_tx(',
  'enforce_scope(security, Resource::Taxonomy, Action::Update)?;',
  'enforce_scope(security, Resource::Taxonomy, Action::Read)?;',
  'enforce_scope(security, Resource::Taxonomy, Action::Delete)?;',
  'TaxonomyScopeType::Module',
  'taxonomy_term::Column::ScopeValue.eq(&module_scope)',
  'taxonomy_term::Column::Kind.eq(kind)',
  'taxonomy_term_translation::Column::Revision.eq(existing.revision)',
  'taxonomy_term::Column::Revision.eq(term.revision)',
  'record_translation_change_in_tx(',
]) need(taxonomyMutation, marker, files.taxonomyMutation);
for (const marker of [
  'pub mod module_term_mutation;',
  'delete_module_term_in_tx',
  'update_module_term_in_tx',
]) need(taxonomyLib, marker, files.taxonomyLib);

const updateSection = between(tagService, 'pub async fn update_tag(', 'pub async fn delete_tag(');
const deleteSection = between(tagService, 'pub async fn delete_tag(', 'pub async fn list_tags(');
for (const marker of [
  'self.db.begin()',
  'update_module_term_in_tx(',
  'publish_blog_reindex_in_tx(&txn, tenant_id, security.user_id)',
  'txn.commit()',
]) need(updateSection, marker, `${files.tagService} update_tag`);
for (const marker of [
  'self.db.begin()',
  'delete_module_term_in_tx(',
  'publish_blog_reindex_in_tx(&txn, tenant_id, security.user_id)',
  'txn.commit()',
]) need(deleteSection, marker, `${files.tagService} delete_tag`);
forbid(deleteSection, 'blog_post_tag::Entity::delete_many()', `${files.tagService} delete_tag`);
for (const marker of [
  'TransactionalEventBus::publish_root_in_tx(',
  'DomainEvent::ReindexRequested {',
  'target_type: "blog".to_string()',
  'target_id: None',
]) need(tagService, marker, files.tagService);
need(tagService, 'pub fn new(db: DatabaseConnection) -> Self', files.tagService);

for (const marker of [
  '.name("fk_blog_post_tags_tag")',
  '.from(BlogPostTags::Table, BlogPostTags::TagId)',
  '.to(TaxonomyTerms::Table, TaxonomyTerms::Id)',
  '.on_delete(ForeignKeyAction::Cascade)',
]) need(relationMigration, marker, files.relationMigration);

for (const marker of [
  'SysEventsMigration',
  'tag_update_commits_dictionary_change_and_blog_reindex_together',
  'tag_update_rolls_back_when_blog_reindex_outbox_write_fails',
  'DROP TABLE sys_events',
  'assert_eq!(translation.name, "rust")',
  'assert_eq!(term.revision, 1)',
  'tag_delete_relies_on_taxonomy_fk_cascade_and_retains_reindex',
  'DomainEvent::ReindexRequested',
]) need(harness, marker, files.harness);

for (const marker of [
  'Status: `tag_mutation_atomic_reindex_source_ready_maintainer_execution_pending`.',
  '`taxonomy_module_term_mutation = owner_supplied_transaction_source_ready`',
  '`tag_mutation_atomic_reindex = source_ready_maintainer_execution_pending`',
  '`tag_delete_relation_cleanup = declared_fk_cascade`',
  'These cases are source evidence only. They were not executed',
  'Do not add another tag mutation scaffolding slice without new evidence.',
]) need(slice, marker, files.slice);
for (const marker of [
  '`tag_mutation_atomic_reindex = source_ready_maintainer_execution_pending`',
  'source-complete through slice 104',
]) need(current, marker, files.current);

if (failures.length) {
  console.error('[verify-blog-tag-mutation-reindex-source] FAIL');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}
console.log('[verify-blog-tag-mutation-reindex-source] PASS atomic=taxonomy+reindex delete=fk-cascade execution=not-run');
