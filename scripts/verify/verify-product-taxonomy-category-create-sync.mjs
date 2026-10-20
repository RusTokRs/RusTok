#!/usr/bin/env node
// Current same-ID owner composition. Source only, not transaction/runtime proof.
import {readFileSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
const root = process.env.RUSTOK_VERIFY_REPO_ROOT ?? path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = p => readFileSync(path.join(root,p),'utf8');
const source=read('crates/modules/rustok-product/src/services/catalog_schema_service/categories.rs');
const policy=read('crates/modules/rustok-product/src/services/catalog_schema_service.rs');
const sync=read('crates/modules/rustok-taxonomy/src/owner_category_sync.rs');
const create=source.slice(source.indexOf('pub async fn create_category('),source.indexOf('pub async fn list_categories('));
const errors=[];
const need=(s,t)=>{if(!s.includes(t))errors.push(`Missing ${t}`)};
for(const marker of ['input.validate()?','normalize_category_translations(&input.translations)?','validate_taxonomy_category_route(&input.slug, input.position)?','current_product_operation_id().unwrap_or_else(generate_id)','ProductWriteTransaction::begin','INSERT INTO catalog_categories','record_product_operation_result(&result)'])need(create,marker);
let cursor=-1;
for(const marker of ['ProductWriteTransaction::begin','sync_created_category_to_taxonomy_in_tx(','INSERT INTO catalog_categories','DomainEvent::CatalogCategoryCreated','record_product_operation_result(&result)','txn.commit().await?']) {
 const index=create.indexOf(marker);if(index<=cursor)errors.push(`Create order broken: ${marker}`);cursor=index;
}
for(const marker of ['sync_module_category_in_tx(','for translation in translations','canonical_key: canonical_key_for_product_category(category_id)','module_scope: PRODUCT_TAXONOMY_SCOPE.to_owned()','parent_id: input.parent_id','position: input.position','aliases: Vec::new()','.map_err(super::map_taxonomy_sync_error)?','name.chars().count() > 120','value.chars().count() > 2_000','TAXONOMY_CATEGORY_ROUTE_KEY_MAX_BYTES: usize = 120'])need(source,marker);
need(policy,'TaxonomyError::Database(error) => CommerceError::Database(error)');
need(policy,'CommerceError::Conflict("Taxonomy category conflicts with current state".into())');
need(sync,'lock_category_hierarchy_writer_in_tx(txn, tenant_id).await?');
for(const retired of ['INSERT INTO catalog_category_translations','INSERT INTO product_catalog_category_taxonomy_bindings','INSERT INTO catalog_category_closure'])if(source.includes(retired))errors.push(`Retired duplicate writer: ${retired}`);
if(errors.length)throw Error(errors.join('\n'));
console.log('Product category create: same-ID Taxonomy owner transaction and typed failures source-locked (runtime open)');
