use thiserror::Error;
use uuid::Uuid;

use rustok_api::{PortContext, PortError};
use rustok_index::{
    IndexQueryExecutionError, IndexQueryPage, IndexQueryPort, SharedIndexQueryRuntime,
};
use rustok_product::{
    FilteredPublishedProductsRequest, ProductCatalogReadRuntime,
    ProductStorefrontAttributeFilterResolutionRequest, StorefrontProductList,
    StorefrontProductListQuery,
};

use super::{
    ProductStorefrontIndexShadowError, build_product_storefront_index_shadow_query,
};

/// Non-serving Product Storefront parity execution result.
///
/// The authoritative owner result is always produced first. Projected failures or mismatches are retained
/// separately and never replace the owner result.
#[derive(Debug)]
pub(crate) struct ProductStorefrontIndexShadowExecution {
    pub(crate) authoritative: StorefrontProductList,
    pub(crate) projected: Result<IndexQueryPage, ProductStorefrontIndexShadowProjectionError>,
    pub(crate) comparison: Option<ProductStorefrontIndexShadowComparison>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProductStorefrontIndexShadowComparison {
    pub(crate) identities_match: bool,
    pub(crate) exact_count_matches: bool,
    pub(crate) has_more_matches: bool,
}

impl ProductStorefrontIndexShadowComparison {
    pub(crate) fn is_match(self) -> bool {
        self.identities_match && self.exact_count_matches && self.has_more_matches
    }
}

#[derive(Debug, Error)]
pub(crate) enum ProductStorefrontIndexShadowProjectionError {
    #[error("Product Storefront shadow schema-read capability is unavailable")]
    SchemaReadPortUnavailable,
    #[error("Product Storefront shadow request tenant identity is invalid")]
    InvalidTenant,
    #[error("Product Storefront shadow requires a trusted public channel slug/id pair")]
    PublicChannelIdentityUnavailable,
    #[error("Product Storefront shadow attribute-filter owner resolution failed: {0}")]
    AttributeFilterResolution(PortError),
    #[error(transparent)]
    QueryBuild(#[from] ProductStorefrontIndexShadowError),
    #[error(transparent)]
    Index(#[from] IndexQueryExecutionError),
}

/// Owner-first, non-serving Product Storefront shadow executor.
///
/// This object composes only host-selected Product and Index capabilities. It never constructs
/// `CatalogService`, `ProductCatalogSchemaService`, a PostgreSQL Index port, or a database connection.
/// The owner list result remains authoritative even when Product metadata resolution, shadow query build,
/// Index readiness/admission, or Index execution fails.
///
/// `public_channel_slug` and `public_channel_id` must be a trusted pair supplied by the caller's current
/// channel context. This executor only checks that both identities are present and non-empty/non-nil; it
/// does not independently prove slug/UUID correspondence.
#[derive(Clone)]
pub(crate) struct ProductStorefrontIndexShadowExecutor {
    product: ProductCatalogReadRuntime,
    index: SharedIndexQueryRuntime,
}

impl ProductStorefrontIndexShadowExecutor {
    pub(crate) fn new(
        product: ProductCatalogReadRuntime,
        index: SharedIndexQueryRuntime,
    ) -> Self {
        Self { product, index }
    }

    pub(crate) async fn execute(
        &self,
        context: PortContext,
        fallback_locale: String,
        public_channel_slug: Option<String>,
        public_channel_id: Option<Uuid>,
        query: StorefrontProductListQuery,
    ) -> Result<ProductStorefrontIndexShadowExecution, PortError> {
        let authoritative = self
            .product
            .read_port()
            .list_filtered_published_products(
                context.clone(),
                FilteredPublishedProductsRequest {
                    locale: Some(context.locale.clone()),
                    fallback_locale: Some(fallback_locale.clone()),
                    public_channel_slug: public_channel_slug.clone(),
                    query: query.clone(),
                },
            )
            .await?;

        let projected = self
            .execute_projected(
                context,
                fallback_locale,
                public_channel_slug,
                public_channel_id,
                query,
            )
            .await;
        let comparison = projected
            .as_ref()
            .ok()
            .map(|projected| compare_owner_and_index(&authoritative, projected));

        Ok(ProductStorefrontIndexShadowExecution {
            authoritative,
            projected,
            comparison,
        })
    }

    async fn execute_projected(
        &self,
        context: PortContext,
        fallback_locale: String,
        public_channel_slug: Option<String>,
        public_channel_id: Option<Uuid>,
        query: StorefrontProductListQuery,
    ) -> Result<IndexQueryPage, ProductStorefrontIndexShadowProjectionError> {
        let public_channel_id = match (
            public_channel_slug.as_deref().map(str::trim),
            public_channel_id,
        ) {
            (Some(slug), Some(channel_id)) if !slug.is_empty() && !channel_id.is_nil() => channel_id,
            _ => {
                return Err(
                    ProductStorefrontIndexShadowProjectionError::PublicChannelIdentityUnavailable,
                );
            }
        };
        let tenant_id = Uuid::parse_str(context.tenant_id.as_str())
            .map_err(|_| ProductStorefrontIndexShadowProjectionError::InvalidTenant)?;
        let schema_read = self
            .product
            .schema_read_port()
            .ok_or(ProductStorefrontIndexShadowProjectionError::SchemaReadPortUnavailable)?;
        let resolved = schema_read
            .resolve_storefront_attribute_filters(
                context.clone(),
                ProductStorefrontAttributeFilterResolutionRequest {
                    fallback_locale: fallback_locale.clone(),
                    filters: query.attribute_filters.clone(),
                },
            )
            .await
            .map_err(ProductStorefrontIndexShadowProjectionError::AttributeFilterResolution)?;
        let index_query = build_product_storefront_index_shadow_query(
            tenant_id,
            context.locale.as_str(),
            fallback_locale.as_str(),
            Some(public_channel_id),
            &query,
            resolved,
        )?;
        self.index
            .execute_localized_query(index_query)
            .await
            .map_err(ProductStorefrontIndexShadowProjectionError::Index)
    }
}

fn compare_owner_and_index(
    authoritative: &StorefrontProductList,
    projected: &IndexQueryPage,
) -> ProductStorefrontIndexShadowComparison {
    let authoritative_ids = authoritative
        .items
        .iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();
    let projected_ids = projected
        .items
        .iter()
        .map(|item| item.entity_id)
        .collect::<Vec<_>>();
    ProductStorefrontIndexShadowComparison {
        identities_match: authoritative_ids == projected_ids,
        exact_count_matches: projected.exact_count == Some(authoritative.total),
        has_more_matches: projected.has_more == authoritative.has_next,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparison_requires_identity_order_count_and_page_boundary() {
        let authoritative = StorefrontProductList {
            items: Vec::new(),
            total: 0,
            page: 1,
            per_page: 12,
            has_next: false,
        };
        let projected = IndexQueryPage {
            items: Vec::new(),
            exact_count: Some(0),
            has_more: false,
            next_cursor: None,
        };
        let comparison = compare_owner_and_index(&authoritative, &projected);
        assert!(comparison.is_match());
    }
}
