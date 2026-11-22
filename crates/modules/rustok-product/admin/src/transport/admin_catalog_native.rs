use leptos::prelude::*;

#[cfg(feature = "ssr")]
use std::time::Duration;

#[cfg(feature = "ssr")]
use rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
#[cfg(feature = "ssr")]
use rustok_product::ports::types::{AdminCatalogFacetsRequest, AdminProductsRequest};
#[cfg(feature = "ssr")]
use rustok_product::ProductCatalogReadPort;

use crate::catalog_controls::ProductAdminListInput;
use crate::model::ProductList;
#[cfg(feature = "ssr")]
use crate::model::ProductListItem;
#[cfg(feature = "ssr")]
use rustok_product::AdminProductListQueryExt;

pub(crate) async fn fetch_products(
    tenant_id: String,
    locale: Option<String>,
    controls: ProductAdminListInput,
) -> Result<ProductList, ServerFnError> {
    product_admin_catalog_list_native(
        tenant_id,
        locale,
        controls.search,
        controls.status,
        controls.vendor,
        controls.category_id,
        controls.sort_by,
        controls.sort_direction,
        controls.attribute_filters,
        controls.page,
        controls.per_page,
    )
    .await
}

#[cfg(feature = "ssr")]
fn map_product_internal_error() -> ServerFnError {
    ServerFnError::new(rustok_product::ProductPublicError::internal().to_string())
}

#[cfg(feature = "ssr")]
async fn require_request_context(
    operation: &'static str,
) -> Result<rustok_api::RequestContext, ServerFnError> {
    leptos_axum::extract::<rustok_api::RequestContext>()
        .await
        .map_err(|_| {
            let public = rustok_product::ProductPublicError::internal();
            tracing::error!(
                owner = "rustok_product.admin",
                owner_operation = operation,
                boundary = "product_admin_catalog_native",
                code = "product.admin_catalog_request_context_unavailable",
                public_code = public.code,
                correlation_id = %public.correlation_id,
                request_context_rejected = true,
                "product admin trusted request context extraction failed"
            );
            ServerFnError::new(public.to_string())
        })
}

#[cfg(feature = "ssr")]
fn map_product_service_error(
    error: rustok_product::CommerceError,
    operation: &'static str,
) -> ServerFnError {
    ServerFnError::new(
        rustok_product::map_product_public_error(&error, operation, "product_admin_catalog_native")
            .to_string(),
    )
}

#[cfg(feature = "ssr")]
fn product_admin_read_port_context(
    tenant_id: uuid::Uuid,
    user_id: uuid::Uuid,
    locale: &str,
    operation: &str,
) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(user_id.to_string()),
        locale.to_string(),
        format!("product-admin:{operation}:{tenant_id}"),
    )
    .with_deadline(Duration::from_secs(2))
}

#[cfg(feature = "ssr")]
fn map_product_port_error(
    context: &PortContext,
    error: PortError,
    operation: &'static str,
) -> ServerFnError {
    let (message, public_kind, retryable) = match &error.kind {
        PortErrorKind::Validation => (
            if error.message.trim().is_empty() {
                "Invalid product catalog request"
            } else {
                error.message.as_str()
            },
            "validation",
            false,
        ),
        PortErrorKind::NotFound => ("Product was not found", "not_found", false),
        PortErrorKind::Conflict => (
            "Product operation conflicts with current state",
            "conflict",
            false,
        ),
        PortErrorKind::Forbidden => ("Product operation is not permitted", "forbidden", false),
        PortErrorKind::Unavailable => (
            "Product data is temporarily unavailable",
            "unavailable",
            true,
        ),
        PortErrorKind::Timeout => ("Product data is temporarily unavailable", "timeout", true),
        PortErrorKind::InvariantViolation => (
            "Product operation could not be completed safely",
            "invariant_violation",
            false,
        ),
    };

    tracing::error!(
        owner = "rustok_product.admin",
        owner_operation = operation,
        boundary = "product_admin_catalog_native",
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        owner_code = %error.code,
        owner_kind = public_kind,
        owner_retryable = error.retryable,
        retryable,
        "product admin read-port request failed"
    );
    ServerFnError::new(message.to_string())
}

#[cfg(feature = "ssr")]
fn map_product_list(value: rustok_product::AdminProductList) -> ProductList {
    ProductList {
        items: value
            .items
            .into_iter()
            .map(|item| ProductListItem {
                id: item.id.to_string(),
                status: item.status.to_string().to_ascii_uppercase(),
                title: item.title,
                handle: item.handle,
                seller_id: item.seller_id,
                vendor: item.vendor,
                product_type: item.product_type,
                shipping_profile_slug: item.shipping_profile_slug,
                primary_category_id: item.primary_category_id.map(|value| value.to_string()),
                tags: item.tags,
                created_at: item.created_at.to_rfc3339(),
                published_at: item.published_at.map(|value| value.to_rfc3339()),
            })
            .collect(),
        total: value.total,
        page: value.page,
        per_page: value.per_page,
        has_next: value.has_next,
    }
}

#[server(prefix = "/api/fn", endpoint = "product/admin/catalog-list")]
async fn product_admin_catalog_list_native(
    tenant_id: String,
    locale: Option<String>,
    search: Option<String>,
    status: Option<String>,
    vendor: Option<String>,
    category_id: Option<String>,
    sort_by: Option<String>,
    sort_direction: Option<String>,
    attribute_filters: Vec<String>,
    page: u64,
    per_page: u64,
) -> Result<ProductList, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_product::{AdminProductListQuery, CatalogService};

        let runtime_ctx = use_context::<rustok_api::HostRuntimeContext>()
            .ok_or_else(map_product_internal_error)?;
        let event_bus = runtime_ctx
            .shared_get::<rustok_outbox::TransactionalEventBus>()
            .ok_or_else(map_product_internal_error)?;
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(|_| ServerFnError::new("Authentication required"))?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(|_| map_product_internal_error())?;
        if !rustok_api::has_any_effective_permission(
            &auth.permissions,
            &[
                rustok_api::Permission::PRODUCTS_LIST,
                rustok_api::Permission::PRODUCTS_READ,
            ],
        ) {
            return Err(ServerFnError::new(
                "Permission denied: products:list or products:read required",
            ));
        }
        let requested_tenant_id = uuid::Uuid::parse_str(tenant_id.trim())
            .map_err(|_| ServerFnError::new("Invalid tenant_id"))?;
        if requested_tenant_id != tenant.id || auth.tenant_id != tenant.id {
            return Err(ServerFnError::new(
                "tenant_id does not match current tenant",
            ));
        }
        let request_context = require_request_context("admin_catalog_list").await?;
        let requested_locale = locale
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or(request_context.locale);
        let list_query = AdminProductListQuery::try_from_transport_with_attribute_filters(
            search,
            status,
            category_id,
            sort_by,
            sort_direction,
            attribute_filters,
        )
        .map_err(|error| map_product_service_error(error, "admin_catalog_list_input"))?;
        let service = CatalogService::new(runtime_ctx.db_clone(), event_bus);
        let port_context = product_admin_read_port_context(
            tenant.id,
            auth.user_id,
            requested_locale.as_str(),
            "admin_catalog_list",
        );
        let products = service
            .list_admin_products(
                port_context.clone(),
                AdminProductsRequest {
                    locale: Some(requested_locale.clone()),
                    fallback_locale: Some(tenant.default_locale.clone()),
                    query: list_query,
                    raw_status: None,
                    vendor,
                    product_type: None,
                    empty_missing_title: false,
                    page,
                    per_page,
                },
            )
            .await
            .map_err(|error| map_product_port_error(&port_context, error, "admin_catalog_list"))?;
        Ok(map_product_list(products))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (
            tenant_id,
            locale,
            search,
            status,
            vendor,
            category_id,
            sort_by,
            sort_direction,
            attribute_filters,
            page,
            per_page,
        );
        Err(ServerFnError::new(
            "product/admin/catalog-list requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn map_admin_catalog_facets(
    facets: Vec<rustok_product::StorefrontCatalogFacet>,
) -> Vec<crate::model::AdminCatalogFacet> {
    facets
        .into_iter()
        .map(|facet| crate::model::AdminCatalogFacet {
            code: facet.code,
            label: facet.label,
            value_type: facet.value_type,
            is_localized: facet.is_localized,
            is_enumerable: facet.is_enumerable,
            is_truncated: facet.is_truncated,
            total_products: facet.total_products,
            values: facet
                .values
                .into_iter()
                .map(|value| crate::model::AdminCatalogFacetValue {
                    value: value.value,
                    label: value.label,
                    count: value.count,
                })
                .collect(),
        })
        .collect()
}

/// Owner-counted admin facet buckets for the requested attribute codes.
///
/// The counts cover every lifecycle status the admin list shows, so the panel describes the same
/// population the grid below it renders.
pub(crate) async fn fetch_admin_catalog_facets(
    tenant_id: String,
    locale: Option<String>,
    controls: ProductAdminListInput,
    facet_codes: Vec<String>,
) -> Result<Vec<crate::model::AdminCatalogFacet>, ServerFnError> {
    product_admin_catalog_facets_native(
        tenant_id,
        locale,
        controls.search,
        controls.status,
        controls.vendor,
        controls.category_id,
        controls.sort_by,
        controls.sort_direction,
        controls.attribute_filters,
        facet_codes,
    )
    .await
}

#[server(prefix = "/api/fn", endpoint = "product/admin/catalog-facets")]
async fn product_admin_catalog_facets_native(
    tenant_id: String,
    locale: Option<String>,
    search: Option<String>,
    status: Option<String>,
    vendor: Option<String>,
    category_id: Option<String>,
    sort_by: Option<String>,
    sort_direction: Option<String>,
    attribute_filters: Vec<String>,
    facet_codes: Vec<String>,
) -> Result<Vec<crate::model::AdminCatalogFacet>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_product::{AdminProductListQuery, CatalogService};

        let runtime_ctx = use_context::<rustok_api::HostRuntimeContext>()
            .ok_or_else(map_product_internal_error)?;
        let event_bus = runtime_ctx
            .shared_get::<rustok_outbox::TransactionalEventBus>()
            .ok_or_else(map_product_internal_error)?;
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(|_| ServerFnError::new("Authentication required"))?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(|_| map_product_internal_error())?;
        if !rustok_api::has_any_effective_permission(
            &auth.permissions,
            &[
                rustok_api::Permission::PRODUCTS_LIST,
                rustok_api::Permission::PRODUCTS_READ,
            ],
        ) {
            return Err(ServerFnError::new(
                "Permission denied: products:list or products:read required",
            ));
        }
        let requested_tenant_id = uuid::Uuid::parse_str(tenant_id.trim())
            .map_err(|_| ServerFnError::new("Invalid tenant_id"))?;
        if requested_tenant_id != tenant.id || auth.tenant_id != tenant.id {
            return Err(ServerFnError::new(
                "tenant_id does not match current tenant",
            ));
        }
        let request_context = require_request_context("admin_catalog_facets").await?;
        let requested_locale = locale
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or(request_context.locale);
        let list_query = AdminProductListQuery::try_from_transport_with_attribute_filters(
            search,
            status,
            category_id,
            sort_by,
            sort_direction,
            attribute_filters,
        )
        .map_err(|error| map_product_service_error(error, "admin_catalog_facets_input"))?;
        let service = CatalogService::new(runtime_ctx.db_clone(), event_bus);
        let port_context = product_admin_read_port_context(
            tenant.id,
            auth.user_id,
            requested_locale.as_str(),
            "admin_catalog_facets",
        );
        let facets = service
            .load_admin_catalog_facets(
                port_context.clone(),
                AdminCatalogFacetsRequest {
                    locale: Some(requested_locale.clone()),
                    fallback_locale: Some(tenant.default_locale.clone()),
                    query: list_query,
                    vendor,
                    facet_codes,
                },
            )
            .await
            .map_err(|error| map_product_port_error(&port_context, error, "admin_catalog_facets"))?;
        Ok(map_admin_catalog_facets(facets))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (
            tenant_id,
            locale,
            search,
            status,
            vendor,
            category_id,
            sort_by,
            sort_direction,
            attribute_filters,
            facet_codes,
        );
        Err(ServerFnError::new(
            "product/admin/catalog-facets requires the `ssr` feature",
        ))
    }
}
