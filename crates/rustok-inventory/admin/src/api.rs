use leptos::prelude::*;
use std::fmt::{Display, Formatter};

use crate::core::{InventoryProductRequest, InventoryProductsRequest};
use crate::model::{InventoryAdminBootstrap, InventoryProductDetail, InventoryProductList};
use crate::transport::{
    CommerceGraphqlInventoryReadAdapter, InventoryReadTransport, InventoryTransportError,
};

#[derive(Debug, Clone)]
pub enum ApiError {
    ServerFn(String),
    Transport(InventoryTransportError),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServerFn(error) => write!(f, "{error}"),
            Self::Transport(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

impl From<InventoryTransportError> for ApiError {
    fn from(value: InventoryTransportError) -> Self {
        Self::Transport(value)
    }
}

fn transitional_read_transport() -> impl InventoryReadTransport {
    CommerceGraphqlInventoryReadAdapter
}

fn products_request(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: Option<String>,
    search: Option<String>,
    status: Option<String>,
) -> InventoryProductsRequest {
    InventoryProductsRequest {
        token,
        tenant_slug,
        tenant_id,
        locale,
        search,
        status,
    }
}

fn product_request(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    locale: Option<String>,
) -> InventoryProductRequest {
    InventoryProductRequest {
        token,
        tenant_slug,
        tenant_id,
        id,
        locale,
    }
}

pub async fn fetch_bootstrap(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<InventoryAdminBootstrap, ApiError> {
    match inventory_bootstrap_native().await {
        Ok(value) => Ok(value),
        Err(err) if cfg!(feature = "ssr") => Err(err.into()),
        Err(_) => transitional_read_transport()
            .fetch_bootstrap(token, tenant_slug)
            .await
            .map_err(Into::into),
    }
}

pub async fn fetch_products(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: Option<String>,
    search: Option<String>,
    status: Option<String>,
) -> Result<InventoryProductList, ApiError> {
    match inventory_products_native(
        tenant_id.clone(),
        locale.clone(),
        search.clone(),
        status.clone(),
    )
    .await
    {
        Ok(value) => Ok(value),
        Err(err) if cfg!(feature = "ssr") => Err(err.into()),
        Err(_) => transitional_read_transport()
            .fetch_products(products_request(
                token,
                tenant_slug,
                tenant_id,
                locale,
                search,
                status,
            ))
            .await
            .map_err(Into::into),
    }
}

pub async fn fetch_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    locale: Option<String>,
) -> Result<Option<InventoryProductDetail>, ApiError> {
    match inventory_product_native(tenant_id.clone(), id.clone(), locale.clone()).await {
        Ok(value) => Ok(value),
        Err(err) if cfg!(feature = "ssr") => Err(err.into()),
        Err(_) => transitional_read_transport()
            .fetch_product(product_request(token, tenant_slug, tenant_id, id, locale))
            .await
            .map_err(Into::into),
    }
}

#[cfg(feature = "ssr")]
fn ensure_permission(
    permissions: &[rustok_core::Permission],
    required: &[rustok_core::Permission],
    message: &str,
) -> Result<(), ServerFnError> {
    if !rustok_api::has_any_effective_permission(permissions, required) {
        return Err(ServerFnError::new(format!("Permission denied: {message}")));
    }

    Ok(())
}

#[cfg(feature = "ssr")]
fn parse_uuid(value: &str, field_name: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value.trim())
        .map_err(|_| ServerFnError::new(format!("Invalid {field_name}")))
}

#[cfg(feature = "ssr")]
fn parse_product_status(
    value: Option<String>,
) -> Result<Option<rustok_inventory::ProductStatus>, ServerFnError> {
    let Some(value) = crate::core::normalize_status_filter(value) else {
        return Ok(None);
    };

    match value.as_str() {
        "DRAFT" => Ok(Some(rustok_inventory::ProductStatus::Draft)),
        "ACTIVE" => Ok(Some(rustok_inventory::ProductStatus::Active)),
        "ARCHIVED" => Ok(Some(rustok_inventory::ProductStatus::Archived)),
        _ => Err(ServerFnError::new("Invalid product status")),
    }
}

#[cfg(feature = "ssr")]
fn assert_requested_tenant(
    tenant: &rustok_api::TenantContext,
    requested_tenant_id: &str,
) -> Result<(), ServerFnError> {
    let requested_tenant_id = parse_uuid(requested_tenant_id, "tenant_id")?;
    if requested_tenant_id != tenant.id {
        return Err(ServerFnError::new(
            "Requested tenant_id does not match request tenant context",
        ));
    }

    Ok(())
}

#[cfg(feature = "ssr")]
fn map_current_tenant(tenant: &rustok_api::TenantContext) -> crate::model::CurrentTenant {
    crate::model::CurrentTenant {
        id: tenant.id.to_string(),
        slug: tenant.slug.clone(),
        name: tenant.name.clone(),
    }
}

#[cfg(feature = "ssr")]
fn map_status(status: rustok_inventory::ProductStatus) -> String {
    status.to_string().to_ascii_uppercase()
}

#[cfg(feature = "ssr")]
fn map_product_list(value: rustok_inventory::AdminInventoryProductList) -> InventoryProductList {
    InventoryProductList {
        items: value.items.into_iter().map(map_product_list_item).collect(),
        total: value.total,
        page: value.page,
        per_page: value.per_page,
        has_next: value.has_next,
    }
}

#[cfg(feature = "ssr")]
fn map_product_list_item(
    value: rustok_inventory::AdminInventoryProductListItem,
) -> crate::model::InventoryProductListItem {
    crate::model::InventoryProductListItem {
        id: value.id.to_string(),
        status: map_status(value.status),
        title: value.title,
        handle: value.handle,
        vendor: value.vendor,
        product_type: value.product_type,
        shipping_profile_slug: value.shipping_profile_slug,
        tags: value.tags,
        created_at: value.created_at,
        published_at: value.published_at,
    }
}

#[cfg(feature = "ssr")]
fn map_product_detail(
    value: rustok_inventory::AdminInventoryProductDetail,
) -> InventoryProductDetail {
    InventoryProductDetail {
        id: value.id.to_string(),
        status: map_status(value.status),
        vendor: value.vendor,
        product_type: value.product_type,
        shipping_profile_slug: value.shipping_profile_slug,
        created_at: value.created_at,
        updated_at: value.updated_at,
        published_at: value.published_at,
        translations: value
            .translations
            .into_iter()
            .map(|translation| crate::model::InventoryProductTranslation {
                locale: translation.locale,
                title: translation.title,
                handle: translation.handle,
                description: translation.description,
            })
            .collect(),
        variants: value.variants.into_iter().map(map_variant).collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_variant(value: rustok_inventory::AdminInventoryVariant) -> crate::model::InventoryVariant {
    crate::model::InventoryVariant {
        id: value.id.to_string(),
        sku: value.sku,
        barcode: value.barcode,
        shipping_profile_slug: value.shipping_profile_slug,
        title: value.title,
        option1: value.option1,
        option2: value.option2,
        option3: value.option3,
        prices: value
            .prices
            .into_iter()
            .map(|price| crate::model::InventoryPrice {
                currency_code: price.currency_code,
                amount: price.amount,
                compare_at_amount: price.compare_at_amount,
                on_sale: price.on_sale,
            })
            .collect(),
        inventory_quantity: value.inventory_quantity,
        inventory_policy: value.inventory_policy,
        in_stock: value.in_stock,
    }
}

#[server(prefix = "/api/fn", endpoint = "inventory/bootstrap")]
async fn inventory_bootstrap_native() -> Result<InventoryAdminBootstrap, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{AuthContext, TenantContext};
        use rustok_core::Permission;

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        ensure_permission(
            &auth.permissions,
            &[Permission::INVENTORY_LIST, Permission::INVENTORY_READ],
            "inventory:list or inventory:read required",
        )?;

        Ok(InventoryAdminBootstrap {
            current_tenant: map_current_tenant(&tenant),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "inventory/bootstrap requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "inventory/products")]
async fn inventory_products_native(
    tenant_id: String,
    locale: Option<String>,
    search: Option<String>,
    status: Option<String>,
) -> Result<InventoryProductList, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, RequestContext, TenantContext};
        use rustok_core::Permission;
        use rustok_inventory::{AdminInventoryProductsFilter, AdminInventoryReadService};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request_context = leptos_axum::extract::<RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        ensure_permission(
            &auth.permissions,
            &[Permission::INVENTORY_LIST],
            "inventory:list required",
        )?;
        assert_requested_tenant(&tenant, &tenant_id)?;

        let requested_locale = crate::core::normalize_locale_filter(locale)
            .unwrap_or_else(|| request_context.locale.clone());
        let service = AdminInventoryReadService::new(app_ctx.db.clone());
        let products = service
            .list_products(
                tenant.id,
                Some(requested_locale.as_str()),
                AdminInventoryProductsFilter {
                    status: parse_product_status(status)?,
                    search: crate::core::normalize_search_filter(search),
                    page: None,
                    per_page: None,
                },
            )
            .await
            .map_err(ServerFnError::new)?;

        Ok(map_product_list(products))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (tenant_id, locale, search, status);
        Err(ServerFnError::new(
            "inventory/products requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "inventory/product")]
async fn inventory_product_native(
    tenant_id: String,
    id: String,
    locale: Option<String>,
) -> Result<Option<InventoryProductDetail>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, RequestContext, TenantContext};
        use rustok_core::Permission;
        use rustok_inventory::AdminInventoryReadService;

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request_context = leptos_axum::extract::<RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        ensure_permission(
            &auth.permissions,
            &[Permission::INVENTORY_READ],
            "inventory:read required",
        )?;
        assert_requested_tenant(&tenant, &tenant_id)?;

        let product_id = parse_uuid(&id, "product_id")?;
        let requested_locale = crate::core::normalize_locale_filter(locale)
            .unwrap_or_else(|| request_context.locale.clone());
        let service = AdminInventoryReadService::new(app_ctx.db.clone());
        let product = service
            .get_product(tenant.id, product_id, Some(requested_locale.as_str()))
            .await
            .map_err(ServerFnError::new)?;

        Ok(product.map(map_product_detail))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (tenant_id, id, locale);
        Err(ServerFnError::new(
            "inventory/product requires the `ssr` feature",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{product_request, products_request};

    #[test]
    fn products_request_preserves_inventory_facade_context() {
        let request = products_request(
            Some("token".to_string()),
            Some("tenant-slug".to_string()),
            "tenant-id".to_string(),
            Some("en".to_string()),
            Some("boots".to_string()),
            Some("ACTIVE".to_string()),
        );

        assert_eq!(request.token.as_deref(), Some("token"));
        assert_eq!(request.tenant_slug.as_deref(), Some("tenant-slug"));
        assert_eq!(request.tenant_id, "tenant-id");
        assert_eq!(request.locale.as_deref(), Some("en"));
        assert_eq!(request.search.as_deref(), Some("boots"));
        assert_eq!(request.status.as_deref(), Some("ACTIVE"));
    }

    #[test]
    fn product_request_preserves_inventory_facade_context() {
        let request = product_request(
            Some("token".to_string()),
            Some("tenant-slug".to_string()),
            "tenant-id".to_string(),
            "product-id".to_string(),
            Some("de".to_string()),
        );

        assert_eq!(request.token.as_deref(), Some("token"));
        assert_eq!(request.tenant_slug.as_deref(), Some("tenant-slug"));
        assert_eq!(request.tenant_id, "tenant-id");
        assert_eq!(request.id, "product-id");
        assert_eq!(request.locale.as_deref(), Some("de"));
    }
}
