use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use loco_rs::{app::AppContext, Error, Result};
use rustok_api::{
    loco::transactional_event_bus_from_context, AuthContext, RequestContext, TenantContext,
};
use rustok_core::Permission;
use rustok_telemetry::metrics;
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};
use std::{collections::HashMap, time::Instant};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    dto::ProductResponse,
    entities::{product, product_translation},
    search::product_translation_title_search_condition,
    CatalogService,
};

use super::common::{ensure_permissions, PaginatedResponse, PaginationMeta, PaginationParams};

/// Shared admin product list handler.
pub async fn list_products(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Query(params): Query<ListProductsParams>,
) -> Result<Json<PaginatedResponse<ProductListItem>>> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_LIST],
        "Permission denied: products:list required",
    )?;

    let requested_limit = params
        .pagination
        .as_ref()
        .map(|pagination| pagination.per_page);
    let pagination = params.pagination.unwrap_or_default();
    let locale = params
        .locale
        .as_deref()
        .unwrap_or(request_context.locale.as_str());

    let mut query = product::Entity::find().filter(product::Column::TenantId.eq(tenant.id));

    if let Some(status) = &params.status {
        query = query.filter(product::Column::Status.eq(status));
    }
    if let Some(vendor) = &params.vendor {
        query = query.filter(product::Column::Vendor.eq(vendor));
    }
    if let Some(product_type) = &params.product_type {
        query = query.filter(product::Column::ProductType.eq(product_type));
    }
    if let Some(search) = &params.search {
        query = query.filter(product_translation_title_search_condition(
            ctx.db.get_database_backend(),
            locale,
            search,
        ));
    }

    let count_started_at = Instant::now();
    let total = query
        .clone()
        .count(&ctx.db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    metrics::record_read_path_query(
        "http",
        "commerce.list_products",
        "count",
        count_started_at.elapsed().as_secs_f64(),
        total,
    );

    let products_started_at = Instant::now();
    let products = query
        .order_by_desc(product::Column::CreatedAt)
        .offset(pagination.offset())
        .limit(pagination.limit())
        .all(&ctx.db)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    metrics::record_read_path_query(
        "http",
        "commerce.list_products",
        "products_page",
        products_started_at.elapsed().as_secs_f64(),
        products.len() as u64,
    );

    let product_ids = products
        .iter()
        .map(|product| product.id)
        .collect::<Vec<_>>();
    let translations = if product_ids.is_empty() {
        Vec::new()
    } else {
        let translations_started_at = Instant::now();
        let translations = product_translation::Entity::find()
            .filter(product_translation::Column::ProductId.is_in(product_ids))
            .filter(product_translation::Column::Locale.eq(locale))
            .all(&ctx.db)
            .await
            .map_err(|err| Error::BadRequest(err.to_string()))?;
        metrics::record_read_path_query(
            "http",
            "commerce.list_products",
            "translations",
            translations_started_at.elapsed().as_secs_f64(),
            translations.len() as u64,
        );
        translations
    };

    let translation_map = translations
        .into_iter()
        .map(|translation| (translation.product_id, translation))
        .collect::<HashMap<_, _>>();

    let items = products
        .into_iter()
        .map(|product| {
            let translation = translation_map.get(&product.id);
            ProductListItem {
                id: product.id,
                status: product.status.to_string(),
                title: translation
                    .map(|value| value.title.clone())
                    .unwrap_or_default(),
                handle: translation
                    .map(|value| value.handle.clone())
                    .unwrap_or_default(),
                vendor: product.vendor,
                product_type: product.product_type,
                created_at: product.created_at.to_rfc3339(),
                published_at: product.published_at.map(|value| value.to_rfc3339()),
            }
        })
        .collect::<Vec<_>>();

    metrics::record_read_path_budget(
        "http",
        "commerce.list_products",
        requested_limit,
        pagination.limit(),
        items.len(),
    );

    Ok(Json(PaginatedResponse {
        data: items,
        meta: PaginationMeta::new(pagination.page, pagination.limit(), total),
    }))
}

/// Shared admin product details handler.
pub async fn show_product(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ProductResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_READ],
        "Permission denied: products:read required",
    )?;

    let service = CatalogService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let product = service
        .get_product(tenant.id, id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    Ok(Json(product))
}

/// Shared admin product delete handler.
pub async fn delete_product(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_DELETE],
        "Permission denied: products:delete required",
    )?;

    let service = CatalogService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    service
        .delete_product(tenant.id, auth.user_id, id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

/// Shared admin product publish handler.
pub async fn publish_product(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ProductResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_UPDATE],
        "Permission denied: products:update required",
    )?;

    let service = CatalogService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let product = service
        .publish_product(tenant.id, auth.user_id, id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    Ok(Json(product))
}

/// Shared admin product unpublish handler.
pub async fn unpublish_product(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ProductResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_UPDATE],
        "Permission denied: products:update required",
    )?;

    let service = CatalogService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let product = service
        .unpublish_product(tenant.id, auth.user_id, id)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    Ok(Json(product))
}

#[derive(Debug, serde::Deserialize, ToSchema, utoipa::IntoParams)]
pub struct ListProductsParams {
    #[serde(flatten)]
    pub pagination: Option<PaginationParams>,
    pub status: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub search: Option<String>,
    pub locale: Option<String>,
}

#[derive(Debug, serde::Serialize, ToSchema)]
pub struct ProductListItem {
    pub id: Uuid,
    pub status: String,
    pub title: String,
    pub handle: String,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub created_at: String,
    pub published_at: Option<String>,
}
