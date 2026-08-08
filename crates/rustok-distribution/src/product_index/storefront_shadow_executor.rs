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
    ProductStorefrontIndexPublicProjectionError, ProductStorefrontIndexShadowError,
    build_product_storefront_index_shadow_query, project_product_storefront_index_page,
};

const MAX_INDEX_OFFSET_DEPTH: u64 = 10_000;

/// Non-serving Product Storefront parity execution result.
///
/// The authoritative owner result is always produced first. `projected` retains the raw generic Index page
/// for identity/count evidence. `public_projected` is derived only after that page exists and applies the
/// Product owner public placeholder contract without feeding values back into Index query semantics.
#[derive(Debug)]
pub(crate) struct ProductStorefrontIndexShadowExecution {
    pub(crate) authoritative: StorefrontProductList,
    pub(crate) projected: Result<IndexQueryPage, ProductStorefrontIndexShadowProjectionError>,
    pub(crate) public_projected:
        Option<Result<IndexQueryPage, ProductStorefrontIndexPublicProjectionError>>,
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

/// Request-shape decision for owner-valid Product Storefront offset pagination.
///
/// The Product owner has no explicit maximum offset, while the generic Index offset contract is bounded at
/// 10,000. Requests inside that bound remain shadow-eligible. Owner-valid deeper pages remain owner-native;
/// they are never clamped or rewritten to cursor pagination because either would change owner semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProductStorefrontIndexPageScopeDecision {
    ShadowEligible { offset: u64 },
    OwnerNativeDeepPage { offset: u64 },
}

#[derive(Debug, Error)]
pub(crate) enum ProductStorefrontIndexShadowProjectionError {
    #[error("Product Storefront shadow schema-read capability is unavailable")]
    SchemaReadPortUnavailable,
    #[error("Product Storefront shadow request tenant identity is invalid")]
    InvalidTenant,
    #[error("Product Storefront channel-less requests remain owner-native for the current Index key-4 contract")]
    ChannelLessOwnerNative,
    #[error("Product Storefront deep page at offset {offset} remains owner-native beyond the Index offset bound")]
    DeepPageOwnerNative { offset: u64 },
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

/// Classify Product owner offset pagination before projected metadata or Index work begins.
///
/// Invalid pagination remains an error and is not treated as an owner-native fallback shape. A valid offset
/// above the generic Index bound is a deliberate owner-native request shape. The pure shadow query builder
/// retains its own `OffsetTooDeep` fail-closed check as a second boundary for direct callers.
pub(crate) fn classify_product_storefront_index_page_scope(
    query: &StorefrontProductListQuery,
) -> Result<ProductStorefrontIndexPageScopeDecision, ProductStorefrontIndexShadowProjectionError> {
    if query.page == 0 || query.per_page == 0 || query.per_page > 48 {
        return Err(ProductStorefrontIndexShadowError::InvalidPagination.into());
    }
    let offset = query
        .page
        .checked_sub(1)
        .and_then(|page| page.checked_mul(query.per_page))
        .ok_or(ProductStorefrontIndexShadowError::InvalidPagination)?;
    if offset > MAX_INDEX_OFFSET_DEPTH {
        Ok(ProductStorefrontIndexPageScopeDecision::OwnerNativeDeepPage { offset })
    } else {
        Ok(ProductStorefrontIndexPageScopeDecision::ShadowEligible { offset })
    }
}

/// Owner-first, non-serving Product Storefront shadow executor.
///
/// This object composes only host-selected Product and Index capabilities. It never constructs
/// `CatalogService`, `ProductCatalogSchemaService`, a PostgreSQL Index port, or a database connection.
/// The owner list result remains authoritative even when Product metadata resolution, shadow query build,
/// Index readiness/admission, Index execution, or public post-page projection fails.
///
/// Channel-scoped projection requires a trusted current slug/UUID pair supplied by the caller's current
/// channel context. Channel-less requests and owner-valid deep offset pages are intentionally retained as
/// typed owner-native projected results rather than approximated by Index.
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
        let public_projected = projected
            .as_ref()
            .ok()
            .cloned()
            .map(project_product_storefront_index_page);
        let comparison = projected
            .as_ref()
            .ok()
            .map(|projected| compare_owner_and_index(&authoritative, projected));

        Ok(ProductStorefrontIndexShadowExecution {
            authoritative,
            projected,
            public_projected,
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
        match classify_product_storefront_index_page_scope(&query)? {
            ProductStorefrontIndexPageScopeDecision::ShadowEligible { .. } => {}
            ProductStorefrontIndexPageScopeDecision::OwnerNativeDeepPage { offset } => {
                return Err(ProductStorefrontIndexShadowProjectionError::DeepPageOwnerNative {
                    offset,
                });
            }
        }
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

    #[test]
    fn page_scope_distinguishes_shallow_from_owner_native_deep_pages() {
        let shallow = StorefrontProductListQuery::default().with_pagination(209, 48);
        assert_eq!(
            classify_product_storefront_index_page_scope(&shallow).unwrap(),
            ProductStorefrontIndexPageScopeDecision::ShadowEligible { offset: 9_984 }
        );

        let deep = StorefrontProductListQuery::default().with_pagination(210, 48);
        assert_eq!(
            classify_product_storefront_index_page_scope(&deep).unwrap(),
            ProductStorefrontIndexPageScopeDecision::OwnerNativeDeepPage { offset: 10_032 }
        );

        let invalid = StorefrontProductListQuery::default().with_pagination(0, 48);
        assert!(matches!(
            classify_product_storefront_index_page_scope(&invalid),
            Err(ProductStorefrontIndexShadowProjectionError::QueryBuild(
                ProductStorefrontIndexShadowError::InvalidPagination
            ))
        ));

        let overflow = StorefrontProductListQuery::default().with_pagination(u64::MAX, 48);
        assert!(matches!(
            classify_product_storefront_index_page_scope(&overflow),
            Err(ProductStorefrontIndexShadowProjectionError::QueryBuild(
                ProductStorefrontIndexShadowError::InvalidPagination
            ))
        ));
    }
}
