use super::{CatalogService, ProductTagState, helpers::normalize_tag_names};
use chrono::Utc;
use rustok_api::{PortActor, PortContext, PortErrorKind};
use rustok_core::error::Error as CoreError;
use rustok_taxonomy_ports::{
    MAX_TAXONOMY_TERM_NAMES_PER_CALL, ReadTaxonomyTermNamesRequest,
    TAXONOMY_TERM_NAMES_READ_CLAIM, TaxonomyTermNamesReadPort,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseTransaction, EntityTrait, QueryFilter,
    QueryOrder, Set,
};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::{
    entities,
    error::CommerceResult,
    ports::{AsTaxonomyConnection, NoopProductTaxonomyPort, TaxonomyConnection},
};

use crate::entities::product_tag;

impl CatalogService {
    pub(crate) async fn load_product_tag_map(
        &self,
        tenant_id: Uuid,
        products: &[entities::product::Model],
        locale: &str,
        fallback_locale: Option<&str>,
        context: Option<&PortContext>,
    ) -> CommerceResult<HashMap<Uuid, Vec<String>>> {
        let product_ids = products
            .iter()
            .map(|product| product.id)
            .collect::<Vec<_>>();
        if product_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let relations = product_tag::Entity::find()
            .filter(product_tag::Column::TenantId.eq(tenant_id))
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

        let names = self
            .read_taxonomy_term_names(tenant_id, &ordered_term_ids, locale, fallback_locale, context)
            .await?;

        let mut tags_by_product = HashMap::new();
        for product in products {
            if let Some(relations) = relations_by_product.get(&product.id) {
                let tags = relations
                    .iter()
                    .filter_map(|relation| names.get(&relation.term_id).cloned())
                    .collect::<Vec<_>>();
                tags_by_product.insert(product.id, tags);
            }
        }

        Ok(tags_by_product)
    }

    pub(crate) async fn load_product_tags_in<C: ConnectionTrait + AsTaxonomyConnection>(
        &self,
        conn: &C,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        context: Option<&PortContext>,
    ) -> CommerceResult<ProductTagState> {
        let relations = product_tag::Entity::find()
            .filter(product_tag::Column::TenantId.eq(tenant_id))
            .filter(product_tag::Column::ProductId.eq(product_id))
            .order_by_asc(product_tag::Column::CreatedAt)
            .all(conn)
            .await?;

        if relations.is_empty() {
            return Ok(ProductTagState { tags: Vec::new() });
        }

        let term_ids = relations
            .iter()
            .map(|relation| relation.term_id)
            .collect::<Vec<_>>();
        let names = match conn.as_taxonomy_connection() {
            TaxonomyConnection::Connection(_) => {
                self.read_taxonomy_term_names(tenant_id, &term_ids, locale, fallback_locale, context)
                    .await?
            }
            TaxonomyConnection::Transaction(txn) => {
                self.taxonomy()
                    .resolve_term_names_in_tx(txn, tenant_id, &term_ids, locale, fallback_locale)
                    .await?
            }
        };

        let mut tags = Vec::new();
        for relation in relations {
            if let Some(name) = names.get(&relation.term_id) {
                tags.push(name.clone());
            }
        }

        Ok(ProductTagState { tags })
    }

    async fn read_taxonomy_term_names(
        &self,
        tenant_id: Uuid,
        term_ids: &[Uuid],
        locale: &str,
        fallback_locale: Option<&str>,
        origin: Option<&PortContext>,
    ) -> CommerceResult<HashMap<Uuid, String>> {
        let port = self.taxonomy_term_names_port.as_deref().or_else(|| {
            self.taxonomy_port.as_deref().map(|port| port as &dyn TaxonomyTermNamesReadPort)
        }).unwrap_or(&NoopProductTaxonomyPort);
        let budget = Duration::from_millis(
            origin.and_then(|context| context.deadline_ms).unwrap_or(2_000).min(2_000),
        );
        let mut context = PortContext::new(
            tenant_id.to_string(),
            PortActor::service("product"),
            locale,
            origin.map_or_else(|| Uuid::new_v4().to_string(), |context| context.correlation_id.clone()),
        )
        .with_claim(TAXONOMY_TERM_NAMES_READ_CLAIM);
        if let Some(origin) = origin {
            context.traceparent.clone_from(&origin.traceparent);
            context.causation_id.clone_from(&origin.causation_id);
            context.channel.clone_from(&origin.channel);
        }

        let started = Instant::now();
        let mut names = HashMap::new();
        for ids in term_ids.chunks(MAX_TAXONOMY_TERM_NAMES_PER_CALL) {
            let remaining = budget.saturating_sub(started.elapsed());
            if remaining.as_millis() == 0 {
                return Err(CoreError::External("Taxonomy term names deadline elapsed".into()).into());
            }
            let response = tokio::time::timeout(
                remaining,
                port.read_term_names(
                    context.clone().with_deadline(remaining),
                    ReadTaxonomyTermNamesRequest {
                        term_ids: ids.to_vec(),
                        fallback_locale: fallback_locale.map(str::to_owned),
                    },
                ),
            )
            .await
            .map_err(|_| CoreError::External("Taxonomy term names deadline elapsed".into()))?
            .map_err(|error| match error.kind {
                PortErrorKind::Unavailable | PortErrorKind::Timeout => {
                    CoreError::External("Taxonomy term names are unavailable".into())
                }
                PortErrorKind::Forbidden => CoreError::Forbidden("Taxonomy term names access denied".into()),
                _ => CoreError::Internal("Taxonomy term names could not be read safely".into()),
            })?;
            if response.keys().any(|id| !ids.contains(id)) {
                return Err(CoreError::Internal("Taxonomy returned an unrequested term identity".into()).into());
            }
            names.extend(response);
        }
        Ok(names)
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
            .filter(product_tag::Column::TenantId.eq(tenant_id))
            .filter(product_tag::Column::ProductId.eq(product_id))
            .exec(txn)
            .await?;

        if normalized_tags.is_empty() {
            return Ok(());
        }

        let term_ids = self
            .taxonomy()
            .ensure_product_tags_in_tx(txn, tenant_id, locale, &normalized_tags)
            .await?;

        let now = Utc::now();
        for (position, term_id) in term_ids.into_iter().enumerate() {
            product_tag::ActiveModel {
                product_id: Set(product_id),
                term_id: Set(term_id),
                tenant_id: Set(tenant_id),
                created_at: Set((now + chrono::Duration::microseconds(position as i64)).into()),
            }
            .insert(txn)
            .await?;
        }

        Ok(())
    }
}
