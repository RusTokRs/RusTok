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
  'delete_generic_attached_values',
]);

requireAll('crates/flex/src/migrations/m20260822_000001_create_generic_attached_donor_storage.rs', [
  'flex_attached_field_definitions',
  'flex_attached_values',
  'create_field_definition_cache_generation_trigger',
]);

requireAll('crates/flex/src/graphql/runtime.rs', [
  'pub trait AttachedValuesGraphqlPort',
  'async fn resolve_values(',
  'async fn update_values(',
  'async fn delete_values(',
  'attached_values: Arc<dyn AttachedValuesGraphqlPort>',
]);

requireAll('crates/flex/src/graphql/query.rs', [
  'async fn attached_values(',
  'Permission::FLEX_ENTRIES_READ',
  '&tenant.default_locale',
  '.resolve_values(',
]);

requireAll('crates/flex/src/graphql/mutation.rs', [
  'async fn update_attached_values(',
  'Permission::FLEX_ENTRIES_UPDATE',
  '.update_values(',
  'async fn delete_attached_values(',
  'Permission::FLEX_ENTRIES_DELETE',
  '.delete_values(',
]);

requireAll('crates/rustok-taxonomy/src/owner_identity.rs', [
  'pub async fn taxonomy_term_identity_exists',
  'taxonomy_term::Column::TenantId.eq(tenant_id)',
  'taxonomy_term::Column::Kind.eq(kind)',
]);

requireAll('crates/rustok-taxonomy/src/category_delete.rs', [
  'pub trait TaxonomyCategoryDeleteCleanupPort',
  'pub async fn delete_category_with_cleanup(',
  'cleanup.cleanup_in_tx(&txn, tenant_id, category_id).await?;',
  'TaxonomyTermKind::Category',
  'txn.commit().await?;',
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
  'pub struct FlexAttachedValuesGraphqlAdapter',
  'impl flex::graphql::AttachedValuesGraphqlPort for FlexAttachedValuesGraphqlAdapter',
  'pub struct FlexTaxonomyCategoryDeleteCleanup',
  'impl rustok_taxonomy::TaxonomyCategoryDeleteCleanupPort for FlexTaxonomyCategoryDeleteCleanup',
  'delete_generic_attached_values(',
  'rustok_taxonomy::taxonomy_term_identity_exists',
  'rustok_taxonomy::TaxonomyTermKind::Category',
  'let txn = db.begin().await?;',
  'persist_prepared_generic_attached_values(',
  'txn.commit().await?;',
  'Err(Error::NotFound)',
]);

requireAll('apps/server/src/graphql/schema.rs', [
  'FlexAttachedValuesGraphqlAdapter',
  'Arc::new(FlexAttachedValuesGraphqlAdapter::new(db.clone()))',
]);

requireAll('apps/server/tests/taxonomy_category_flex_attached_postgres.rs', [
  'category_flex_transport_roundtrips_real_owner_and_hard_delete_cleans_values',
  'FlexAttachedValuesGraphqlAdapter::new(db.clone())',
  'TaxonomyTermKind::Category',
  'TaxonomyTermKind::Tag',
  'other_tenant_id',
  '"مرحبا"',
  'delete_category_with_cleanup(',
  'FlexTaxonomyCategoryDeleteCleanup',
  'load_generic_attached_shared_values',
  'load_exact_locale_values',
]);

requireAll('crates/flex/tests/generic_attached_definitions.rs', [
  'generic_definition_service_is_tenant_scoped_and_reuses_flex_guards',
  'same key should be allowed in another tenant',
  'DuplicateFieldKey',
  'get_schema',
]);

requireAll('crates/flex/tests/generic_attached_storage.rs', [
  'generic_attached_values_split_shared_and_exact_locale_rows',
  'exact_locale_authoring_does_not_seed_from_read_fallback',
]);

requireAll('crates/flex/tests/postgres_generic_attached_storage.rs', [
  'postgres_generic_category_donor_roundtrips_and_advances_definition_generation',
  'RUSTOK_FLEX_TEST_POSTGRES_URL',
  'generation(&db).await > generation_before',
  'generic values must remain tenant-isolated',
]);

requireAll('crates/rustok-taxonomy/tests/owner_identity.rs', [
  'owner_identity_is_bounded_by_tenant_and_term_kind',
  'TaxonomyModule.migrations()',
  'TaxonomyService::new',
  'TaxonomyTermKind::Category',
  'TaxonomyTermKind::Category, tag_id',
]);

requireAll('.github/workflows/taxonomy-category-flex-donor-contract.yml', [
  'TARGET_SHA: ${{ github.event.pull_request.head.sha || github.sha }}',
  'ref: ${{ env.TARGET_SHA }}',
  'Assert exact checked-out SHA',
  'Check CAT-4 Rust formatting only',
  'crates/flex/src/graphql/runtime.rs',
  'crates/rustok-taxonomy/src/category_delete.rs',
  'apps/server/src/graphql/schema.rs',
  'taxonomy_category_flex_attached_postgres',
  'generic_attached_definitions',
  'generic_attached_storage',
  'postgres_generic_attached_storage',
  'RUSTOK_FLEX_TEST_POSTGRES_URL',
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
