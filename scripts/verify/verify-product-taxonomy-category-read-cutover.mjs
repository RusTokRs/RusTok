#!/usr/bin/env node
// Current all-backend canonical read, not a historical donor compatibility gate.
import {readFileSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
const root=process.env.RUSTOK_VERIFY_REPO_ROOT??path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const read=p=>readFileSync(path.join(root,p),'utf8');
const categories=read('crates/modules/rustok-product/src/services/catalog_schema_service/categories.rs');
const owner=read('crates/modules/rustok-taxonomy/src/owner_category_read.rs');
const errors=[];const need=(s,t)=>{if(!s.includes(t))errors.push(`Missing ${t}`)};
const list=categories.slice(categories.indexOf('pub async fn list_categories('),categories.indexOf('pub async fn create_category_group('));
need(list,'list_categories_from_taxonomy(self, tenant_id, &locale).await');
if(/Postgres|list_categories_from_product_donor/.test(list))errors.push('All backends must use the canonical Taxonomy reader');
for(const marker of ['WHERE c.tenant_id = $1 AND c.deleted_at IS NULL','MAX_SCHEMA_LIST_ROWS + 1','ensure_schema_list_within_limit(rows.len(), "category")','TaxonomyOwnerCategoryReader::new(service.db.clone())','Some(&taxonomy_category_ids)','Some(PRODUCT_TAXONOMY_SCOPE)','Some(PLATFORM_FALLBACK_LOCALE)','owner.parent_id','slug: owner.slug','name: owner.name','code: row.code','path: row.path','kind: row.kind','canonical_key_for_product_category(row.id)','owner.scope_type != TaxonomyScopeType::Module','owner.available_locales.is_empty()','super::taxonomy_projection_error(','missing its Taxonomy owner projection','.map_err(super::map_taxonomy_read_error)?'])need(categories,marker);
for(const marker of ['pub async fn load_scoped_categories(','TaxonomyTermKind::Category','taxonomy_term::Column::TenantId.eq(tenant_id)','resolve_by_locale_with_fallback(','taxonomy_category_hierarchy::Entity::find()'])need(owner,marker);
for(const retired of ['list_categories_from_product_donor','catalog_category_translations','product_catalog_category_taxonomy_bindings'])if(categories.includes(retired))errors.push(`Retired Category read: ${retired}`);
if(errors.length)throw Error(errors.join('\n'));
console.log('Product category reads: bounded all-backend Taxonomy projection and internal corruption class source-locked (runtime open)');
