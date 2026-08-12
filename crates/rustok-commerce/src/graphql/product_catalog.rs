use async_graphql::{Context, ErrorExtensions, InputObject, Object, Result, SimpleObject};
use rustok_api::{
    Permission, PortActor, PortContext, PortError, PortErrorKind, RequestContext, TenantContext,
    graphql::require_module_enabled,
};
use rustok_outbox::TransactionalEventBus;
use rustok_product::{AdminProductListQuery, StorefrontProductListQuery};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use super::{
    GqlProductList, GqlProductListItem, GqlProductStatus, PRODUCT_MODULE_SLUG,
    map_product_service_error, product_query_tenant, require_commerce_permission,
    require_storefront_channel_enabled,
};

pub(crate) fn product_catalog_port_error(
    context: &PortContext,
    error: PortError,
    operation: &'static str,
) -> async_graphql::Error {
    let (code, message, retryable, error_kind) = match &error.kind {
        PortErrorKind::Validation => (
            "PRODUCT_VALIDATION",
            "Product request is invalid",
            false,
            "validation",
        ),
        PortErrorKind::NotFound => (
            "PRODUCT_NOT_FOUND",
            "Product was not found",
            false,
            "not_found",
        ),
        PortErrorKind::Conflict => (
            "PRODUCT_OPERATION_FAILED",
            "Product operation could not be completed safely",
            false,
            "conflict",
        ),
        PortErrorKind::Forbidden => (
            "PRODUCT_ACCESS_DENIED",
            "Product operation is not permitted",
            false,
            "forbidden",
        ),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            "PRODUCT_TEMPORARILY_UNAVAILABLE",
            "Product data is temporarily unavailable",
            true,
            "unavailable",
        ),
        PortErrorKind::InvariantViolation => (
            "PRODUCT_OPERATION_FAILED",
            "Product operation could not be completed safely",
            false,
            "invariant_violation",
        ),
    };

    tracing::error!(
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        operation,
        owner_code = %error.code,
        owner_kind = error_kind,
        owner_retryable = error.retryable,
        public_code = code,
        retryable,
        boundary = "commerce_graphql_product",
        "commerce GraphQL Product catalog owner port failed"
    );

    async_graphql::Error::new(message).extend_with(|_, extensions| {
        extensions.set("code", code);
        extensions.set("retryable", retryable);
        extensions.set("correlation_id", context.correlation_id.to_string());
    })
}

#[derive(InputObject, Default)]
pub struct StorefrontProductCatalogFilter {
    pub search: Option<String>,
    pub category_id: Option<Uuid>,
    pub sort_by: Option<String>,
    pub sort_direction: Option<String>,
    pub attribute_filters: Option<Vec<String>>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

#[derive(InputObject, Default)]
pub struct AdminProductCatalogFilter {
    pub search: Option<String>,
    pub status: Option<String>,
    pub category_id: Option<Uuid>,
    pub sort_by: Option<String>,
    pub sort_direction: Option<String>,
    pub attribute_filters: Option<Vec<String>>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

#[derive(SimpleObject)]
pub struct AdminProductCatalogList {
    pub items: Vec<AdminProductCatalogItem>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub has_next: bool,
}

#[derive(SimpleObject)]
pub struct AdminProductCatalogItem {
    pub id: Uuid,
    pub status: GqlProductStatus,
    pub title: String,
    pub handle: String,
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub primary_category_id: Option<Uuid>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub published_at: Option<String>,
}

#[derive(Default)]
pub struct ProductCatalogQuery;

#[Object]
impl ProductCatalogQuery {
    async fn storefront_product_catalog(
        &self,
        ctx: &Context<'_>,
        locale: Option<String>,
        filter: Option<StorefrontProductCatalogFilter>,
    ) -> Result<GqlProductList> {
        require_module_enabled(ctx, PRODUCT_MODULE_SLUG).await?;
        require_storefront_channel_enabled(ctx).await?;

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let request_context = ctx.data_opt::<RequestContext>();
        let requested_locale = locale
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| request_context.map(|context| context.locale.clone()))
            .unwrap_or_else(|| tenant.default_locale.clone());
        let public_channel_slug = request_context
            .and_then(|context| context.channel_slug.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_ascii_lowercase());
        let filter = filter.unwrap_or_default();
        let page = filter.page.unwrap_or(1);
        let per_page = filter.per_page.unwrap_or(12);
        let list_query = StorefrontProductListQuery::try_new_with_attribute_filters(
            filter.search,
            filter.category_id,
            filter.sort_by,
            filter.sort_direction,
            filter.attribute_filters.unwrap_or_default(),
        )
        .map_err(|error| map_product_service_error(error, "storefront_product_catalog_input"))?
        .with_pagination(page, per_page);

        let port_context = PortContext::new(
            tenant.id.to_string(),
            PortActor::service("commerce-storefront-graphql"),
            requested_locale.as_str(),
            format!("commerce-graphql-product:storefront-catalog:{page}:{per_page}"),
        )
        .with_deadline(std::time::Duration::from_secs(2));
        let port_context = match public_channel_slug.as_deref() {
            Some(channel) => port_context.with_channel(channel),
            None => port_context,
        };
        let product_read_runtime =
            crate::graphql_runtime::product_catalog_read_runtime_for_current_graphql_scope(
                db.clone(),
                event_bus.clone(),
            );
        let products = product_read_runtime
            .read_port()
            .list_filtered_published_products(
                port_context.clone(),
                rustok_product::FilteredPublishedProductsRequest {
                    locale: Some(requested_locale),
                    fallback_locale: Some(tenant.default_locale.clone()),
                    public_channel_slug,
                    query: list_query,
                },
            )
            .await
            .map_err(|error| {
                product_catalog_port_error(&port_context, error, "storefront_product_catalog")
            })?;

        Ok(GqlProductList {
            total: products.total,
            page: products.page,
            per_page: products.per_page,
            has_next: products.has_next,
            items: products
                .items
                .into_iter()
                .map(|item| GqlProductListItem {
                    id: item.id,
                    status: item.status.into(),
                    title: item.title,
                    handle: item.handle,
                    seller_id: item.seller_id,
                    vendor: item.vendor,
                    product_type: item.product_type,
                    shipping_profile_slug: None,
                    tags: item.tags,
                    created_at: item.created_at.to_rfc3339(),
                    published_at: item.published_at.map(|value| value.to_rfc3339()),
                })
                .collect(),
        })
    }

    async fn admin_product_catalog(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        locale: Option<String>,
        filter: Option<AdminProductCatalogFilter>,
    ) -> Result<AdminProductCatalogList> {
        require_module_enabled(ctx, PRODUCT_MODULE_SLUG).await?;
        let auth = require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_LIST, Permission::PRODUCTS_READ],
            "Permission denied: products:list or products:read required",
        )?;
        let tenant_id = product_query_tenant(ctx, tenant_id)?;

        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let request_context = ctx.data_opt::<RequestContext>();
        let requested_locale = locale
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| request_context.map(|context| context.locale.clone()))
            .unwrap_or_else(|| tenant.default_locale.clone());
        let filter = filter.unwrap_or_default();
        let page = filter.page.unwrap_or(1);
        let per_page = filter.per_page.unwrap_or(24);
        if page == 0 || per_page == 0 || per_page > 100 {
            return Err(map_product_service_error(
                rustok_product::CommerceError::Validation(
                    "page must be at least 1 and per_page must be between 1 and 100".to_string(),
                ),
                "admin_product_catalog",
            ));
        }
        let list_query = AdminProductListQuery::try_from_transport_with_attribute_filters(
            filter.search,
            filter.status,
            filter.category_id.map(|value| value.to_string()),
            filter.sort_by,
            filter.sort_direction,
            filter.attribute_filters.unwrap_or_default(),
        )
        .map_err(|error| map_product_service_error(error, "admin_product_catalog_input"))?;

        let port_context = PortContext::new(
            tenant_id.to_string(),
            PortActor::user(auth.user_id.to_string()),
            requested_locale.as_str(),
            format!("commerce-graphql-product:admin-catalog:{page}:{per_page}"),
        )
        .with_deadline(std::time::Duration::from_secs(2));
        let port_context = match request_context.and_then(|context| context.channel_slug.as_deref())
        {
            Some(channel) => port_context.with_channel(channel),
            None => port_context,
        };
        let product_read_runtime =
            crate::graphql_runtime::product_catalog_read_runtime_for_current_graphql_scope(
                db.clone(),
                event_bus.clone(),
            );
        let products = product_read_runtime
            .read_port()
            .list_admin_products(
                port_context.clone(),
                rustok_product::AdminProductsRequest {
                    locale: Some(requested_locale),
                    fallback_locale: Some(tenant.default_locale.clone()),
                    query: list_query,
                    raw_status: None,
                    vendor: None,
                    product_type: None,
                    empty_missing_title: false,
                    page,
                    per_page,
                },
            )
            .await
            .map_err(|error| {
                product_catalog_port_error(&port_context, error, "admin_product_catalog")
            })?;

        Ok(AdminProductCatalogList {
            total: products.total,
            page: products.page,
            per_page: products.per_page,
            has_next: products.has_next,
            items: products
                .items
                .into_iter()
                .map(|item| AdminProductCatalogItem {
                    id: item.id,
                    status: item.status.into(),
                    title: item.title,
                    handle: item.handle,
                    seller_id: item.seller_id,
                    vendor: item.vendor,
                    product_type: item.product_type,
                    shipping_profile_slug: item.shipping_profile_slug,
                    primary_category_id: item.primary_category_id,
                    tags: item.tags,
                    created_at: item.created_at.to_rfc3339(),
                    published_at: item.published_at.map(|value| value.to_rfc3339()),
                })
                .collect(),
        })
    }
}
