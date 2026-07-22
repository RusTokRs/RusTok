use super::{
    CatalogService, PRODUCT_SCOPE_VALUE, ProductTagState,
    helpers::{extract_metadata_tags, metadata_has_tags_field, normalize_tag_names},
};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use rustok_commerce_foundation::{
    entities,
    error::{CommerceError, CommerceResult},
};
use rustok_taxonomy::{TaxonomyService, TaxonomyTermKind};

use crate::entities::product_tag;

impl CatalogService {
    pub async fn load_product_tag_map(
        &self,
        tenant_id: Uuid,
        products: &[entities::product::Model],
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> CommerceResult<HashMap<Uuid, Vec<String>>> {
        let product_ids = products
            .iter()
            .map(|product| product.id)
            .collect::<Vec<_>>();
        if product_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let relations = product_tag::Entity::find()
            .filter(product_tag::Column::ProductId.is_in(product_ids.clone()))
            .order_by_asc(product_tag::Column::ProductId)
            .order_by_asc(product_tag::Column::CreatedAt)
            .all(&self.db)
            .await?;

        let mut relations_by_product: HashMap<Uuid, Vec<product_tag::Model>> = HashMap::new();
        let mut ordered_term_ids = Vec::new();
        let mut seen_term_ids = HashSet::new();
        for relation in relations {
            if seen_term_ids.insert(relation.term_id) {
                ordered_term_ids.push(relation.term_id);
            }
            relations_by_product
                .entry(relation.product_id)
                .or_default()
                .push(relation);
        }

        let names = if ordered_term_ids.is_empty() {
            HashMap::new()
        } else {
            TaxonomyService::new(self.db.clone())
                .resolve_term_names(tenant_id, &ordered_term_ids, locale, fallback_locale)
                .await
                .map_err(|error| CommerceError::Validation(error.to_string()))?
        };

        let mut tags_by_product = HashMap::new();
        for product in products {
            if let Some(relations) = relations_by_product.get(&product.id) {
                let tags = relations
                    .iter()
                    .filter_map(|relation| names.get(&relation.term_id).cloned())
                    .collect::<Vec<_>>();
                tags_by_product.insert(product.id, tags);
                continue;
            }

            if metadata_has_tags_field(&product.metadata) {
                tags_by_product.insert(
                    product.id,
                    normalize_tag_names(&extract_metadata_tags(&product.metadata)),
                );
            }
        }

        Ok(tags_by_product)
    }

    pub(crate) async fn load_product_tags(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        metadata: &Value,
    ) -> CommerceResult<ProductTagState> {
        let relations = product_tag::Entity::find()
            .filter(product_tag::Column::ProductId.eq(product_id))
            .order_by_asc(product_tag::Column::CreatedAt)
            .all(&self.db)
            .await?;

        if relations.is_empty() {
            if metadata_has_tags_field(metadata) {
                return Ok(ProductTagState {
                    tags: normalize_tag_names(&extract_metadata_tags(metadata)),
                });
            }

            return Ok(ProductTagState { tags: Vec::new() });
        }

        let term_ids = relations
            .iter()
            .map(|relation| relation.term_id)
            .collect::<Vec<_>>();
        let names = TaxonomyService::new(self.db.clone())
            .resolve_term_names(tenant_id, &term_ids, locale, fallback_locale)
            .await
            .map_err(|error| CommerceError::Validation(error.to_string()))?;

        let mut tags = Vec::new();
        for relation in relations {
            if let Some(name) = names.get(&relation.term_id) {
                tags.push(name.clone());
            }
        }

        Ok(ProductTagState { tags })
    }

    pub(crate) async fn sync_product_tags_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: &str,
        tag_names: &[String],
    ) -> CommerceResult<()> {
        let normalized_tags = normalize_tag_names(tag_names);

        product_tag::Entity::delete_many()
            .filter(product_tag::Column::ProductId.eq(product_id))
            .exec(txn)
            .await?;

        if normalized_tags.is_empty() {
            return Ok(());
        }

        let term_ids = TaxonomyService::new(self.db.clone())
            .ensure_terms_for_module_in_tx(
                txn,
                tenant_id,
                TaxonomyTermKind::Tag,
                PRODUCT_SCOPE_VALUE,
                locale,
                &normalized_tags,
            )
            .await
            .map_err(|error| CommerceError::Validation(error.to_string()))?;

        let now = Utc::now();
        for term_id in term_ids {
            product_tag::ActiveModel {
                product_id: Set(product_id),
                term_id: Set(term_id),
                tenant_id: Set(tenant_id),
                created_at: Set(now.into()),
            }
            .insert(txn)
            .await?;
        }

        Ok(())
    }
}
