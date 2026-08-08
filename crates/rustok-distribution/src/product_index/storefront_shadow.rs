use thiserror::Error;
use uuid::Uuid;

use rustok_index::{
    EntityName, FieldName, FieldPath, FilterExpr, IndexQuery, IndexQueryScope, IndexValue,
    LocaleKey, LocalizedEntityQuery, ModuleName, OrderDirection, OrderExpr, Pagination, SchemaRef,
    SchemaVersion,
};
use rustok_product::{
    StorefrontProductListQuery, StorefrontProductSortBy, StorefrontProductSortDirection,
    entities::product::ProductStatus,
};

use super::PRODUCT_SCHEMA_ROUTING_KEY;

const MAX_TEXT_LIKE_PATTERN_BYTES: usize = 1024;
const MAX_INDEX_OFFSET_DEPTH: u64 = 10_000;
const MAX_STOREFRONT_ATTRIBUTE_FILTERS: usize = 8;

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum ProductStorefrontIndexShadowError {
    #[error("Product Storefront Index shadow query requires a non-nil tenant")]
    NilTenant,
    #[error("Product Storefront Index shadow query requires a current public channel id")]
    PublicChannelRequired,
    #[error("Product Storefront Index shadow query received a nil public channel id")]
    NilPublicChannel,
    #[error("Product Storefront Index shadow query locale contract is invalid")]
    InvalidLocale,
    #[error("Product Storefront Index shadow query schema contract is invalid")]
    InvalidSchemaContract,
    #[error("Product Storefront Index shadow query pagination is invalid")]
    InvalidPagination,
    #[error("Product Storefront Index shadow query offset exceeds the Index bounded offset depth")]
    OffsetTooDeep,
    #[error("Product Storefront Index shadow query title pattern exceeds the bounded TextLike contract")]
    SearchPatternTooLong,
    #[error("Product Storefront Index shadow query title pattern contains a NUL byte")]
    SearchPatternContainsNul,
    #[error("resolved Product attribute filter count does not match owner Storefront filter count")]
    AttributeFilterResolutionMismatch,
    #[error("resolved Product attribute filter is not a canonical attribute_terms predicate")]
    InvalidAttributeTermPredicate,
}

/// Build the Product Storefront query used only for shadow/equivalence work.
///
/// The caller must resolve every public Product attribute filter through a Product-owned metadata
/// capability before calling this function. `canonical_attribute_filters` are accepted only when every
/// leaf is `Contains(attribute_terms, String(term))`; localized text fallback may compose those leaves
/// with And/Or/Not. This keeps Product option/attribute ownership out of the generic Index engine.
///
/// A public channel ID is mandatory in this source slice. The owner channel-less contract means
/// "metadata unrestricted only", while the current `sales_channel_ids` projection cannot distinguish
/// unrestricted from a restricted Product that currently contains every channel. Authoritative
/// channel-less execution therefore remains fail-closed.
pub(crate) fn build_product_storefront_index_shadow_query(
    tenant_id: Uuid,
    requested_locale: &str,
    fallback_locale: &str,
    public_channel_id: Option<Uuid>,
    owner: &StorefrontProductListQuery,
    canonical_attribute_filters: Vec<FilterExpr>,
) -> Result<LocalizedEntityQuery, ProductStorefrontIndexShadowError> {
    if tenant_id.is_nil() {
        return Err(ProductStorefrontIndexShadowError::NilTenant);
    }
    let public_channel_id = public_channel_id
        .ok_or(ProductStorefrontIndexShadowError::PublicChannelRequired)?;
    if public_channel_id.is_nil() {
        return Err(ProductStorefrontIndexShadowError::NilPublicChannel);
    }

    let requested_locale =
        LocaleKey::new(requested_locale).map_err(|_| ProductStorefrontIndexShadowError::InvalidLocale)?;
    let fallback_locale =
        LocaleKey::new(fallback_locale).map_err(|_| ProductStorefrontIndexShadowError::InvalidLocale)?;

    if owner.page == 0 || owner.per_page == 0 || owner.per_page > 48 {
        return Err(ProductStorefrontIndexShadowError::InvalidPagination);
    }
    let offset = owner
        .page
        .checked_sub(1)
        .and_then(|page| page.checked_mul(owner.per_page))
        .ok_or(ProductStorefrontIndexShadowError::InvalidPagination)?;
    if offset > MAX_INDEX_OFFSET_DEPTH {
        return Err(ProductStorefrontIndexShadowError::OffsetTooDeep);
    }
    let limit = u32::try_from(owner.per_page)
        .map_err(|_| ProductStorefrontIndexShadowError::InvalidPagination)?;

    if owner.attribute_filters.len() > MAX_STOREFRONT_ATTRIBUTE_FILTERS
        || canonical_attribute_filters.len() != owner.attribute_filters.len()
        || !canonical_attribute_filters
            .iter()
            .all(is_canonical_attribute_term_predicate)
    {
        return Err(ProductStorefrontIndexShadowError::AttributeFilterResolutionMismatch);
    }

    let mut filters = vec![
        FilterExpr::Eq(
            root_field("status")?,
            IndexValue::String(ProductStatus::Active.to_string()),
        ),
        FilterExpr::IsNull(root_field("published_at")?, false),
        FilterExpr::Contains(
            root_field("sales_channel_ids")?,
            IndexValue::Uuid(public_channel_id),
        ),
    ];
    if let Some(category_id) = owner.category_id {
        if category_id.is_nil() {
            return Err(ProductStorefrontIndexShadowError::InvalidSchemaContract);
        }
        filters.push(FilterExpr::Eq(
            root_field("primary_category_id")?,
            IndexValue::Uuid(category_id),
        ));
    }
    filters.extend(canonical_attribute_filters);

    let any_locale_filter = owner
        .search
        .as_deref()
        .map(str::trim)
        .filter(|search| !search.is_empty())
        .map(|search| {
            let pattern = format!("%{search}%");
            if pattern.len() > MAX_TEXT_LIKE_PATTERN_BYTES {
                return Err(ProductStorefrontIndexShadowError::SearchPatternTooLong);
            }
            if pattern.contains('\0') {
                return Err(ProductStorefrontIndexShadowError::SearchPatternContainsNul);
            }
            Ok(FilterExpr::TextLike(root_field("title")?, pattern))
        })
        .transpose()?;

    let direction = match owner.sort_direction {
        StorefrontProductSortDirection::Asc => OrderDirection::Asc,
        StorefrontProductSortDirection::Desc => OrderDirection::Desc,
    };
    let timestamp_fields = match owner.sort_by {
        StorefrontProductSortBy::PublishedAt => ["published_at", "created_at"],
        StorefrontProductSortBy::CreatedAt => ["created_at", "published_at"],
    };
    let order_by = timestamp_fields
        .into_iter()
        .map(|field| {
            Ok(OrderExpr {
                field: root_field(field)?,
                direction,
            })
        })
        .collect::<Result<Vec<_>, ProductStorefrontIndexShadowError>>()?;

    let query = IndexQuery {
        scope: IndexQueryScope {
            tenant_id,
            locale: Some(requested_locale),
        },
        schema: product_schema_ref()?,
        fields: [
            "id",
            "status",
            "title",
            "handle",
            "seller_id",
            "vendor",
            "product_type",
            "tag_ids",
            "created_at",
            "published_at",
        ]
        .into_iter()
        .map(root_field)
        .collect::<Result<Vec<_>, _>>()?,
        filter: Some(FilterExpr::And(filters)),
        order_by,
        pagination: Pagination::Offset { limit, offset },
        include_exact_count: true,
    };

    Ok(LocalizedEntityQuery::new(
        query,
        Some(fallback_locale),
        any_locale_filter,
    )
    .with_localized_projection_fields([root_field("title")?, root_field("handle")?])
    .with_identity_order_direction(direction))
}

fn product_schema_ref() -> Result<SchemaRef, ProductStorefrontIndexShadowError> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-product")
            .map_err(|_| ProductStorefrontIndexShadowError::InvalidSchemaContract)?,
        entity: EntityName::new("product")
            .map_err(|_| ProductStorefrontIndexShadowError::InvalidSchemaContract)?,
        version: SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY),
    })
}

fn root_field(name: &str) -> Result<FieldPath, ProductStorefrontIndexShadowError> {
    Ok(FieldPath::new(
        FieldName::new(name).map_err(|_| ProductStorefrontIndexShadowError::InvalidSchemaContract)?,
    ))
}

fn is_canonical_attribute_term_predicate(filter: &FilterExpr) -> bool {
    match filter {
        FilterExpr::Contains(path, IndexValue::String(term)) => {
            path.links().is_empty()
                && path.field().as_str() == "attribute_terms"
                && !term.is_empty()
        }
        FilterExpr::And(children) | FilterExpr::Or(children) => {
            !children.is_empty() && children.iter().all(is_canonical_attribute_term_predicate)
        }
        FilterExpr::Not(child) => is_canonical_attribute_term_predicate(child),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_product::ProductAttributeFilter;

    fn owner_query(direction: StorefrontProductSortDirection) -> StorefrontProductListQuery {
        StorefrontProductListQuery::try_from_transport_with_attribute_filters(
            Some(" phone ".to_owned()),
            Some(Uuid::new_v4()),
            Some("published_at"),
            Some(match direction {
                StorefrontProductSortDirection::Asc => "asc",
                StorefrontProductSortDirection::Desc => "desc",
            }),
            vec![ProductAttributeFilter {
                code: "brand".to_owned(),
                value: "acme".to_owned(),
            }],
        )
        .unwrap()
        .with_pagination(2, 12)
    }

    fn attribute_term() -> FilterExpr {
        FilterExpr::Contains(
            root_field("attribute_terms").unwrap(),
            IndexValue::String("a:00000000-0000-0000-0000-000000000001:t:acme".to_owned()),
        )
    }

    #[test]
    fn maps_owner_filters_projection_order_and_page_to_localized_fold() {
        let tenant = Uuid::new_v4();
        let channel = Uuid::new_v4();
        let owner = owner_query(StorefrontProductSortDirection::Desc);
        let query = build_product_storefront_index_shadow_query(
            tenant,
            "fi",
            "en",
            Some(channel),
            &owner,
            vec![attribute_term()],
        )
        .unwrap();

        assert_eq!(query.query.scope.tenant_id, tenant);
        assert_eq!(query.identity_order_direction, OrderDirection::Desc);
        assert_eq!(query.query.order_by.len(), 2);
        assert!(query.query.order_by.iter().all(|order| order.direction == OrderDirection::Desc));
        assert_eq!(query.query.order_by[0].field.field().as_str(), "published_at");
        assert_eq!(query.query.order_by[1].field.field().as_str(), "created_at");
        assert!(matches!(
            query.query.pagination,
            Pagination::Offset { limit: 12, offset: 12 }
        ));
        assert!(matches!(query.any_locale_filter, Some(FilterExpr::TextLike(_, _))));
        assert_eq!(query.localized_projection_fields.len(), 2);
    }

    #[test]
    fn fails_closed_without_public_channel_or_resolved_attribute_terms() {
        let owner = owner_query(StorefrontProductSortDirection::Asc);
        assert_eq!(
            build_product_storefront_index_shadow_query(
                Uuid::new_v4(),
                "fi",
                "en",
                None,
                &owner,
                vec![attribute_term()],
            ),
            Err(ProductStorefrontIndexShadowError::PublicChannelRequired)
        );
        assert_eq!(
            build_product_storefront_index_shadow_query(
                Uuid::new_v4(),
                "fi",
                "en",
                Some(Uuid::new_v4()),
                &owner,
                Vec::new(),
            ),
            Err(ProductStorefrontIndexShadowError::AttributeFilterResolutionMismatch)
        );
    }

    #[test]
    fn fails_closed_when_owner_page_exceeds_bounded_index_offset() {
        let mut owner = owner_query(StorefrontProductSortDirection::Asc);
        owner.page = 1_000;
        assert_eq!(
            build_product_storefront_index_shadow_query(
                Uuid::new_v4(),
                "fi",
                "en",
                Some(Uuid::new_v4()),
                &owner,
                vec![attribute_term()],
            ),
            Err(ProductStorefrontIndexShadowError::OffsetTooDeep)
        );
    }
}
