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
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect};
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

pub(crate) fn product_core_http_policy(
    error: &rustok_core::error::Error,
    internal_code: &'static str,
) -> AdminProductHttpPolicy {
    use rustok_api::PortErrorKind;
    match rustok_product::error::core_port_error(error).kind {
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_product_dependency_unavailable",
            "Product dependency is temporarily unavailable",
            "dependency_unavailable",
        ),
        PortErrorKind::Forbidden => (
            StatusCode::FORBIDDEN,
            "commerce_product_access_denied",
            "Product access is denied",
            "forbidden",
        ),
        PortErrorKind::NotFound => (
            StatusCode::NOT_FOUND,
            "commerce_product_dependency_not_found",
            "Referenced product resource was not found",
            "dependency_not_found",
        ),
        PortErrorKind::Validation => (
            StatusCode::BAD_REQUEST,
            "commerce_product_invalid",
            "Product request is invalid",
            "validation",
        ),
        PortErrorKind::Conflict => (
            StatusCode::CONFLICT,
            "commerce_product_conflict",
            "Product request conflicts with the current state",
            "conflict",
        ),
        PortErrorKind::InvariantViolation => (
            StatusCode::INTERNAL_SERVER_ERROR,
            internal_code,
            "Product operation could not be completed safely",
            "unexpected_owner_error",
        ),
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
        CommerceError::ProductNotFound(_)
        | CommerceError::VariantNotFound(_)
        | CommerceError::ImageNotFound(_) => (
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
        CommerceError::CannotDeletePublished | CommerceError::CannotDeleteOnlyVariant => (
            StatusCode::CONFLICT,
            "commerce_admin_product_state_conflict",
            "Product operation conflicts with the current state",
            "state_conflict",
        ),
        CommerceError::Core(error) => {
            product_core_http_policy(error, "commerce_admin_product_failed")
        }
        CommerceError::Conflict(_) => (
            StatusCode::CONFLICT,
            "commerce_admin_product_conflict",
            "Product request conflicts with the current state",
            "conflict",
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

pub(crate) fn admin_product_command_idempotency_key(headers: &HeaderMap) -> HttpResult<String> {
    let value = headers
        .get("Idempotency-Key")
        .ok_or_else(|| {
            HttpError::new(
                StatusCode::BAD_REQUEST,
                "commerce_admin_idempotency_key_required",
                "Idempotency-Key header is required for this write operation",
            )
        })?
        .to_str()
        .map_err(|_| {
            HttpError::new(
                StatusCode::BAD_REQUEST,
                "commerce_admin_idempotency_key_invalid",
                "Idempotency-Key header is invalid",
            )
        })?
        .trim()
        .to_string();

    if value.is_empty() || value.len() > MAX_ADMIN_PRODUCT_LIFECYCLE_KEY_LENGTH {
        return Err(HttpError::new(
            StatusCode::BAD_REQUEST,
            "commerce_admin_idempotency_key_invalid",
            "Idempotency-Key header must contain 1 to 191 bytes",
        ));
    }

    Ok(value)
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

/// All identity dimensions come from upstream authentication/middleware. A typed
/// context is not authority to mix another tenant or principal into an owner call.
pub(crate) fn ensure_admin_product_scope(
    tenant: &TenantContext,
    auth: &AuthContext,
    request: &RequestContext,
) -> HttpResult<()> {
    if tenant.id.is_nil()
        || auth.user_id.is_nil()
        || auth.tenant_id != tenant.id
        || request.tenant_id != tenant.id
        || request.user_id != Some(auth.user_id)
    {
        return Err(HttpError::forbidden(
            "commerce_admin_product_scope_mismatch",
            "Authenticated Product context does not match the request",
        ));
    }
    Ok(())
}

pub(crate) fn admin_product_command_context(
    tenant_id: Uuid,
    auth: &AuthContext,
    request_context: &RequestContext,
    product_id: Option<Uuid>,
    operation: &'static str,
    idempotency_key: String,
) -> PortContext {
    let resource_id = product_id
        .map(|value| value.to_string())
        .unwrap_or_else(|| "new".to_string());
    let mut context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        request_context.locale.as_str(),
        request_context.correlation_id.clone(),
    )
    .with_causation_id(format!("commerce-admin-product:{operation}:{resource_id}"))
    .with_idempotency_key(idempotency_key)
    .with_deadline(std::time::Duration::from_secs(2));
    for permission in &auth.permissions {
        context = context.with_claim(permission.to_string());
    }
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
        PortErrorKind::Validation if error.code == "product.revision_required" => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "commerce_admin_product_revision_required",
            "Product update requires the revision of the document that was read",
            "revision_required",
        ),
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
        PortErrorKind::Conflict if error.code == "product.revision_conflict" => (
            StatusCode::CONFLICT,
            "commerce_admin_product_revision_conflict",
            "Product was modified by another editor",
            "revision_conflict",
        ),
        PortErrorKind::Conflict => (
            StatusCode::CONFLICT,
            "commerce_admin_product_state_conflict",
            "Product operation conflicts with the current state",
            "state_conflict",
        ),
        PortErrorKind::Forbidden => (
            StatusCode::FORBIDDEN,
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
    let owner_code_length = error.code.chars().count();
    tracing::error!(
        owner = ADMIN_PRODUCT_OWNER,
        owner_operation = context.operation,
        correlation_id = %port_context.correlation_id,
        tenant_id = %diagnostic.tenant_id,
        actor_id = %diagnostic.actor_id,
        product_id = ?diagnostic.product_id,
        operation = %diagnostic.operation,
        owner_code_length,
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = %status,
        boundary = ADMIN_PRODUCT_BOUNDARY,
        "commerce admin product owner command failed with bounded diagnostics"
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
    ensure_admin_product_scope(&tenant, &auth, &request_context)?;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    tracing::debug!(
        boundary = ADMIN_PRODUCT_BOUNDARY,
        operation = "list_products",
        tenant_id = %tenant.id,
        actor_id = %auth.user_id,
        "handling list_products"
    );

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
            .filter(product_translation::Column::TenantId.eq(tenant.id))
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
    ensure_admin_product_scope(&tenant, &auth, &request_context)?;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_product_id",
            "Product ID must not be nil",
        ));
    }
    tracing::debug!(
        boundary = ADMIN_PRODUCT_BOUNDARY,
        operation = "show_product",
        tenant_id = %tenant.id,
        actor_id = %auth.user_id,
        product_id = %id,
        "handling show_product"
    );

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
    ensure_admin_product_scope(&tenant, &auth, &request_context)?;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_product_id",
            "Product ID must not be nil",
        ));
    }
    tracing::debug!(
        boundary = ADMIN_PRODUCT_BOUNDARY,
        operation = "delete_product",
        tenant_id = %tenant.id,
        actor_id = %auth.user_id,
        product_id = %id,
        "handling delete_product"
    );

    let idempotency_key = admin_product_lifecycle_idempotency_key(
        &headers,
        tenant.id,
        auth.user_id,
        id,
        "delete_product",
    )?;
    let port_context = admin_product_command_context(
        tenant.id,
        &auth,
        &request_context,
        Some(id),
        "delete_product",
        idempotency_key,
    );
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
    ensure_admin_product_scope(&tenant, &auth, &request_context)?;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_product_id",
            "Product ID must not be nil",
        ));
    }
    tracing::debug!(
        boundary = ADMIN_PRODUCT_BOUNDARY,
        operation = "publish_product",
        tenant_id = %tenant.id,
        actor_id = %auth.user_id,
        product_id = %id,
        "handling publish_product"
    );

    let idempotency_key = admin_product_lifecycle_idempotency_key(
        &headers,
        tenant.id,
        auth.user_id,
        id,
        "publish_product",
    )?;
    let port_context = admin_product_command_context(
        tenant.id,
        &auth,
        &request_context,
        Some(id),
        "publish_product",
        idempotency_key,
    );
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
    ensure_admin_product_scope(&tenant, &auth, &request_context)?;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_product_id",
            "Product ID must not be nil",
        ));
    }
    tracing::debug!(
        boundary = ADMIN_PRODUCT_BOUNDARY,
        operation = "unpublish_product",
        tenant_id = %tenant.id,
        actor_id = %auth.user_id,
        product_id = %id,
        "handling unpublish_product"
    );

    let idempotency_key = admin_product_lifecycle_idempotency_key(
        &headers,
        tenant.id,
        auth.user_id,
        id,
        "unpublish_product",
    )?;
    let port_context = admin_product_command_context(
        tenant.id,
        &auth,
        &request_context,
        Some(id),
        "unpublish_product",
        idempotency_key,
    );
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

/// Shared admin product archive handler.
pub async fn archive_product(
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
    ensure_admin_product_scope(&tenant, &auth, &request_context)?;
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_product_id",
            "Product ID must not be nil",
        ));
    }
    tracing::debug!(
        boundary = ADMIN_PRODUCT_BOUNDARY,
        operation = "archive_product",
        tenant_id = %tenant.id,
        actor_id = %auth.user_id,
        product_id = %id,
        "handling archive_product"
    );

    let idempotency_key = admin_product_lifecycle_idempotency_key(
        &headers,
        tenant.id,
        auth.user_id,
        id,
        "archive_product",
    )?;
    let port_context = admin_product_command_context(
        tenant.id,
        &auth,
        &request_context,
        Some(id),
        "archive_product",
        idempotency_key,
    );
    let product = runtime
        .product_catalog_command_port()
        .archive_product(port_context.clone(), id)
        .await
        .map_err(|error| {
            map_admin_product_port_error(
                AdminProductErrorContext::new(tenant.id, auth.user_id, Some(id), "archive_product"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::PortActor;

    #[test]
    fn forbidden_port_error_maps_to_http_403() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let context = PortContext::new(
            tenant_id.to_string(),
            PortActor::user(actor_id.to_string()),
            "en",
            "test-correlation",
        )
        .with_idempotency_key("test-idempotency-key")
        .with_deadline(std::time::Duration::from_secs(2));

        let error = map_admin_product_port_error(
            AdminProductErrorContext::new(
                tenant_id,
                actor_id,
                Some(Uuid::new_v4()),
                "forbidden_port_error",
            ),
            &context,
            PortError::forbidden("product.forbidden", "Permission denied"),
        );

        assert_eq!(error.status, StatusCode::FORBIDDEN);
        assert_eq!(error.code, "commerce_permission_denied");
        assert_eq!(error.message, "Permission denied");
    }
    #[test]
    fn product_scope_rejects_mixed_authenticated_dimensions() {
        let id = Uuid::new_v4();
        let user = Uuid::new_v4();
        let tenant = TenantContext {
            id,
            name: "test".into(),
            slug: "test".into(),
            domain: None,
            settings: serde_json::json!({}),
            default_locale: "en".into(),
            is_active: true,
        };
        let mut auth = AuthContext {
            tenant_id: id,
            user_id: user,
            session_id: Uuid::new_v4(),
            permissions: vec![],
            client_id: None,
            scopes: vec![],
            grant_type: "test".into(),
        };
        let mut request = RequestContext {
            tenant_id: id,
            user_id: Some(user),
            channel_id: None,
            channel_slug: None,
            channel_resolution_source: None,
            locale: "en".into(),
            correlation_id: "trace".into(),
        };
        assert!(ensure_admin_product_scope(&tenant, &auth, &request).is_ok());
        auth.tenant_id = Uuid::new_v4();
        assert_eq!(
            ensure_admin_product_scope(&tenant, &auth, &request)
                .unwrap_err()
                .status,
            StatusCode::FORBIDDEN
        );
        auth.tenant_id = id;
        request.tenant_id = Uuid::new_v4();
        assert!(ensure_admin_product_scope(&tenant, &auth, &request).is_err());
        request.tenant_id = id;
        request.user_id = None;
        assert!(ensure_admin_product_scope(&tenant, &auth, &request).is_err());
        request.user_id = Some(Uuid::new_v4());
        assert!(ensure_admin_product_scope(&tenant, &auth, &request).is_err());
        auth.user_id = Uuid::nil();
        request.user_id = Some(Uuid::nil());
        assert!(ensure_admin_product_scope(&tenant, &auth, &request).is_err());
    }

    #[test]
    fn owner_port_error_messages_never_enter_http_envelope() {
        for kind in [
            PortErrorKind::Validation,
            PortErrorKind::NotFound,
            PortErrorKind::Conflict,
            PortErrorKind::Forbidden,
            PortErrorKind::Unavailable,
            PortErrorKind::Timeout,
            PortErrorKind::InvariantViolation,
        ] {
            let error = map_admin_product_port_error(
                AdminProductErrorContext::new(
                    Uuid::new_v4(),
                    Uuid::new_v4(),
                    None,
                    "create_product",
                ),
                &PortContext::new("tenant", PortActor::user("user"), "en", "trace"),
                PortError::new(kind, "SECRET_OWNER_CODE", "SECRET_OWNER_MESSAGE", true),
            );
            assert!(!error.message.contains("SECRET"));
            assert!(!error.code.contains("SECRET"));
        }
    }
}

#[cfg(test)]
mod core_product_policy_tests {
    use super::*;
    use rustok_core::error::Error as CoreError;
    #[test]
    fn nested_core_http_statuses_follow_owner_classification() {
        for (error, expected) in [
            (
                CoreError::Forbidden("private-marker".into()),
                StatusCode::FORBIDDEN,
            ),
            (
                CoreError::Database(sea_orm::DbErr::Custom("private-marker".into())),
                StatusCode::SERVICE_UNAVAILABLE,
            ),
            (
                CoreError::External("private-marker".into()),
                StatusCode::SERVICE_UNAVAILABLE,
            ),
            (
                CoreError::Validation("private-marker".into()),
                StatusCode::BAD_REQUEST,
            ),
            (
                CoreError::NotFound("private-marker".into()),
                StatusCode::NOT_FOUND,
            ),
            (
                CoreError::Internal("private-marker".into()),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        ] {
            let (status, _, message, _) =
                product_core_http_policy(&error, "commerce_admin_product_failed");
            assert_eq!(status, expected);
            assert!(!message.contains("private-marker"));
        }
    }
}
