use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::Permission;
use rustok_api::locale_tags_match;
use rustok_api::{AuthContext, RequestContext, TenantContext};
use rustok_product::{
    CatalogService,
    entities::{product, product_translation},
};
use rustok_telemetry::metrics;
use rustok_web::{HttpError, HttpResult};
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};
use std::{collections::HashMap, time::Instant};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    dto::ProductResponse, search::product_translation_title_search_condition,
    storefront_shipping::product_shipping_profile_slug,
};

use super::common::{PaginatedResponse, PaginationMeta, PaginationParams, ensure_permissions};

/// Shared admin product list handler.
pub async fn list_products(
    State(runtime): State<crate::controllers::CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Query(params): Query<ListProductsParams>,
) -> HttpResult<Json<PaginatedResponse<ProductListItem>>> {
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
            runtime.db().get_database_backend(),
            locale,
            search,
        ));
    }

    let count_started_at = Instant::now();
    let total = query
        .clone()
        .count(runtime.db())
        .await
        .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?;
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
        .all(runtime.db())
        .await
        .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?;
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
            .all(runtime.db())
            .await
            .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?;
        metrics::record_read_path_query(
            "http",
            "commerce.list_products",
            "translations",
            translations_started_at.elapsed().as_secs_f64(),
            translations.len() as u64,
        );
        translations
    };

    let mut translation_map = HashMap::<Uuid, Vec<product_translation::Model>>::new();
    for translation in translations {
        translation_map
            .entry(translation.product_id)
            .or_default()
            .push(translation);
    }
    let catalog = CatalogService::new(runtime.db_clone(), runtime.event_bus());
    let product_tags = catalog
        .load_product_tag_map(
            tenant.id,
            &products,
            locale,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?;

    let items = products
        .into_iter()
        .map(|product| {
            let translation = translation_map.get(&product.id).and_then(|items| {
                pick_product_translation(items, locale, tenant.default_locale.as_str())
            });
            ProductListItem {
                id: product.id,
                status: product.status.to_string(),
                title: translation
                    .map(|value| value.title.clone())
                    .unwrap_or_default(),
                handle: translation
                    .map(|value| value.handle.clone())
                    .unwrap_or_default(),
                seller_id: product.seller_id,
                vendor: product.vendor,
                product_type: product.product_type,
                shipping_profile_slug: Some(product_shipping_profile_slug(
                    product.shipping_profile_slug.as_deref(),
                    &product.metadata,
                )),
                tags: product_tags.get(&product.id).cloned().unwrap_or_default(),
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

fn pick_product_translation<'a>(
    translations: &'a [product_translation::Model],
    locale: &str,
    default_locale: &str,
) -> Option<&'a product_translation::Model> {
    translations
        .iter()
        .find(|translation| locale_tags_match(&translation.locale, locale))
        .or_else(|| {
            (!locale_tags_match(default_locale, locale)).then(|| {
                translations
                    .iter()
                    .find(|translation| locale_tags_match(&translation.locale, default_locale))
            })?
        })
        .or_else(|| translations.first())
}

/// Shared admin product details handler.
pub async fn show_product(
    State(runtime): State<crate::controllers::CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<ProductResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_READ],
        "Permission denied: products:read required",
    )?;

    let service = CatalogService::new(runtime.db_clone(), runtime.event_bus());
    let product = service
        .get_product_with_locale_fallback(
            tenant.id,
            id,
            request_context.locale.as_str(),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?;

    Ok(Json(product))
}

/// Shared admin product delete handler.
pub async fn delete_product(
    State(runtime): State<crate::controllers::CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<StatusCode> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_DELETE],
        "Permission denied: products:delete required",
    )?;

    let service = CatalogService::new(runtime.db_clone(), runtime.event_bus());
    service
        .delete_product(tenant.id, auth.user_id, id)
        .await
        .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

/// Shared admin product publish handler.
pub async fn publish_product(
    State(runtime): State<crate::controllers::CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<ProductResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_UPDATE],
        "Permission denied: products:update required",
    )?;

    let service = CatalogService::new(runtime.db_clone(), runtime.event_bus());
    let product = service
        .publish_product(tenant.id, auth.user_id, id)
        .await
        .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?;

    Ok(Json(product))
}

/// Shared admin product unpublish handler.
pub async fn unpublish_product(
    State(runtime): State<crate::controllers::CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<ProductResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_UPDATE],
        "Permission denied: products:update required",
    )?;

    let service = CatalogService::new(runtime.db_clone(), runtime.event_bus());
    let product = service
        .unpublish_product(tenant.id, auth.user_id, id)
        .await
        .map_err(|err| HttpError::bad_request("commerce_operation_failed", err.to_string()))?;

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
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub published_at: Option<String>,
}
