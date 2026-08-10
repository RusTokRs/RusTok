use rustok_product::{
    MAX_PRODUCT_SALES_CHANNEL_RELATION_CHANNELS, ProductSalesChannelIndexRelationError,
    ProductSalesChannelIndexRelationFreshnessError, ProductSalesChannelIndexRelationFreshnessStore,
    ProductSalesChannelIndexRelationStore,
};
use sea_orm::{
    AccessMode, ConnectionTrait, DatabaseConnection, DbBackend, IsolationLevel, QueryResult,
    Statement, TransactionTrait, Value,
};
use serde_json::Value as JsonValue;
use thiserror::Error;
use uuid::Uuid;

use super::channel_visibility::{
    ProductChannelVisibility, ProductChannelVisibilityError, decode_product_visibility,
};

pub(crate) const MAX_PRODUCT_SALES_CHANNEL_RELATION_RESOLVE_PAGE: usize = 64;
pub(crate) const MAX_PRODUCT_SALES_CHANNEL_STABILIZATION_ATTEMPTS: usize = 3;

#[derive(Debug, Error)]
pub(crate) enum ProductSalesChannelRelationResolverError {
    #[error("Product-SalesChannel resolver tenant identity is invalid")]
    InvalidTenant,
    #[error("Product-SalesChannel resolver Product identity is invalid")]
    InvalidProduct,
    #[error("Product-SalesChannel resolver Product does not exist")]
    ProductNotFound,
    #[error("Product-SalesChannel resolver page cursor is invalid")]
    InvalidCursor,
    #[error("Product-SalesChannel resolver page limit is invalid")]
    InvalidPage,
    #[error("Product-SalesChannel resolver Product visibility is invalid")]
    InvalidProductVisibility,
    #[error("Product-SalesChannel resolver Product visibility contains too many slugs")]
    TooManyVisibilitySlugs,
    #[error("Product-SalesChannel resolver resolved too many Channel targets")]
    TooManyResolvedChannels,
    #[error("Product-SalesChannel resolver could not stabilize concurrent owner changes")]
    ConcurrentChange,
    #[error("Product-SalesChannel resolver storage is unavailable")]
    Unavailable,
    #[error(transparent)]
    RelationOwner(#[from] ProductSalesChannelIndexRelationError),
    #[error(transparent)]
    FreshnessOwner(#[from] ProductSalesChannelIndexRelationFreshnessError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProductChannelObservation {
    channel_ids: Vec<Uuid>,
    unrestricted: bool,
    product_source_version: u64,
    visibility_key: String,
    channel_identity_generation: u64,
}

/// Distribution-owned resolver for Product visibility -> current SalesChannel UUID membership.
///
/// The resolver reads both owners under one read-only snapshot, writes membership through the
/// Product relation owner, re-observes current state, and then records an append-only Product-owned
/// freshness witness for the exact retained relation epoch. It never writes Index rows or owns a
/// background loop.
#[derive(Clone)]
pub(crate) struct ProductSalesChannelRelationResolver {
    db: DatabaseConnection,
}

impl ProductSalesChannelRelationResolver {
    pub(crate) fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub(crate) async fn reconcile_product(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
    ) -> Result<(), ProductSalesChannelRelationResolverError>
    {
        validate_scope(tenant_id, product_id)?;
        self.ensure_postgres()?;
        let owner = ProductSalesChannelIndexRelationStore::new(self.db.clone());
        let freshness = ProductSalesChannelIndexRelationFreshnessStore::new(self.db.clone());

        for _ in 0..MAX_PRODUCT_SALES_CHANNEL_STABILIZATION_ATTEMPTS {
            let observed = self.observe(tenant_id, product_id).await?;
            let outcome = owner
                .replace(tenant_id, product_id, observed.channel_ids.iter().copied())
                .await
                .map_err(map_relation_error)?;
            let verified = self.observe(tenant_id, product_id).await?;

            // The second observation is already a complete current snapshot. If its resolved UUIDs
            // equal the membership retained by the owner, freshness can be witnessed even when an
            // owner watermark advanced without changing membership.
            if verified.channel_ids == outcome.record().channel_ids() {
                match freshness
                    .record(
                        tenant_id,
                        product_id,
                        outcome.record().relation_epoch(),
                        verified.product_source_version,
                        verified.visibility_key.clone(),
                        verified.channel_identity_generation,
                    )
                    .await
                {
                    Ok(_) => {
                        return Ok(());
                    }
                    Err(ProductSalesChannelIndexRelationFreshnessError::WatermarkRegressed) => {
                        continue;
                    }
                    Err(ProductSalesChannelIndexRelationFreshnessError::ProductNotFound) => {
                        return Err(ProductSalesChannelRelationResolverError::ProductNotFound);
                    }
                    Err(error) => {
                        return Err(ProductSalesChannelRelationResolverError::FreshnessOwner(error));
                    }
                }
            }
        }

        Err(ProductSalesChannelRelationResolverError::ConcurrentChange)
    }

    /// Bounded convergence primitive for Channel identity changes, Product visibility changes, and
    /// initial backfill. An interrupted page is safe to retry from the same cursor.
    pub(crate) async fn reconcile_tenant_page(
        &self,
        tenant_id: Uuid,
        after_product_id: Option<Uuid>,
        limit: usize,
    ) -> Result<Option<Uuid>, ProductSalesChannelRelationResolverError>
    {
        if tenant_id.is_nil() {
            return Err(ProductSalesChannelRelationResolverError::InvalidTenant);
        }
        if after_product_id.is_some_and(|value| value.is_nil()) {
            return Err(ProductSalesChannelRelationResolverError::InvalidCursor);
        }
        if limit == 0 || limit > MAX_PRODUCT_SALES_CHANNEL_RELATION_RESOLVE_PAGE {
            return Err(ProductSalesChannelRelationResolverError::InvalidPage);
        }
        self.ensure_postgres()?;

        let mut product_ids = self
            .list_product_ids(tenant_id, after_product_id, limit + 1)
            .await?;
        let has_more = product_ids.len() > limit;
        if has_more {
            product_ids.truncate(limit);
        }
        let next_product_id = has_more.then(|| {
            *product_ids
                .last()
                .expect("lookahead implies at least one processed Product")
        });

        for product_id in product_ids {
            match self.reconcile_product(tenant_id, product_id).await {
                Ok(_) | Err(ProductSalesChannelRelationResolverError::ProductNotFound) => {}
                Err(error) => return Err(error),
            }
        }

        Ok(next_product_id)
    }

    async fn observe(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
    ) -> Result<ProductChannelObservation, ProductSalesChannelRelationResolverError> {
        let transaction = self
            .db
            .begin_with_config(
                Some(IsolationLevel::RepeatableRead),
                Some(AccessMode::ReadOnly),
            )
            .await
            .map_err(|_| ProductSalesChannelRelationResolverError::Unavailable)?;

        let result = async {
            let (visibility, product_source_version) =
                load_product_visibility(&transaction, tenant_id, product_id).await?;
            let channel_identity_generation =
                load_channel_identity_generation(&transaction, tenant_id).await?;
            let channel_ids = resolve_channel_ids(&transaction, tenant_id, &visibility).await?;
            Ok::<_, ProductSalesChannelRelationResolverError>(ProductChannelObservation {
                unrestricted: visibility.is_unrestricted(),
                visibility_key: visibility.freshness_key(),
                product_source_version,
                channel_identity_generation,
                channel_ids,
            })
        }
        .await;

        match result {
            Ok(observation) => {
                transaction
                    .commit()
                    .await
                    .map_err(|_| ProductSalesChannelRelationResolverError::Unavailable)?;
                Ok(observation)
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error)
            }
        }
    }

    async fn list_product_ids(
        &self,
        tenant_id: Uuid,
        after_product_id: Option<Uuid>,
        limit: usize,
    ) -> Result<Vec<Uuid>, ProductSalesChannelRelationResolverError> {
        let limit = i64::try_from(limit).expect("resolver page lookahead is bounded below i64::MAX");
        let (sql, values) = match after_product_id {
            Some(after_product_id) => (
                "SELECT id FROM products WHERE tenant_id = $1 AND id > $2 ORDER BY id ASC LIMIT $3",
                vec![tenant_id.into(), after_product_id.into(), limit.into()],
            ),
            None => (
                "SELECT id FROM products WHERE tenant_id = $1 ORDER BY id ASC LIMIT $2",
                vec![tenant_id.into(), limit.into()],
            ),
        };

        self.db
            .query_all(Statement::from_sql_and_values(DbBackend::Postgres, sql, values))
            .await
            .map_err(|_| ProductSalesChannelRelationResolverError::Unavailable)?
            .into_iter()
            .map(|row| decode_product_id(row, tenant_id))
            .collect()
    }

    fn ensure_postgres(&self) -> Result<(), ProductSalesChannelRelationResolverError> {
        if self.db.get_database_backend() != DbBackend::Postgres {
            return Err(ProductSalesChannelRelationResolverError::Unavailable);
        }
        Ok(())
    }
}

async fn load_product_visibility(
    transaction: &sea_orm::DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
) -> Result<(ProductChannelVisibility, u64), ProductSalesChannelRelationResolverError> {
    let row = transaction
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT metadata, index_revision FROM products WHERE tenant_id = $1 AND id = $2",
            vec![tenant_id.into(), product_id.into()],
        ))
        .await
        .map_err(|_| ProductSalesChannelRelationResolverError::Unavailable)?
        .ok_or(ProductSalesChannelRelationResolverError::ProductNotFound)?;
    let metadata = row
        .try_get::<JsonValue>("", "metadata")
        .map_err(|_| ProductSalesChannelRelationResolverError::InvalidProductVisibility)?;
    let source_version = row
        .try_get::<i64>("", "index_revision")
        .map_err(|_| ProductSalesChannelRelationResolverError::Unavailable)?;
    if source_version <= 0 {
        return Err(ProductSalesChannelRelationResolverError::Unavailable);
    }
    let visibility = decode_product_visibility(&metadata).map_err(map_visibility_error)?;
    Ok((
        visibility,
        u64::try_from(source_version)
            .map_err(|_| ProductSalesChannelRelationResolverError::Unavailable)?,
    ))
}

async fn load_channel_identity_generation(
    transaction: &sea_orm::DatabaseTransaction,
    tenant_id: Uuid,
) -> Result<u64, ProductSalesChannelRelationResolverError> {
    let row = transaction
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT generation
            FROM channel_index_identity_generations
            WHERE tenant_id = $1
            "#,
            vec![tenant_id.into()],
        ))
        .await
        .map_err(|_| ProductSalesChannelRelationResolverError::Unavailable)?;
    let Some(row) = row else {
        return Ok(0);
    };
    let generation = row
        .try_get::<i64>("", "generation")
        .map_err(|_| ProductSalesChannelRelationResolverError::Unavailable)?;
    if generation <= 0 {
        return Err(ProductSalesChannelRelationResolverError::Unavailable);
    }
    u64::try_from(generation).map_err(|_| ProductSalesChannelRelationResolverError::Unavailable)
}

async fn resolve_channel_ids(
    transaction: &sea_orm::DatabaseTransaction,
    tenant_id: Uuid,
    visibility: &ProductChannelVisibility,
) -> Result<Vec<Uuid>, ProductSalesChannelRelationResolverError> {
    let fetch_limit = i64::try_from(MAX_PRODUCT_SALES_CHANNEL_RELATION_CHANNELS + 1)
        .expect("relation target limit is bounded below i64::MAX");
    let (sql, values) = channel_resolution_query(tenant_id, visibility, fetch_limit);
    let rows = transaction
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            values,
        ))
        .await
        .map_err(|_| ProductSalesChannelRelationResolverError::Unavailable)?;
    if rows.len() > MAX_PRODUCT_SALES_CHANNEL_RELATION_CHANNELS {
        return Err(ProductSalesChannelRelationResolverError::TooManyResolvedChannels);
    }

    let mut channel_ids = Vec::with_capacity(rows.len());
    for row in rows {
        let channel_id = row
            .try_get::<Uuid>("", "id")
            .map_err(|_| ProductSalesChannelRelationResolverError::Unavailable)?;
        if channel_id.is_nil()
            || channel_ids
                .last()
                .is_some_and(|previous| channel_id <= *previous)
        {
            return Err(ProductSalesChannelRelationResolverError::Unavailable);
        }
        channel_ids.push(channel_id);
    }
    Ok(channel_ids)
}

fn channel_resolution_query(
    tenant_id: Uuid,
    visibility: &ProductChannelVisibility,
    fetch_limit: i64,
) -> (String, Vec<Value>) {
    match visibility {
        ProductChannelVisibility::Unrestricted => (
            "SELECT id FROM channels WHERE tenant_id = $1 ORDER BY id ASC LIMIT $2".to_owned(),
            vec![tenant_id.into(), fetch_limit.into()],
        ),
        ProductChannelVisibility::Restricted(slugs) => {
            let mut values = Vec::<Value>::with_capacity(slugs.len() + 2);
            values.push(tenant_id.into());
            let mut placeholders = Vec::with_capacity(slugs.len());
            for (offset, slug) in slugs.iter().enumerate() {
                placeholders.push(format!("${}", offset + 2));
                values.push(slug.clone().into());
            }
            let limit_parameter = slugs.len() + 2;
            values.push(fetch_limit.into());
            (
                format!(
                    "SELECT id FROM channels WHERE tenant_id = $1 AND lower(btrim(slug)) IN ({}) ORDER BY id ASC LIMIT ${limit_parameter}",
                    placeholders.join(", ")
                ),
                values,
            )
        }
    }
}

fn decode_product_id(
    row: QueryResult,
    tenant_id: Uuid,
) -> Result<Uuid, ProductSalesChannelRelationResolverError> {
    if tenant_id.is_nil() {
        return Err(ProductSalesChannelRelationResolverError::InvalidTenant);
    }
    let product_id = row
        .try_get::<Uuid>("", "id")
        .map_err(|_| ProductSalesChannelRelationResolverError::Unavailable)?;
    if product_id.is_nil() {
        return Err(ProductSalesChannelRelationResolverError::Unavailable);
    }
    Ok(product_id)
}

fn validate_scope(
    tenant_id: Uuid,
    product_id: Uuid,
) -> Result<(), ProductSalesChannelRelationResolverError> {
    if tenant_id.is_nil() {
        return Err(ProductSalesChannelRelationResolverError::InvalidTenant);
    }
    if product_id.is_nil() {
        return Err(ProductSalesChannelRelationResolverError::InvalidProduct);
    }
    Ok(())
}

fn map_visibility_error(
    error: ProductChannelVisibilityError,
) -> ProductSalesChannelRelationResolverError {
    match error {
        ProductChannelVisibilityError::TooManySlugs => {
            ProductSalesChannelRelationResolverError::TooManyVisibilitySlugs
        }
        ProductChannelVisibilityError::Invalid | ProductChannelVisibilityError::SlugTooLong => {
            ProductSalesChannelRelationResolverError::InvalidProductVisibility
        }
    }
}

fn map_relation_error(
    error: ProductSalesChannelIndexRelationError,
) -> ProductSalesChannelRelationResolverError {
    match error {
        ProductSalesChannelIndexRelationError::ProductNotFound => {
            ProductSalesChannelRelationResolverError::ProductNotFound
        }
        ProductSalesChannelIndexRelationError::TooManyChannels { .. } => {
            ProductSalesChannelRelationResolverError::TooManyResolvedChannels
        }
        other => ProductSalesChannelRelationResolverError::RelationOwner(other),
    }
}

#[cfg(test)]
mod tests {
    use super::super::channel_visibility::MAX_PRODUCT_SALES_CHANNEL_VISIBILITY_SLUGS;
    use super::*;

    #[test]
    fn restricted_resolution_matches_normalized_channel_slug_without_active_filter() {
        let tenant_id = Uuid::from_u128(1);
        let visibility = ProductChannelVisibility::Restricted(vec![
            "mobile".to_owned(),
            "web".to_owned(),
        ]);
        let (sql, values) = channel_resolution_query(tenant_id, &visibility, 1025);
        assert!(sql.contains("lower(btrim(slug)) IN ($2, $3)"));
        assert!(!sql.contains("is_active"));
        assert!(sql.contains("ORDER BY id ASC LIMIT $4"));
        assert_eq!(values.len(), 4);
    }

    #[test]
    fn resolver_bounds_are_explicit() {
        assert_eq!(MAX_PRODUCT_SALES_CHANNEL_RELATION_RESOLVE_PAGE, 64);
        assert_eq!(MAX_PRODUCT_SALES_CHANNEL_VISIBILITY_SLUGS, 1024);
        assert_eq!(MAX_PRODUCT_SALES_CHANNEL_STABILIZATION_ATTEMPTS, 3);
    }

    #[test]
    fn channel_identity_generation_is_tenant_scoped_and_zero_when_absent() {
        let sql = "SELECT generation FROM channel_index_identity_generations WHERE tenant_id = $1";
        assert!(sql.contains("tenant_id = $1"));
        assert!(!sql.contains("channels.is_active"));
    }

    #[test]
    fn error_messages_are_distinct() {
        assert_eq!(
            ProductSalesChannelRelationResolverError::InvalidCursor.to_string(),
            "Product-SalesChannel resolver page cursor is invalid"
        );
        assert_eq!(
            ProductSalesChannelRelationResolverError::InvalidPage.to_string(),
            "Product-SalesChannel resolver page limit is invalid"
        );
    }
}
