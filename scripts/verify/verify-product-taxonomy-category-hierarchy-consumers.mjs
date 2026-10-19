#!/usr/bin/env node
// Source-only hierarchy ownership; no database/closure compatibility evidence.
import {readFileSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
const root=process.env.RUSTOK_VERIFY_REPO_ROOT??path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const read=p=>readFileSync(path.join(root,p),'utf8');
const forms=read('crates/modules/rustok-product/src/services/catalog_schema_service/effective_forms.rs');
const owner=read('crates/modules/rustok-taxonomy/src/owner_category_read.rs');
const errors=[];const need=(s,t)=>{if(!s.includes(t))errors.push(`Missing ${t}`)};
for(const marker of ['pub async fn load_scoped_categories_in<C>(', 'Self::load_scoped_categories_in(', 'C: ConnectionTrait'])need(owner,marker);
for(const marker of ['load_product_taxonomy_category_parent_map(&self.db, tenant_id).await?','load_product_taxonomy_category_parent_map(db, tenant_id).await?','taxonomy_ancestor_chain(category_id, &parent_map)?','TaxonomyOwnerCategoryReader::load_scoped_categories_in(','owner.parent_id','owner.canonical_key != expected_key','owner.scope_type != TaxonomyScopeType::Module','Some(&category_ids)','seen.insert(current_id)','super::taxonomy_projection_error(','.map_err(super::map_taxonomy_read_error)?','category_taxonomy_hierarchy_builds_root_to_leaf_ancestor_chain','category_taxonomy_hierarchy_fails_closed_on_missing_or_cyclic_owner_state'])need(forms,marker);
for(const retired of ['catalog_category_closure','product_catalog_category_taxonomy_bindings'])if(forms.includes(retired))errors.push(`Retired hierarchy source: ${retired}`);
const projection=forms.slice(forms.indexOf('fn compose_product_taxonomy_parent_map('),forms.indexOf('#[cfg(test)]'));
if(projection.includes('CommerceError::Validation'))errors.push('Persisted projection failures must be internal, not client validation');
if(errors.length)throw Error(errors.join('\n'));
console.log('Product schema ancestry: canonical Taxonomy parents, cycle/missing-owner failures source-locked (runtime open)');
