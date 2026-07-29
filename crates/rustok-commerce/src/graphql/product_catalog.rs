use async_graphql::{Context, InputObject, Object, Result, SimpleObject};
use rustok_api::{
    Permission, RequestContext, TenantContext, graphql::require_module_enabled,
};
use rustok_outbox::TransactionalEventBus;
use rustok_product::{AdminProductListQuery, CatalogService, StorefrontProductListQuery};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use super::{
    GqlProductList, GqlProductListItem, GqlProductStatus, PRODUCT_MODULE_SLUG,
    map_product_service_error, product_query_tenant, require_commerce_permission,
    require_storefront_channel_enabled,
};

#[derive(InputObject, Default)]
pub struct StorefrontProductCatalogFilter {
    pub search: Option<String>,
    pub category_id: Option<Uuid>,
    pub sort_by: Option<String>,
    pub sort_direction: Option<String>,
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
        let list_query = StorefrontProductListQuery::try_new(
            filter.search,
            filter.category_id,
            filter.sort_by,
            filter.sort_direction,
        )
        .map_err(|error| map_product_service_error(error, "storefront_product_catalog_input"))?;
        let products = CatalogService::new(db.clone(), event_bus.clone())
            .list_published_products_with_query(
                tenant.id,
                requested_locale.as_str(),
                Some(tenant.default_locale.as_str()),
                public_channel_slug.as_deref(),
                list_query,
                page,
                per_page,
            )
            .await
            .map_err(|error| map_product_service_error(error, "storefront_product_catalog"))?;

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
        require_commerce_permission(
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
        let list_query = AdminProductListQuery::try_from_transport(
            filter.search,
            filter.status,
            filter.category_id.map(|value| value.to_string()),
            filter.sort_by,
            filter.sort_direction,
        )
        .map_err(|error| map_product_service_error(error, "admin_product_catalog_input"))?;
        let products = CatalogService::new(db.clone(), event_bus.clone())
            .list_admin_products_with_query(
                tenant_id,
                requested_locale.as_str(),
                Some(tenant.default_locale.as_str()),
                list_query,
                page,
                per_page,
            )
            .await
            .map_err(|error| map_product_service_error(error, "admin_product_catalog"))?;

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
