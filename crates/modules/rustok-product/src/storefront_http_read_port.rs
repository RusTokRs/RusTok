use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, locale_tags_match};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::entities::{product, product_translation};
use crate::ports::{LegacyStorefrontProductList, LegacyStorefrontProductListItem};
use crate::ports::{product_owner_error_facts, product_port_context_facts};
use crate::{CatalogService, CommerceError};

const MAX_LEGACY_STOREFRONT_HTTP_PRODUCTS_PER_PAGE: u64 = 100;
const LIST_LEGACY_STOREFRONT_HTTP_PRODUCTS_OPERATION: &str = "list_legacy_storefront_http_products";

/// Optional Product-owned compatibility boundary for the mounted legacy storefront REST list.
///
/// This capability is intentionally separate from the legacy GraphQL list projection because the
/// mounted REST contract allows up to 100 rows, paginates after public-channel visibility, emits an
/// empty title when no translation exists, and derives the shipping profile from metadata only.
#[async_trait]
pub trait ProductStorefrontHttpReadPort: Send + Sync {
    async fn list_legacy_storefront_http_products(
        &self,
        context: PortContext,
        request: LegacyStorefrontHttpProductsRequest,
    ) -> Result<LegacyStorefrontProductList, PortError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyStorefrontHttpProductsRequest {
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub public_channel_slug: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub search: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

#[async_trait]
impl ProductStorefrontHttpReadPort for CatalogService {
    async fn list_legacy_storefront_http_products(
        &self,
        context: PortContext,
        request: LegacyStorefrontHttpProductsRequest,
    ) -> Result<LegacyStorefrontProductList, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        validate_request(&context, &request)?;
        let tenant_id = Uuid::parse_str(&context.tenant_id).map_err(|_| {
            PortError::validation(
                "product.tenant_id_invalid",
                "product request context is invalid",
            )
        })?;
        let locale = request.locale.as_deref().unwrap_or(context.locale.as_str());

        list_legacy_storefront_http_products(
            self,
            tenant_id,
            locale,
            request.fallback_locale.as_deref(),
            request.public_channel_slug.as_deref(),
            request.vendor.as_deref(),
            request.product_type.as_deref(),
            request.search.as_deref(),
            request.page,
            request.per_page,
        )
        .await
        .map_err(|error| map_product_error(&context, error))
    }
}

fn validate_request(
    context: &PortContext,
    request: &LegacyStorefrontHttpProductsRequest,
) -> Result<(), PortError> {
    if request.page == 0 {
        let context_facts = product_port_context_facts(context);
        tracing::warn!(
            operation = LIST_LEGACY_STOREFRONT_HTTP_PRODUCTS_OPERATION,
            correlation_id_length = context_facts.correlation_id_length,
            tenant_id_length = context_facts.tenant_id_length,
            page_zero = request.page == 0,
            per_page_zero = request.per_page == 0,
            per_page_exceeds_limit =
                request.per_page > MAX_LEGACY_STOREFRONT_HTTP_PRODUCTS_PER_PAGE,
            code = "product.page_invalid",
            "legacy storefront HTTP product page validation failed"
        );
        return Err(PortError::validation(
            "product.page_invalid",
            "published products page is invalid",
        ));
    }
    if !(1..=MAX_LEGACY_STOREFRONT_HTTP_PRODUCTS_PER_PAGE).contains(&request.per_page) {
        let context_facts = product_port_context_facts(context);
        tracing::warn!(
            operation = LIST_LEGACY_STOREFRONT_HTTP_PRODUCTS_OPERATION,
            correlation_id_length = context_facts.correlation_id_length,
            tenant_id_length = context_facts.tenant_id_length,
            page_zero = request.page == 0,
            per_page_zero = request.per_page == 0,
            per_page_exceeds_limit =
                request.per_page > MAX_LEGACY_STOREFRONT_HTTP_PRODUCTS_PER_PAGE,
            max_per_page = MAX_LEGACY_STOREFRONT_HTTP_PRODUCTS_PER_PAGE,
            code = "product.per_page_invalid",
            "legacy storefront HTTP product page-size validation failed"
        );
        return Err(PortError::validation(
            "product.per_page_invalid",
            "published products page size is invalid",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn list_legacy_storefront_http_products(
    service: &CatalogService,
    tenant_id: Uuid,
    locale: &str,
    fallback_locale: Option<&str>,
    public_channel_slug: Option<&str>,
    vendor: Option<&str>,
    product_type: Option<&str>,
    search: Option<&str>,
    page: u64,
    per_page: u64,
) -> Result<LegacyStorefrontProductList, CommerceError> {
    let fallback_locale = fallback_locale.unwrap_or(rustok_api::PLATFORM_FALLBACK_LOCALE);
    let offset = page
        .checked_sub(1)
        .and_then(|page| page.checked_mul(per_page))
        .ok_or_else(|| CommerceError::Validation("page and per_page are too large".to_owned()))?;
    let offset_usize = usize::try_from(offset)
        .map_err(|_| CommerceError::Validation("page and per_page are too large".to_owned()))?;
    let db = service.database();

    let mut query = product::Entity::find()
        .filter(product::Column::TenantId.eq(tenant_id))
        .filter(product::Column::Status.eq(product::ProductStatus::Active))
        .filter(product::Column::PublishedAt.is_not_null());
    if let Some(vendor) = vendor {
        query = query.filter(product::Column::Vendor.eq(vendor));
    }
    if let Some(product_type) = product_type {
        query = query.filter(product::Column::ProductType.eq(product_type));
    }
    if let Some(search) = search {
        query = query.filter(product_title_search_condition(
            db.get_database_backend(),
            tenant_id,
            search,
        ));
    }

    let visible_products = query
        .order_by_desc(product::Column::PublishedAt)
        .order_by_desc(product::Column::CreatedAt)
        .all(db)
        .await?
        .into_iter()
        .filter(|product| {
            rustok_inventory::is_metadata_visible_for_public_channel(
                &product.metadata,
                public_channel_slug,
            )
        })
        .collect::<Vec<_>>();
    let total = visible_products.len() as u64;
    let products = visible_products
        .into_iter()
        .skip(offset_usize)
        .take(per_page as usize)
        .collect::<Vec<_>>();

    let product_ids = products
        .iter()
        .map(|product| product.id)
        .collect::<Vec<_>>();
    let translations = if product_ids.is_empty() {
        Vec::new()
    } else {
        product_translation::Entity::find()
            .filter(product_translation::Column::TenantId.eq(tenant_id))
            .filter(product_translation::Column::ProductId.is_in(product_ids))
            .all(db)
            .await?
    };
    let mut translations_by_product =
        std::collections::HashMap::<Uuid, Vec<product_translation::Model>>::new();
    for translation in translations {
        translations_by_product
            .entry(translation.product_id)
            .or_default()
            .push(translation);
    }
    let product_tags = service
        .load_product_tag_map(tenant_id, &products, locale, Some(fallback_locale))
        .await?;

    let items = products
        .into_iter()
        .map(|product| {
            let translation = translations_by_product.get(&product.id).and_then(|items| {
                pick_product_translation(items.as_slice(), locale, fallback_locale)
            });
            LegacyStorefrontProductListItem {
                id: product.id,
                status: product.status,
                title: translation
                    .map(|value| value.title.clone())
                    .unwrap_or_default(),
                handle: translation
                    .map(|value| value.handle.clone())
                    .unwrap_or_default(),
                seller_id: product.seller_id,
                vendor: product.vendor,
                product_type: product.product_type,
                shipping_profile_slug: shipping_profile_slug_from_metadata(&product.metadata),
                tags: product_tags.get(&product.id).cloned().unwrap_or_default(),
                created_at: product.created_at.into(),
                published_at: product.published_at.map(Into::into),
            }
        })
        .collect::<Vec<_>>();

    let items_len = items.len() as u64;
    Ok(LegacyStorefrontProductList {
        items,
        total,
        page,
        per_page,
        has_next: offset
            .checked_add(items_len)
            .is_some_and(|through| through < total),
    })
}

fn pick_product_translation<'a>(
    translations: &'a [product_translation::Model],
    locale: &str,
    fallback_locale: &str,
) -> Option<&'a product_translation::Model> {
    translations
        .iter()
        .find(|translation| locale_tags_match(&translation.locale, locale))
        .or_else(|| {
            (!locale_tags_match(fallback_locale, locale)).then(|| {
                translations
                    .iter()
                    .find(|translation| locale_tags_match(&translation.locale, fallback_locale))
            })?
        })
        .or_else(|| translations.first())
}

fn shipping_profile_slug_from_metadata(metadata: &serde_json::Value) -> String {
    metadata
        .get("shipping_profile")
        .and_then(|profile| profile.get("slug"))
        .and_then(serde_json::Value::as_str)
        .and_then(normalize_shipping_profile_slug)
        .or_else(|| {
            metadata
                .get("shipping_profile_slug")
                .and_then(serde_json::Value::as_str)
                .and_then(normalize_shipping_profile_slug)
        })
        .unwrap_or_else(|| "default".to_string())
}

fn normalize_shipping_profile_slug(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    (!normalized.is_empty()).then_some(normalized)
}

fn product_title_search_condition(
    backend: sea_orm::DbBackend,
    tenant_id: Uuid,
    search: &str,
) -> sea_orm::Condition {
    let pattern = format!("%{search}%");
    let exists_sql = match backend {
        sea_orm::DbBackend::Sqlite => {
            "EXISTS (\n                SELECT 1\n                FROM product_translations pt\n                WHERE pt.product_id = products.id\n                  AND pt.tenant_id = ?\n                  AND pt.title LIKE ?\n            )"
        }
        _ => {
            "EXISTS (\n                SELECT 1\n                FROM product_translations pt\n                WHERE pt.product_id = products.id\n                  AND pt.tenant_id = $1\n                  AND pt.title LIKE $2\n            )"
        }
    };

    sea_orm::Condition::all().add(sea_orm::sea_query::Expr::cust_with_values(
        exists_sql,
        vec![
            sea_orm::Value::from(tenant_id),
            sea_orm::Value::from(pattern),
        ],
    ))
}

fn map_product_error(context: &PortContext, error: CommerceError) -> PortError {
    let context_facts = product_port_context_facts(context);
    let error_facts = product_owner_error_facts(&error);
    let classified = match &error {
        CommerceError::Core(error) => Some(crate::error::core_port_error(error)),
        CommerceError::Conflict(_) => Some(PortError::conflict(
            "product.conflict",
            "product request conflicts with the current state",
        )),
        _ => None,
    };
    if let Some(port) = classified {
        tracing::error!(
            owner = "rustok_product",
            operation = LIST_LEGACY_STOREFRONT_HTTP_PRODUCTS_OPERATION,
            correlation_id_length = context_facts.correlation_id_length,
            tenant_id_length = context_facts.tenant_id_length,
            owner_error_variant = error_facts.error_variant,
            text_field_count = error_facts.text_field_count,
            text_total_length = error_facts.text_total_length,
            uuid_field_count = error_facts.uuid_field_count,
            uuid_non_nil_count = error_facts.uuid_non_nil_count,
            opaque_payload_present = error_facts.opaque_payload_present,
            public_code = %port.code,
            retryable = port.retryable,
            "storefront HTTP product dependency read failed"
        );
        return port;
    }
    let (kind, code, message, retryable, variant) = match error {
        CommerceError::Database(_) => (
            "unavailable",
            "product.database_unavailable",
            "product storage is temporarily unavailable",
            true,
            "database",
        ),
        CommerceError::ProductNotFound(_) => (
            "not_found",
            "product.product_not_found",
            "product was not found",
            false,
            "not_found",
        ),
        CommerceError::Validation(_) => (
            "validation",
            "product.validation",
            "product request is invalid",
            false,
            "validation",
        ),
        _ => (
            "invariant",
            "product.invariant_violation",
            "product operation could not be completed safely",
            false,
            "invariant",
        ),
    };
    tracing::error!(
        owner = "rustok_product",
        operation = LIST_LEGACY_STOREFRONT_HTTP_PRODUCTS_OPERATION,
        correlation_id_length = context_facts.correlation_id_length,
        tenant_id_length = context_facts.tenant_id_length,
        error_variant = variant,
        owner_error_variant = error_facts.error_variant,
        text_field_count = error_facts.text_field_count,
        text_total_length = error_facts.text_total_length,
        uuid_field_count = error_facts.uuid_field_count,
        uuid_non_nil_count = error_facts.uuid_non_nil_count,
        opaque_payload_present = error_facts.opaque_payload_present,
        public_code = code,
        retryable,
        "legacy storefront HTTP product owner read failed"
    );
    match kind {
        "unavailable" => PortError::unavailable(code, message),
        "not_found" => PortError::not_found(code, message),
        "validation" => PortError::validation(code, message),
        _ => PortError::invariant_violation(code, message),
    }
}

#[cfg(test)]
mod diagnostic_policy_tests {
    use super::*;
    use rustok_api::{PortActor, PortErrorKind};

    fn context() -> PortContext {
        PortContext::new(
            "private-tenant",
            PortActor::system(),
            "private-locale",
            "private-correlation",
        )
    }

    #[test]
    fn pagination_admission_keeps_bounds_codes_and_safe_public_copy() {
        let mut request = LegacyStorefrontHttpProductsRequest {
            locale: None,
            fallback_locale: None,
            public_channel_slug: None,
            vendor: None,
            product_type: None,
            search: None,
            page: 1,
            per_page: 1,
        };
        for size in [1, MAX_LEGACY_STOREFRONT_HTTP_PRODUCTS_PER_PAGE] {
            request.per_page = size;
            assert!(validate_request(&context(), &request).is_ok());
        }
        for (page, size, code) in [
            (0, 10, "product.page_invalid"),
            (1, 0, "product.per_page_invalid"),
            (
                1,
                MAX_LEGACY_STOREFRONT_HTTP_PRODUCTS_PER_PAGE + 1,
                "product.per_page_invalid",
            ),
            (1, u64::MAX, "product.per_page_invalid"),
        ] {
            request.page = page;
            request.per_page = size;
            let error = validate_request(&context(), &request).unwrap_err();
            assert_eq!(error.kind, PortErrorKind::Validation);
            assert_eq!(error.code, code);
            assert!(!error.retryable);
            assert!(!error.message.contains("private-"));
        }
    }

    #[test]
    fn http_owner_errors_preserve_nested_policy_retryability_and_privacy() {
        use rustok_core::error::Error as CoreError;
        for (error, kind, retryable, code) in [
            (
                CommerceError::Core(CoreError::Cache("private-source".into())),
                PortErrorKind::Unavailable,
                true,
                "product.dependency_unavailable",
            ),
            (
                CommerceError::Core(CoreError::Forbidden("private-source".into())),
                PortErrorKind::Forbidden,
                false,
                "product.forbidden",
            ),
            (
                CommerceError::Conflict("private-source".into()),
                PortErrorKind::Conflict,
                false,
                "product.conflict",
            ),
            (
                CommerceError::Validation("private-source".into()),
                PortErrorKind::Validation,
                false,
                "product.validation",
            ),
            (
                CommerceError::ProductNotFound(Uuid::new_v4()),
                PortErrorKind::NotFound,
                false,
                "product.product_not_found",
            ),
        ] {
            let mapped = map_product_error(&context(), error);
            assert_eq!(mapped.kind, kind);
            assert_eq!(mapped.retryable, retryable);
            assert_eq!(mapped.code, code);
            assert!(!format!("{mapped}").contains("private-"));
        }
    }
}
