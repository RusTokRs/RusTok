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

/// Request-shape decision for the current Product key-4 channel projection.
///
/// `sales_channel_ids` stores resolved Channel UUID membership. For unrestricted Product metadata this is
/// the set of all current Channels, so it cannot distinguish an unrestricted Product from a restricted
/// Product whose allowed slugs currently resolve to that same complete set. Channel-less owner semantics
/// therefore remain owner-native instead of being inferred from membership equality.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProductStorefrontIndexChannelScopeDecision {
    ShadowEligible { public_channel_id: Uuid },
    OwnerNativeChannelLess,
}

#[derive(Debug, Error)]
pub(crate) enum ProductStorefrontIndexShadowProjectionError {
    #[error("Product Storefront shadow schema-read capability is unavailable")]
    SchemaReadPortUnavailable,
    #[error("Product Storefront shadow request tenant identity is invalid")]
    InvalidTenant,
    #[error("Product Storefront channel-less requests remain owner-native for the current Index key-4 contract")]
    ChannelLessOwnerNative,
    #[error("Product Storefront shadow requires a trusted public channel slug/id pair")]
    PublicChannelIdentityUnavailable,
    #[error("Product Storefront shadow attribute-filter owner resolution failed: {0}")]
    AttributeFilterResolution(PortError),
    #[error(transparent)]
    QueryBuild(#[from] ProductStorefrontIndexShadowError),
    #[error(transparent)]
    Index(#[from] IndexQueryExecutionError),
}

/// Classify the caller's public-channel context without guessing channel-less visibility from Index
/// membership.
///
/// An absent/blank slug paired with no UUID is the owner's channel-less shape and is deliberately retained as
/// owner-native. A present non-empty slug plus a non-nil UUID is eligible for channel-scoped shadow
/// translation. Partial, contradictory or nil identities fail closed as malformed context rather than being
/// treated as channel-less.
pub(crate) fn classify_product_storefront_index_channel_scope(
    public_channel_slug: Option<&str>,
    public_channel_id: Option<Uuid>,
) -> Result<ProductStorefrontIndexChannelScopeDecision, ProductStorefrontIndexShadowProjectionError> {
    let public_channel_slug = public_channel_slug.map(str::trim).filter(|slug| !slug.is_empty());
    match (public_channel_slug, public_channel_id) {
        (None, None) => Ok(ProductStorefrontIndexChannelScopeDecision::OwnerNativeChannelLess),
        (Some(_), Some(public_channel_id)) if !public_channel_id.is_nil() => {
            Ok(ProductStorefrontIndexChannelScopeDecision::ShadowEligible { public_channel_id })
        }
        _ => Err(ProductStorefrontIndexShadowProjectionError::PublicChannelIdentityUnavailable),
    }
}

/// Owner-first, non-serving Product Storefront shadow executor.
///
/// This object composes only host-selected Product and Index capabilities. It never constructs
/// `CatalogService`, `ProductCatalogSchemaService`, a PostgreSQL Index port, or a database connection.
/// The owner list result remains authoritative even when Product metadata resolution, shadow query build,
/// Index readiness/admission, or Index execution fails.
///
/// Channel-scoped projection requires a trusted current slug/UUID pair supplied by the caller's current
/// channel context. Channel-less requests are intentionally retained as typed owner-native projected results
/// for the current key-4 schema rather than approximated from `sales_channel_ids`.
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
        let public_channel_id = match classify_product_storefront_index_channel_scope(
            public_channel_slug.as_deref(),
            public_channel_id,
        )? {
            ProductStorefrontIndexChannelScopeDecision::ShadowEligible { public_channel_id } => {
                public_channel_id
            }
            ProductStorefrontIndexChannelScopeDecision::OwnerNativeChannelLess => {
                return Err(ProductStorefrontIndexShadowProjectionError::ChannelLessOwnerNative);
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

    #[test]
    fn channel_scope_distinguishes_owner_native_channel_less_from_invalid_identity() {
        assert_eq!(
            classify_product_storefront_index_channel_scope(None, None).unwrap(),
            ProductStorefrontIndexChannelScopeDecision::OwnerNativeChannelLess
        );
        assert_eq!(
            classify_product_storefront_index_channel_scope(Some("   "), None).unwrap(),
            ProductStorefrontIndexChannelScopeDecision::OwnerNativeChannelLess
        );

        let channel_id = Uuid::new_v4();
        assert_eq!(
            classify_product_storefront_index_channel_scope(Some(" web "), Some(channel_id))
                .unwrap(),
            ProductStorefrontIndexChannelScopeDecision::ShadowEligible { public_channel_id: channel_id }
        );

        assert!(matches!(
            classify_product_storefront_index_channel_scope(Some("web"), None),
            Err(ProductStorefrontIndexShadowProjectionError::PublicChannelIdentityUnavailable)
        ));
        assert!(matches!(
            classify_product_storefront_index_channel_scope(None, Some(channel_id)),
            Err(ProductStorefrontIndexShadowProjectionError::PublicChannelIdentityUnavailable)
        ));
        assert!(matches!(
            classify_product_storefront_index_channel_scope(Some("web"), Some(Uuid::nil())),
            Err(ProductStorefrontIndexShadowProjectionError::PublicChannelIdentityUnavailable)
        ));
    }
}
