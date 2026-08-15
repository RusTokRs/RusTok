use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use rustok_api::Permission;
use rustok_api::locale_tags_match;
use rustok_api::{
    AuthContext, PortActor, PortContext, PortError, PortErrorKind, RequestContext, TenantContext,
};
use rustok_product::{
    CatalogService, CommerceError,
    entities::{product, product_translation},
};
use rustok_telemetry::metrics;
use rustok_web::{HttpError, HttpResult};
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, time::Instant};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    dto::ProductResponse, search::product_translation_title_search_condition,
    storefront_shipping::product_shipping_profile_slug,
};

use super::common::{PaginatedResponse, PaginationMeta, PaginationParams, ensure_permissions};

const ADMIN_PRODUCT_OWNER: &str = "rustok_product.catalog";
const ADMIN_PRODUCT_BOUNDARY: &str = "commerce_admin_product_http";
const MAX_ADMIN_PRODUCT_LIFECYCLE_KEY_LENGTH: usize = 191;

type AdminProductHttpPolicy = (StatusCode, &'static str, &'static str, &'static str);

#[derive(Clone, Copy)]
pub(crate) struct AdminProductErrorContext {
    tenant_id: Uuid,
    actor_id: Uuid,
    product_id: Option<Uuid>,
    operation: &'static str,
}

impl AdminProductErrorContext {
    pub(crate) fn new(
        tenant_id: Uuid,
        actor_id: Uuid,
        product_id: Option<Uuid>,
        operation: &'static str,
    ) -> Self {
        Self {
            tenant_id,
            actor_id,
            product_id,
            operation,
        }
    }
}

struct AdminProductDiagnosticContext {
    tenant_id: &'static str,
    actor_id: &'static str,
    product_id: &'static str,
    operation: &'static str,
}

impl From<&AdminProductErrorContext> for AdminProductDiagnosticContext {
    fn from(context: &AdminProductErrorContext) -> Self {
        Self {
            tenant_id: uuid_shape(context.tenant_id),
            actor_id: uuid_shape(context.actor_id),
            product_id: optional_uuid_shape(context.product_id),
            operation: context.operation,
        }
    }
}

struct AdminProductDiagnosticError;

impl std::fmt::Debug for AdminProductDiagnosticError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("redacted")
    }
}

fn uuid_shape(value: Uuid) -> &'static str {
    if value.is_nil() { "nil" } else { "non_nil" }
}

fn optional_uuid_shape(value: Option<Uuid>) -> &'static str {
    match value {
        None => "absent",
        Some(value) if value.is_nil() => "present_nil",
        Some(_) => "present_non_nil",
    }
}

fn product_error_policy(error: &CommerceError) -> AdminProductHttpPolicy {
    match error {
        CommerceError::Database(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_product_storage_unavailable",
            "Product storage is temporarily unavailable",
            "database",
        ),
        CommerceError::ProductNotFound(_) => (
            StatusCode::NOT_FOUND,
            "commerce_admin_not_found",
            "Commerce resource not found",
            "not_found",
        ),
        CommerceError::DuplicateHandle { .. } => (
            StatusCode::CONFLICT,
            "commerce_admin_product_handle_conflict",
            "A product with this handle already exists",
            "duplicate_handle",
        ),
        CommerceError::DuplicateSku(_) => (
            StatusCode::CONFLICT,
            "commerce_admin_product_sku_conflict",
            "A product variant with this SKU already exists",
            "duplicate_sku",
        ),
        CommerceError::Validation(_) | CommerceError::NoVariants => (
            StatusCode::BAD_REQUEST,
            "commerce_admin_product_invalid",
            "Product request is invalid",
            "validation",
        ),
        CommerceError::CannotDeletePublished => (
            StatusCode::CONFLICT,
            "commerce_admin_product_state_conflict",
            "Product operation conflicts with the current state",
            "state_conflict",
        ),
        CommerceError::Core(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_admin_product_failed",
            "Product operation could not be completed safely",
            "unexpected_owner_error",
        ),
    }
}

fn adopt_product_error_identity(context: &mut AdminProductErrorContext, error: &CommerceError) {
    if let CommerceError::ProductNotFound(id) = error {
        context.product_id = Some(*id);
    }
}

pub(crate) fn map_admin_product_error(
    mut context: AdminProductErrorContext,
    error: CommerceError,
) -> HttpError {
    adopt_product_error_identity(&mut context, &error);
    let (status, code, message, error_kind) = product_error_policy(&error);
    let context = AdminProductDiagnosticContext::from(&context);
    let error = AdminProductDiagnosticError;
    tracing::error!(
        error = ?error,
        owner = ADMIN_PRODUCT_OWNER,
        tenant_id = %context.tenant_id,
        actor_id = %context.actor_id,
        product_id = ?context.product_id,
        operation = %context.operation,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_PRODUCT_BOUNDARY,
        "commerce admin product operation failed"
    );
    HttpError::new(status, code, message)
}

pub(crate) fn admin_product_command_idempotency_key<T: Serialize>(
    tenant_id: Uuid,
    actor_id: Uuid,
    product_id: Option<Uuid>,
    operation: &'static str,
    payload: &T,
) -> HttpResult<String> {
    let payload = serde_json::to_vec(payload).map_err(|_| {
        tracing::error!(
            owner = ADMIN_PRODUCT_OWNER,
            tenant_id = %uuid_shape(tenant_id),
            actor_id = %uuid_shape(actor_id),
            product_id = %optional_uuid_shape(product_id),
            operation,
            error_kind = "request_identity_serialization",
            public_code = "commerce_admin_product_failed",
            boundary = ADMIN_PRODUCT_BOUNDARY,
            "commerce admin product command identity could not be materialized"
        );
        HttpError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_admin_product_failed",
            "Product operation could not be completed safely",
        )
    })?;
    let mut digest = Sha256::new();
    digest.update(tenant_id.as_bytes());
    digest.update(actor_id.as_bytes());
    digest.update(operation.as_bytes());
    if let Some(product_id) = product_id {
        digest.update(product_id.as_bytes());
    }
    digest.update(payload);
    Ok(format!(
        "commerce-admin-product:{operation}:{}",
        hex::encode(digest.finalize())
    ))
}

pub(crate) fn admin_product_lifecycle_idempotency_key(
    headers: &HeaderMap,
    tenant_id: Uuid,
    actor_id: Uuid,
    product_id: Uuid,
    operation: &'static str,
) -> HttpResult<String> {
    let caller_key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            HttpError::bad_request(
                "product_idempotency_key_required",
                "Idempotency-Key header is required",
            )
        })?;
    if caller_key.len() > MAX_ADMIN_PRODUCT_LIFECYCLE_KEY_LENGTH {
        return Err(HttpError::bad_request(
            "product_idempotency_key_invalid",
            format!(
                "Idempotency-Key must contain at most {MAX_ADMIN_PRODUCT_LIFECYCLE_KEY_LENGTH} bytes"
            ),
        ));
    }

    let mut digest = Sha256::new();
    digest.update(tenant_id.as_bytes());
    digest.update(actor_id.as_bytes());
    digest.update(product_id.as_bytes());
    digest.update(operation.as_bytes());
    digest.update(caller_key.as_bytes());
    Ok(format!(
        "commerce-admin-product:{operation}:{}",
        hex::encode(digest.finalize())
    ))
}

pub(crate) fn admin_product_command_context(
    tenant_id: Uuid,
    auth: &AuthContext,
    request_context: &RequestContext,
    idempotency_key: String,
) -> PortContext {
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        request_context.locale.as_str(),
        idempotency_key.clone(),
    )
    .with_idempotency_key(idempotency_key)
    .with_deadline(std::time::Duration::from_secs(2));
    match request_context.channel_slug.as_deref() {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

pub(crate) fn map_admin_product_port_error(
    context: AdminProductErrorContext,
    port_context: &PortContext,
    error: PortError,
) -> HttpError {
    let (status, code, message, error_kind) = match &error.kind {
        PortErrorKind::Validation => (
            StatusCode::BAD_REQUEST,
            "commerce_admin_product_invalid",
            "Product request is invalid",
            "validation",
        ),
        PortErrorKind::NotFound => (
            StatusCode::NOT_FOUND,
            "commerce_admin_not_found",
            "Commerce resource not found",
            "not_found",
        ),
        PortErrorKind::Conflict if error.code == "product.duplicate_handle" => (
            StatusCode::CONFLICT,
            "commerce_admin_product_handle_conflict",
            "A product with this handle already exists",
            "duplicate_handle",
        ),
        PortErrorKind::Conflict if error.code == "product.duplicate_sku" => (
            StatusCode::CONFLICT,
            "commerce_admin_product_sku_conflict",
            "A product variant with this SKU already exists",
            "duplicate_sku",
        ),
        PortErrorKind::Conflict => (
            StatusCode::CONFLICT,
            "commerce_admin_product_state_conflict",
            "Product operation conflicts with the current state",
            "state_conflict",
        ),
        PortErrorKind::Forbidden => (
            StatusCode::UNAUTHORIZED,
            "commerce_permission_denied",
            "Permission denied",
            "forbidden",
        ),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_product_storage_unavailable",
            "Product storage is temporarily unavailable",
            "unavailable",
        ),
        PortErrorKind::InvariantViolation => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_admin_product_failed",
            "Product operation could not be completed safely",
            "invariant_violation",
        ),
    };
    let diagnostic = AdminProductDiagnosticContext::from(&context);
    tracing::error!(
        owner = ADMIN_PRODUCT_OWNER,
        owner_operation = context.operation,
        correlation_id = %port_context.correlation_id,
        tenant_id = %diagnostic.tenant_id,
        actor_id = %diagnostic.actor_id,
        product_id = ?diagnostic.product_id,
        operation = %diagnostic.operation,
        internal_code = %error.code,
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_PRODUCT_BOUNDARY,
        "commerce admin product owner command failed"
    );
    HttpError::new(status, code, message)
}

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
    let total = query.clone().count(runtime.db()).await.map_err(|error| {
        map_admin_product_error(
            AdminProductErrorContext::new(tenant.id, auth.user_id, None, "list_products_count"),
            CommerceError::Database(error),
        )
    })?;
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
        .map_err(|error| {
            map_admin_product_error(
                AdminProductErrorContext::new(tenant.id, auth.user_id, None, "list_products_page"),
                CommerceError::Database(error),
            )
        })?;
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
            .map_err(|error| {
                map_admin_product_error(
                    AdminProductErrorContext::new(
                        tenant.id,
                        auth.user_id,
                        None,
                        "list_product_translations",
                    ),
                    CommerceError::Database(error),
                )
            })?;
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
        .map_err(|error| {
            map_admin_product_error(
                AdminProductErrorContext::new(tenant.id, auth.user_id, None, "list_product_tags"),
                error,
            )
        })?;

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
        .map_err(|error| {
            map_admin_product_error(
                AdminProductErrorContext::new(tenant.id, auth.user_id, Some(id), "show_product"),
                error,
            )
        })?;

    Ok(Json(product))
}

/// Shared admin product delete handler.
pub async fn delete_product(
    State(runtime): State<crate::controllers::CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> HttpResult<StatusCode> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_DELETE],
        "Permission denied: products:delete required",
    )?;

    let idempotency_key = admin_product_lifecycle_idempotency_key(
        &headers,
        tenant.id,
        auth.user_id,
        id,
        "delete_product",
    )?;
    let port_context =
        admin_product_command_context(tenant.id, &auth, &request_context, idempotency_key);
    runtime
        .product_catalog_command_port()
        .delete_product(port_context.clone(), id)
        .await
        .map_err(|error| {
            map_admin_product_port_error(
                AdminProductErrorContext::new(tenant.id, auth.user_id, Some(id), "delete_product"),
                &port_context,
                error,
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

/// Shared admin product publish handler.
pub async fn publish_product(
    State(runtime): State<crate::controllers::CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<ProductResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_UPDATE],
        "Permission denied: products:update required",
    )?;

    let idempotency_key = admin_product_lifecycle_idempotency_key(
        &headers,
        tenant.id,
        auth.user_id,
        id,
        "publish_product",
    )?;
    let port_context =
        admin_product_command_context(tenant.id, &auth, &request_context, idempotency_key);
    let product = runtime
        .product_catalog_command_port()
        .publish_product(port_context.clone(), id)
        .await
        .map_err(|error| {
            map_admin_product_port_error(
                AdminProductErrorContext::new(tenant.id, auth.user_id, Some(id), "publish_product"),
                &port_context,
                error,
            )
        })?;

    Ok(Json(product))
}

/// Shared admin product unpublish handler.
pub async fn unpublish_product(
    State(runtime): State<crate::controllers::CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<ProductResponse>> {
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_UPDATE],
        "Permission denied: products:update required",
    )?;

    let idempotency_key = admin_product_lifecycle_idempotency_key(
        &headers,
        tenant.id,
        auth.user_id,
        id,
        "unpublish_product",
    )?;
    let port_context =
        admin_product_command_context(tenant.id, &auth, &request_context, idempotency_key);
    let product = runtime
        .product_catalog_command_port()
        .unpublish_product(port_context.clone(), id)
        .await
        .map_err(|error| {
            map_admin_product_port_error(
                AdminProductErrorContext::new(
                    tenant.id,
                    auth.user_id,
                    Some(id),
                    "unpublish_product",
                ),
                &port_context,
                error,
            )
        })?;

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
