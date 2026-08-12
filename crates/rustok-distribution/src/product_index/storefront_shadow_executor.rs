use thiserror::Error;
use uuid::Uuid;

use rustok_api::{PortContext, PortError};
use rustok_index::{
    IndexQueryExecutionError, IndexQueryPage, IndexQueryPort, SharedIndexQueryRuntime,
};
use rustok_product::{
    FilteredPublishedProductsRequest, ProductCatalogReadRuntime,
    ProductStorefrontAttributeFilterResolutionRequest, ProductStorefrontTagHydration,
    ProductStorefrontTagHydrationRequest, StorefrontProductList, StorefrontProductListQuery,
};

use super::{
    ProductStorefrontIndexPublicProjectionError, ProductStorefrontIndexShadowError,
    build_product_storefront_index_shadow_query, project_product_storefront_index_page,
};

const MAX_INDEX_OFFSET_DEPTH: u64 = 10_000;

/// Non-serving Product Storefront parity execution result.
///
/// The authoritative owner result is always produced first. `projected` retains the raw generic Index page
/// for identity/count evidence. `public_projected` derives Product title/handle placeholders from that page.
/// `tag_hydration` is a separate Product-owned post-page read keyed only by identities from the raw Index page.
#[derive(Debug)]
pub(crate) struct ProductStorefrontIndexShadowExecution {
    pub(crate) authoritative: StorefrontProductList,
    pub(crate) projected: Result<IndexQueryPage, ProductStorefrontIndexShadowProjectionError>,
    pub(crate) public_projected:
        Option<Result<IndexQueryPage, ProductStorefrontIndexPublicProjectionError>>,
    pub(crate) tag_hydration:
        Option<Result<ProductStorefrontTagHydration, ProductStorefrontIndexTagHydrationError>>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProductStorefrontIndexChannelScopeDecision {
    ShadowEligible { public_channel_id: Uuid },
    OwnerNativeChannelLess,
}

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

#[derive(Debug, Error)]
pub(crate) enum ProductStorefrontIndexTagHydrationError {
    #[error("Product Storefront tag hydration capability is unavailable")]
    TagReadPortUnavailable,
    #[error("Product Storefront tag hydration owner read failed: {0}")]
    Owner(PortError),
}

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
/// All enrichment happens only after a successful raw Index page. Product public placeholder projection and
/// Product-owned tag hydration are retained separately; neither can change raw Index identity/order/count.
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
                context.clone(),
                fallback_locale.clone(),
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
        let tag_hydration = match projected.as_ref() {
            Ok(projected) => Some(
                self.hydrate_projected_tags(context, fallback_locale, projected)
                    .await,
            ),
            Err(_) => None,
        };
        let comparison = projected
            .as_ref()
            .ok()
            .map(|projected| compare_owner_and_index(&authoritative, projected));

        Ok(ProductStorefrontIndexShadowExecution {
            authoritative,
            projected,
            public_projected,
            tag_hydration,
            comparison,
        })
    }

    pub(crate) async fn hydrate_projected_tags(
        &self,
        context: PortContext,
        fallback_locale: String,
        projected: &IndexQueryPage,
    ) -> Result<ProductStorefrontTagHydration, ProductStorefrontIndexTagHydrationError> {
        let tag_read = self
            .product
            .storefront_tag_read_port()
            .ok_or(ProductStorefrontIndexTagHydrationError::TagReadPortUnavailable)?;
        let product_ids = projected
            .items
            .iter()
            .map(|item| item.entity_id)
            .collect::<Vec<_>>();
        tag_read
            .hydrate_storefront_product_tags(
                context,
                ProductStorefrontTagHydrationRequest {
                    product_ids,
                    fallback_locale,
                },
            )
            .await
            .map_err(ProductStorefrontIndexTagHydrationError::Owner)
    }

    pub(crate) async fn execute_projected(
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
    }
}
