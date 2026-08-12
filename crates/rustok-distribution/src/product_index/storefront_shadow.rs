use thiserror::Error;
use uuid::Uuid;

use rustok_index::{
    EntityName, FieldName, FieldPath, FilterExpr, IndexQuery, IndexQueryScope, IndexValue,
    LocaleKey, LocalizedEntityQuery, ModuleName, OrderDirection, OrderExpr, Pagination, SchemaRef,
    SchemaVersion,
};
use rustok_product::{
    ProductAttributeTermExpr, ProductResolvedAttributeFilter, StorefrontProductListQuery,
    StorefrontProductSortBy, StorefrontProductSortDirection, entities::product::ProductStatus,
    services::MAX_STOREFRONT_PRODUCT_SEARCH_BYTES,
};

use super::PRODUCT_SCHEMA_ROUTING_KEY;

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
    #[error(
        "Product Storefront Index shadow query title pattern exceeds the bounded TextLike contract"
    )]
    SearchPatternTooLong,
    #[error("Product Storefront Index shadow query title pattern contains a NUL byte")]
    SearchPatternContainsNul,
    #[error(
        "resolved Product attribute filters do not match the owner Storefront filter identities"
    )]
    AttributeFilterResolutionMismatch,
    #[error("resolved Product attribute term expression is invalid")]
    InvalidAttributeTermPredicate,
}

/// Build the Product Storefront query used only for shadow/equivalence work.
///
/// `resolved_attribute_filters` must come from the Product-owned Storefront filter resolution
/// capability. Distribution translates the neutral Product expression shape into Index predicates but
/// never resolves Product attribute codes, option codes or storage identities itself.
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
    resolved_attribute_filters: Vec<ProductResolvedAttributeFilter>,
) -> Result<LocalizedEntityQuery, ProductStorefrontIndexShadowError> {
    if tenant_id.is_nil() {
        return Err(ProductStorefrontIndexShadowError::NilTenant);
    }
    let public_channel_id =
        public_channel_id.ok_or(ProductStorefrontIndexShadowError::PublicChannelRequired)?;
    if public_channel_id.is_nil() {
        return Err(ProductStorefrontIndexShadowError::NilPublicChannel);
    }

    let requested_locale = LocaleKey::new(requested_locale)
        .map_err(|_| ProductStorefrontIndexShadowError::InvalidLocale)?;
    let fallback_locale = LocaleKey::new(fallback_locale)
        .map_err(|_| ProductStorefrontIndexShadowError::InvalidLocale)?;

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

    let attribute_filters = resolved_attribute_filters_to_index(owner, resolved_attribute_filters)?;

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
        filters.push(FilterExpr::Eq(
            root_field("primary_category_id")?,
            IndexValue::Uuid(category_id),
        ));
    }
    filters.extend(attribute_filters);

    let any_locale_filter = owner
        .search
        .as_deref()
        .map(str::trim)
        .filter(|search| !search.is_empty())
        .map(|search| {
            if search.len() > MAX_STOREFRONT_PRODUCT_SEARCH_BYTES {
                return Err(ProductStorefrontIndexShadowError::SearchPatternTooLong);
            }
            let pattern = format!("%{search}%");
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

    Ok(
        LocalizedEntityQuery::new(query, Some(fallback_locale), any_locale_filter)
            .with_localized_projection_fields([root_field("title")?, root_field("handle")?])
            .with_identity_order_direction(direction),
    )
}

fn resolved_attribute_filters_to_index(
    owner: &StorefrontProductListQuery,
    resolved: Vec<ProductResolvedAttributeFilter>,
) -> Result<Vec<FilterExpr>, ProductStorefrontIndexShadowError> {
    if owner.attribute_filters.len() > MAX_STOREFRONT_ATTRIBUTE_FILTERS
        || resolved.len() != owner.attribute_filters.len()
        || owner
            .attribute_filters
            .iter()
            .zip(&resolved)
            .any(|(owner, resolved)| !owner.code.eq_ignore_ascii_case(resolved.code.as_str()))
    {
        return Err(ProductStorefrontIndexShadowError::AttributeFilterResolutionMismatch);
    }

    resolved
        .into_iter()
        .map(|resolved| product_term_expr_to_index(resolved.predicate))
        .collect()
}

fn product_term_expr_to_index(
    expression: ProductAttributeTermExpr,
) -> Result<FilterExpr, ProductStorefrontIndexShadowError> {
    match expression {
        ProductAttributeTermExpr::Term(term) => {
            if term.is_empty() {
                return Err(ProductStorefrontIndexShadowError::InvalidAttributeTermPredicate);
            }
            Ok(FilterExpr::Contains(
                root_field("attribute_terms")?,
                IndexValue::String(term),
            ))
        }
        ProductAttributeTermExpr::And(children) => {
            if children.is_empty() {
                return Err(ProductStorefrontIndexShadowError::InvalidAttributeTermPredicate);
            }
            Ok(FilterExpr::And(
                children
                    .into_iter()
                    .map(product_term_expr_to_index)
                    .collect::<Result<Vec<_>, _>>()?,
            ))
        }
        ProductAttributeTermExpr::Or(children) => {
            if children.is_empty() {
                return Err(ProductStorefrontIndexShadowError::InvalidAttributeTermPredicate);
            }
            Ok(FilterExpr::Or(
                children
                    .into_iter()
                    .map(product_term_expr_to_index)
                    .collect::<Result<Vec<_>, _>>()?,
            ))
        }
        ProductAttributeTermExpr::Not(child) => Ok(FilterExpr::Not(Box::new(
            product_term_expr_to_index(*child)?,
        ))),
        ProductAttributeTermExpr::Never => {
            // Product ids are required/non-null in the current schema. Negating that invariant is a
            // generic, bind-free false predicate without inventing a Product-specific sentinel term.
            Ok(FilterExpr::Not(Box::new(FilterExpr::IsNull(
                root_field("id")?,
                false,
            ))))
        }
    }
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
    Ok(FieldPath::new(FieldName::new(name).map_err(|_| {
        ProductStorefrontIndexShadowError::InvalidSchemaContract
    })?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_product::ProductResolvedAttributeFilter;

    fn owner_query(direction: StorefrontProductSortDirection) -> StorefrontProductListQuery {
        StorefrontProductListQuery::try_from_transport_with_attribute_filters(
            Some(" phone ".to_owned()),
            Some(Uuid::new_v4().to_string()),
            Some("published_at".to_owned()),
            Some(match direction {
                StorefrontProductSortDirection::Asc => "asc".to_owned(),
                StorefrontProductSortDirection::Desc => "desc".to_owned(),
            }),
            vec!["brand=acme".to_owned()],
        )
        .unwrap()
        .with_pagination(2, 12)
    }

    fn attribute_term() -> ProductResolvedAttributeFilter {
        ProductResolvedAttributeFilter {
            code: "brand".to_owned(),
            predicate: ProductAttributeTermExpr::Term(
                "00000000-0000-0000-0000-000000000001|text||61636d65".to_owned(),
            ),
        }
    }

    #[test]
    fn maps_product_owned_terms_projection_order_and_page_to_localized_fold() {
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
        assert!(
            query
                .query
                .order_by
                .iter()
                .all(|order| order.direction == OrderDirection::Desc)
        );
        assert_eq!(
            query.query.order_by[0].field.field().as_str(),
            "published_at"
        );
        assert_eq!(query.query.order_by[1].field.field().as_str(), "created_at");
        assert!(matches!(
            query.query.pagination,
            Pagination::Offset {
                limit: 12,
                offset: 12
            }
        ));
        assert!(matches!(
            query.any_locale_filter,
            Some(FilterExpr::TextLike(_, _))
        ));
        assert_eq!(query.localized_projection_fields.len(), 2);
    }

    #[test]
    fn product_never_expression_becomes_false_root_predicate() {
        let owner = owner_query(StorefrontProductSortDirection::Asc);
        let resolved = ProductResolvedAttributeFilter {
            code: "brand".to_owned(),
            predicate: ProductAttributeTermExpr::Never,
        };
        let query = build_product_storefront_index_shadow_query(
            Uuid::new_v4(),
            "fi",
            "en",
            Some(Uuid::new_v4()),
            &owner,
            vec![resolved],
        )
        .unwrap();
        let Some(FilterExpr::And(filters)) = query.query.filter else {
            panic!("shadow query root filter must be AND");
        };
        assert!(
            filters
                .iter()
                .any(|filter| matches!(filter, FilterExpr::Not(_)))
        );
    }

    #[test]
    fn fails_closed_without_public_channel_or_matching_owner_resolution() {
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
        let wrong = ProductResolvedAttributeFilter {
            code: "other".to_owned(),
            predicate: ProductAttributeTermExpr::Term("term".to_owned()),
        };
        assert_eq!(
            build_product_storefront_index_shadow_query(
                Uuid::new_v4(),
                "fi",
                "en",
                Some(Uuid::new_v4()),
                &owner,
                vec![wrong],
            ),
            Err(ProductStorefrontIndexShadowError::AttributeFilterResolutionMismatch)
        );
    }

    #[test]
    fn rejects_empty_product_term_expression() {
        let owner = owner_query(StorefrontProductSortDirection::Asc);
        let invalid = ProductResolvedAttributeFilter {
            code: "brand".to_owned(),
            predicate: ProductAttributeTermExpr::Or(Vec::new()),
        };
        assert_eq!(
            build_product_storefront_index_shadow_query(
                Uuid::new_v4(),
                "fi",
                "en",
                Some(Uuid::new_v4()),
                &owner,
                vec![invalid],
            ),
            Err(ProductStorefrontIndexShadowError::InvalidAttributeTermPredicate)
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

    #[test]
    fn fails_closed_when_public_query_fields_bypass_owner_search_constructor_bound() {
        let mut owner = owner_query(StorefrontProductSortDirection::Asc);
        owner.search = Some("a".repeat(MAX_STOREFRONT_PRODUCT_SEARCH_BYTES + 1));
        assert_eq!(
            build_product_storefront_index_shadow_query(
                Uuid::new_v4(),
                "fi",
                "en",
                Some(Uuid::new_v4()),
                &owner,
                vec![attribute_term()],
            ),
            Err(ProductStorefrontIndexShadowError::SearchPatternTooLong)
        );
    }
}
