import fs from 'node:fs';
import path from 'node:path';

function read(file) {
  return fs.readFileSync(file, 'utf8');
}

function requireAll(file, snippets) {
  const content = read(file);
  for (const snippet of snippets) {
    if (!content.includes(snippet)) {
      throw new Error(`${file} is missing required CAT-4 contract: ${snippet}`);
    }
  }
  return content;
}

function rustFiles(root) {
  const files = [];
  for (const entry of fs.readdirSync(root, { withFileTypes: true })) {
    const child = path.join(root, entry.name);
    if (entry.isDirectory()) {
      files.push(...rustFiles(child));
    } else if (entry.isFile() && entry.name.endsWith('.rs')) {
      files.push(child);
    }
  }
  return files;
}

requireAll('crates/flex/src/entity_type.rs', [
  'pub const TAXONOMY_CATEGORY_ENTITY_TYPE: &str = "taxonomy.category";',
  "value.split('.').all(is_valid_field_key)",
]);

requireAll('crates/flex/src/attached_definitions.rs', [
  'flex_attached_field_definitions',
  'pub struct GenericAttachedFieldDefinitionService',
  'impl FieldDefinitionService for GenericAttachedFieldDefinitionService',
  'Column::EntityType.eq(self.entity_type)',
]);

requireAll('crates/flex/src/attached_storage.rs', [
  'flex_attached_values',
  'prepare_generic_attached_values_update',
  'persist_prepared_generic_attached_values',
  'resolve_generic_attached_values',
]);

requireAll('crates/flex/src/migrations/m20260822_000001_create_generic_attached_donor_storage.rs', [
  'flex_attached_field_definitions',
  'flex_attached_values',
  'create_field_definition_cache_generation_trigger',
]);

requireAll('crates/rustok-taxonomy/src/owner_identity.rs', [
  'pub async fn taxonomy_term_identity_exists',
  'taxonomy_term::Column::TenantId.eq(tenant_id)',
  'taxonomy_term::Column::Kind.eq(kind)',
]);

requireAll('apps/server/src/services/field_definition_registry_bootstrap.rs', [
  'GenericAttachedFieldDefinitionService::new(',
  'TAXONOMY_CATEGORY_ENTITY_TYPE',
  'registry.register(Arc::new(',
  'registry_bootstrap_registers_taxonomy_category_reference_donor',
]);

requireAll('apps/server/src/services/flex_attached_values.rs', [
  'prepare_registered_generic_update',
  'persist_registered_generic_values',
  'resolve_registered_generic_values',
  'delete_registered_generic_values',
  'rustok_taxonomy::taxonomy_term_identity_exists',
  'rustok_taxonomy::TaxonomyTermKind::Category',
  'let txn = db.begin().await?;',
  'persist_prepared_generic_attached_values(',
  'txn.commit().await?;',
  'Err(Error::NotFound)',
]);

requireAll('crates/flex/tests/generic_attached_storage.rs', [
  'generic_attached_values_split_shared_and_exact_locale_rows',
  'exact_locale_authoring_does_not_seed_from_read_fallback',
]);

requireAll('crates/rustok-taxonomy/tests/owner_identity.rs', [
  'owner_identity_is_bounded_by_tenant_and_term_kind',
  'TaxonomyModule.migrations()',
  'TaxonomyService::new',
  'TaxonomyTermKind::Category',
  'TaxonomyTermKind::Category, tag_id',
]);

for (const forbidden of [
  'TaxonomyCategoryFieldDefinitionService',
  'taxonomy_category_field_definitions',
  'taxonomy_category_custom_field_engine',
]) {
  for (const file of rustFiles('crates/rustok-taxonomy/src')) {
    if (read(file).includes(forbidden)) {
      throw new Error(`CAT-4 must reuse Flex rather than introduce ${forbidden} in ${file}`);
    }
  }
}

console.log('Taxonomy Category Flex donor source contract: OK');
