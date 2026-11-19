/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::PortError;
use rustok_channel_ports::{ChannelIndexIdentitySelector, ChannelIndexIdentitySnapshot};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult};
use serde_json::Value as JsonValue;
use thiserror::Error;
use tracing::instrument;
use uuid::Uuid;

use super::{
    ProductSalesChannelIndexRelationConvergenceClaim,
    ProductSalesChannelIndexRelationConvergenceError,
    ProductSalesChannelIndexRelationConvergenceStore,
    ProductSalesChannelIndexRelationConvergenceWork, ProductSalesChannelIndexRelationError,
    ProductSalesChannelIndexRelationObservation, ProductSalesChannelIndexRelationStore,
};
use crate::index_channel_visibility::{
    ProductChannelVisibility, ProductChannelVisibilityError, decode_product_visibility,
};

#[async_trait]
pub trait ProductSalesChannelObservationSource: Send + Sync {
    async fn observe(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
    ) -> Result<ProductSalesChannelIndexRelationObservation, ProductSalesChannelRelationResolverError>;
}

pub fn channel_identity_selector(
    visibility: &ProductChannelVisibility,
) -> ChannelIndexIdentitySelector {
    match visibility {
        ProductChannelVisibility::Unrestricted => ChannelIndexIdentitySelector::All,
        ProductChannelVisibility::Restricted(slugs) => {
            ChannelIndexIdentitySelector::Slugs(slugs.clone())
        }
    }
}

pub fn bind_channel_observation(
    tenant_id: Uuid,
    product_source_version: u64,
    visibility: ProductChannelVisibility,
    snapshot: Result<ChannelIndexIdentitySnapshot, PortError>,
) -> Result<ProductSalesChannelIndexRelationObservation, ProductSalesChannelRelationResolverError> {
    let snapshot = snapshot.map_err(|error| {
        if error.code == "channel.index_identity_limit" {
            ProductSalesChannelRelationResolverError::TooManyResolvedChannels
        } else {
            tracing::warn!(%tenant_id, %error, "Product could not observe Channel identities");
            ProductSalesChannelRelationResolverError::Unavailable
        }
    })?;
    // INVARIANT: Channel identity snapshot tenant must match port caller tenant.
    if snapshot.tenant_id != tenant_id {
        return Err(ProductSalesChannelRelationResolverError::Unavailable);
    }
    Ok(ProductSalesChannelIndexRelationObservation {
        visibility,
        product_source_version,
        channel_identity_generation: snapshot.generation,
        channel_ids: snapshot.channel_ids,
    })
}

#[derive(Debug, Error)]
pub enum ProductSalesChannelConvergenceWorkError {
    #[error(transparent)]
    Observation(#[from] ProductSalesChannelRelationResolverError),
    #[error(transparent)]
    Store(#[from] ProductSalesChannelIndexRelationConvergenceError),
}

pub const MAX_PRODUCT_SALES_CHANNEL_RELATION_RESOLVE_PAGE: usize = 64;
pub const MAX_PRODUCT_SALES_CHANNEL_STABILIZATION_ATTEMPTS: usize = 3;

#[derive(Debug, Error)]
pub enum ProductSalesChannelRelationResolverError {
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
}

impl ProductSalesChannelRelationResolverError {
    pub(super) fn is_owner_rejection(&self) -> bool {
        matches!(
            self,
            Self::InvalidProductVisibility
                | Self::TooManyVisibilitySlugs
                | Self::TooManyResolvedChannels
        )
    }
}

/// Commits a complete observation and checks both owners before advancing the convergence cursor.
#[derive(Clone)]
pub struct ProductSalesChannelRelationResolver {
    db: DatabaseConnection,
    observer: Arc<dyn ProductSalesChannelObservationSource>,
}

impl ProductSalesChannelRelationResolver {
    pub fn new(
        db: DatabaseConnection,
        observer: Arc<dyn ProductSalesChannelObservationSource>,
    ) -> Self {
        Self { db, observer }
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id, product_id = %product_id))]
    pub async fn reconcile_product(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
    ) -> Result<(), ProductSalesChannelRelationResolverError> {
        validate_scope(tenant_id, product_id)?;
        self.ensure_postgres()?;
        let owner = ProductSalesChannelIndexRelationStore::new(self.db.clone());

        for _ in 0..MAX_PRODUCT_SALES_CHANNEL_STABILIZATION_ATTEMPTS {
            let observed = self.observer.observe(tenant_id, product_id).await?;
            match owner
                .commit_observation(tenant_id, product_id, &observed)
                .await
            {
                Ok(_) => {}
                Err(ProductSalesChannelIndexRelationError::ObservationStale) => continue,
                Err(error) => return Err(map_relation_error(error)),
            }
            if self.observer.observe(tenant_id, product_id).await? == observed {
                return Ok(());
            }
        }

        Err(ProductSalesChannelRelationResolverError::ConcurrentChange)
    }

    /// Bounded convergence primitive for Channel identity changes, Product visibility changes, and
    /// initial backfill. An interrupted page is safe to retry from the same cursor.
    #[instrument(skip(self), fields(tenant_id = %tenant_id, ?after_product_id, limit))]
    pub async fn reconcile_tenant_page(
        &self,
        tenant_id: Uuid,
        after_product_id: Option<Uuid>,
        limit: usize,
    ) -> Result<Option<Uuid>, ProductSalesChannelRelationResolverError> {
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
        let next_product_id = if has_more {
            product_ids.last().copied()
        } else {
            None
        };

        for product_id in product_ids {
            match self.reconcile_product(tenant_id, product_id).await {
                Ok(_) | Err(ProductSalesChannelRelationResolverError::ProductNotFound) => {}
                Err(error) if error.is_owner_rejection() => {
                    tracing::warn!(%tenant_id, %product_id, %error, "Product visibility rejected during Channel sweep");
                }
                Err(error) => return Err(error),
            }
        }

        Ok(next_product_id)
    }

    #[instrument(skip(self, store, claim))]
    pub async fn execute_claim(
        &self,
        store: &ProductSalesChannelIndexRelationConvergenceStore,
        claim: &ProductSalesChannelIndexRelationConvergenceClaim,
        page_size: usize,
    ) -> Result<(), ProductSalesChannelConvergenceWorkError> {
        let tenant_id = claim.tenant_id();
        match *claim.work() {
            ProductSalesChannelIndexRelationConvergenceWork::VisibilityRequest {
                product_id,
                ..
            } => {
                match self.reconcile_product(tenant_id, product_id).await {
                    Ok(()) | Err(ProductSalesChannelRelationResolverError::ProductNotFound) => {}
                    Err(error) if error.is_owner_rejection() => {
                        tracing::warn!(%tenant_id, %product_id, %error, "Product visibility rejected during relation refresh");
                    }
                    Err(error) => return Err(error.into()),
                }
                store.complete_visibility(claim).await?;
            }
            ProductSalesChannelIndexRelationConvergenceWork::ChannelSweep {
                after_product_id,
                ..
            } => {
                let next = self
                    .reconcile_tenant_page(tenant_id, after_product_id, page_size)
                    .await?;
                store.complete_sweep_page(claim, next).await?;
            }
        }
        Ok(())
    }

    async fn list_product_ids(
        &self,
        tenant_id: Uuid,
        after_product_id: Option<Uuid>,
        limit: usize,
    ) -> Result<Vec<Uuid>, ProductSalesChannelRelationResolverError> {
        let limit = i64::try_from(limit)
            .map_err(|_| ProductSalesChannelRelationResolverError::InvalidPage)?;
        self.db
            .query_all_raw(crate::index_projection::relation_product_ids(
                tenant_id,
                after_product_id,
                limit,
            ))
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

pub async fn load_product_visibility(
    connection: &impl ConnectionTrait,
    tenant_id: Uuid,
    product_id: Uuid,
) -> Result<(ProductChannelVisibility, u64), ProductSalesChannelRelationResolverError> {
    let row = connection
        .query_one_raw(crate::index_projection::relation_visibility(
            tenant_id, product_id,
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
    use super::*;
    use crate::index_channel_visibility::MAX_PRODUCT_SALES_CHANNEL_VISIBILITY_SLUGS;

    #[test]
    fn resolver_bounds_are_explicit() {
        assert_eq!(MAX_PRODUCT_SALES_CHANNEL_RELATION_RESOLVE_PAGE, 64);
        assert_eq!(MAX_PRODUCT_SALES_CHANNEL_VISIBILITY_SLUGS, 1024);
        assert_eq!(MAX_PRODUCT_SALES_CHANNEL_STABILIZATION_ATTEMPTS, 3);
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
