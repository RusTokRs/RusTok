use leptos::prelude::*;

use crate::catalog_controls::ProductAdminListInput;
use crate::model::ProductList;
#[cfg(feature = "ssr")]
use crate::model::ProductListItem;

pub(super) async fn fetch_products(
    tenant_id: String,
    locale: Option<String>,
    controls: ProductAdminListInput,
) -> Result<ProductList, ServerFnError> {
    product_admin_catalog_list_native(
        tenant_id,
        locale,
        controls.search,
        controls.status,
        controls.category_id,
        controls.sort_by,
        controls.sort_direction,
        controls.attribute_filters,
    )
    .await
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
    category_id: Option<String>,
    sort_by: Option<String>,
    sort_direction: Option<String>,
    attribute_filters: Vec<String>,
) -> Result<ProductList, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_product::{AdminProductListQuery, CatalogService};

        let runtime_ctx = expect_context::<rustok_api::HostRuntimeContext>();
        let event_bus = runtime_ctx
            .shared_get::<rustok_outbox::TransactionalEventBus>()
            .ok_or_else(|| {
                ServerFnError::new(
                    "product/admin catalog list requires TransactionalEventBus in host runtime context",
                )
            })?;
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
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
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .ok();
        let requested_locale = locale
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| request_context.map(|context| context.locale))
            .unwrap_or_else(|| tenant.default_locale.clone());
        let list_query = AdminProductListQuery::try_from_transport_with_attribute_filters(
            search,
            status,
            category_id,
            sort_by,
            sort_direction,
            attribute_filters,
        )
        .map_err(|error| map_product_service_error(error, "admin_catalog_list_input"))?;
        let products = CatalogService::new(runtime_ctx.db_clone(), event_bus)
            .list_admin_products_with_query(
                tenant.id,
                requested_locale.as_str(),
                Some(tenant.default_locale.as_str()),
                list_query,
                1,
                24,
            )
            .await
            .map_err(|error| map_product_service_error(error, "admin_catalog_list"))?;
        Ok(map_product_list(products))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (
            tenant_id,
            locale,
            search,
            status,
            category_id,
            sort_by,
            sort_direction,
            attribute_filters,
        );
        Err(ServerFnError::new(
            "product/admin/catalog-list requires the `ssr` feature",
        ))
    }
}
