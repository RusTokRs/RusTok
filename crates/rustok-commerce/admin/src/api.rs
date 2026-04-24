use leptos::prelude::*;
#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::{
    CommerceAdminBootstrap, CommerceAdminCartSnapshot, CommerceCartPromotionDraft,
    CommerceCartPromotionPreview, ShippingProfile, ShippingProfileDraft, ShippingProfileList,
};
#[cfg(feature = "ssr")]
use crate::model::{CommerceCartPromotionKind, CommerceCartPromotionScope};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    Graphql(String),
    ServerFn(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graphql(error) => write!(f, "{error}"),
            Self::ServerFn(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<GraphqlHttpError> for ApiError {
    fn from(value: GraphqlHttpError) -> Self {
        Self::Graphql(value.to_string())
    }
}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

const BOOTSTRAP_QUERY: &str = "query CommerceAdminBootstrap { currentTenant { id slug name } }";
const SHIPPING_PROFILES_QUERY: &str = "query CommerceShippingProfiles($tenantId: UUID!, $filter: ShippingProfilesFilter) { shippingProfiles(tenantId: $tenantId, filter: $filter) { total page perPage hasNext items { id tenantId slug name description active metadata createdAt updatedAt } } }";
const SHIPPING_PROFILE_QUERY: &str = "query CommerceShippingProfile($tenantId: UUID!, $id: UUID!) { shippingProfile(tenantId: $tenantId, id: $id) { id tenantId slug name description active metadata createdAt updatedAt } }";
const CREATE_SHIPPING_PROFILE_MUTATION: &str = "mutation CommerceCreateShippingProfile($tenantId: UUID!, $input: CreateShippingProfileInput!) { createShippingProfile(tenantId: $tenantId, input: $input) { id tenantId slug name description active metadata createdAt updatedAt } }";
const UPDATE_SHIPPING_PROFILE_MUTATION: &str = "mutation CommerceUpdateShippingProfile($tenantId: UUID!, $id: UUID!, $input: UpdateShippingProfileInput!) { updateShippingProfile(tenantId: $tenantId, id: $id, input: $input) { id tenantId slug name description active metadata createdAt updatedAt } }";
const DEACTIVATE_SHIPPING_PROFILE_MUTATION: &str = "mutation CommerceDeactivateShippingProfile($tenantId: UUID!, $id: UUID!) { deactivateShippingProfile(tenantId: $tenantId, id: $id) { id tenantId slug name description active metadata createdAt updatedAt } }";
const REACTIVATE_SHIPPING_PROFILE_MUTATION: &str = "mutation CommerceReactivateShippingProfile($tenantId: UUID!, $id: UUID!) { reactivateShippingProfile(tenantId: $tenantId, id: $id) { id tenantId slug name description active metadata createdAt updatedAt } }";

#[derive(Debug, Deserialize)]
struct BootstrapResponse {
    #[serde(rename = "currentTenant")]
    current_tenant: crate::model::CurrentTenant,
}

#[derive(Debug, Deserialize)]
struct ShippingProfilesResponse {
    #[serde(rename = "shippingProfiles")]
    shipping_profiles: ShippingProfileList,
}

#[derive(Debug, Deserialize)]
struct ShippingProfileResponse {
    #[serde(rename = "shippingProfile")]
    shipping_profile: Option<ShippingProfile>,
}

#[derive(Debug, Deserialize)]
struct CreateShippingProfileResponse {
    #[serde(rename = "createShippingProfile")]
    create_shipping_profile: ShippingProfile,
}

#[derive(Debug, Deserialize)]
struct UpdateShippingProfileResponse {
    #[serde(rename = "updateShippingProfile")]
    update_shipping_profile: ShippingProfile,
}

#[derive(Debug, Deserialize)]
struct DeactivateShippingProfileResponse {
    #[serde(rename = "deactivateShippingProfile")]
    deactivate_shipping_profile: ShippingProfile,
}

#[derive(Debug, Deserialize)]
struct ReactivateShippingProfileResponse {
    #[serde(rename = "reactivateShippingProfile")]
    reactivate_shipping_profile: ShippingProfile,
}

#[derive(Debug, Serialize)]
struct TenantScopedVariables<T> {
    #[serde(rename = "tenantId")]
    tenant_id: String,
    #[serde(flatten)]
    extra: T,
}

#[derive(Debug, Serialize)]
struct ShippingProfileVariables {
    id: String,
}

#[derive(Debug, Serialize)]
struct ShippingProfilesVariables {
    filter: ShippingProfilesFilter,
}

#[derive(Debug, Serialize)]
struct CreateShippingProfileVariables {
    input: CreateShippingProfileInput,
}

#[derive(Debug, Serialize)]
struct UpdateShippingProfileVariables {
    id: String,
    input: UpdateShippingProfileInput,
}

#[derive(Debug, Serialize)]
struct ShippingProfilesFilter {
    active: Option<bool>,
    search: Option<String>,
    page: Option<u64>,
    #[serde(rename = "perPage")]
    per_page: Option<u64>,
}

#[derive(Debug, Serialize)]
struct CreateShippingProfileInput {
    slug: String,
    translations: Vec<ShippingProfileTranslationInput>,
    metadata: Option<String>,
}

#[derive(Debug, Serialize)]
struct UpdateShippingProfileInput {
    slug: Option<String>,
    translations: Option<Vec<ShippingProfileTranslationInput>>,
    metadata: Option<String>,
}

#[derive(Debug, Serialize)]
struct ShippingProfileTranslationInput {
    locale: String,
    name: String,
    description: Option<String>,
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/graphql")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/graphql")
    }
}

async fn request<V, T>(
    query: &str,
    variables: Option<V>,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, variables),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(ApiError::from)
}

pub async fn fetch_bootstrap(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<CommerceAdminBootstrap, ApiError> {
    let response: BootstrapResponse =
        request::<serde_json::Value, BootstrapResponse>(BOOTSTRAP_QUERY, None, token, tenant_slug)
            .await?;
    Ok(CommerceAdminBootstrap {
        current_tenant: response.current_tenant,
    })
}

pub async fn fetch_shipping_profiles(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    search: Option<String>,
) -> Result<ShippingProfileList, ApiError> {
    let response: ShippingProfilesResponse = request(
        SHIPPING_PROFILES_QUERY,
        Some(TenantScopedVariables {
            tenant_id,
            extra: ShippingProfilesVariables {
                filter: ShippingProfilesFilter {
                    active: None,
                    search,
                    page: Some(1),
                    per_page: Some(24),
                },
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.shipping_profiles)
}

pub async fn fetch_shipping_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
) -> Result<Option<ShippingProfile>, ApiError> {
    let response: ShippingProfileResponse = request(
        SHIPPING_PROFILE_QUERY,
        Some(TenantScopedVariables {
            tenant_id,
            extra: ShippingProfileVariables { id },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.shipping_profile)
}

pub async fn create_shipping_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    draft: ShippingProfileDraft,
) -> Result<ShippingProfile, ApiError> {
    let response: CreateShippingProfileResponse = request(
        CREATE_SHIPPING_PROFILE_MUTATION,
        Some(TenantScopedVariables {
            tenant_id,
            extra: CreateShippingProfileVariables {
                input: build_create_shipping_profile_input(draft),
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.create_shipping_profile)
}

pub async fn update_shipping_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    draft: ShippingProfileDraft,
) -> Result<ShippingProfile, ApiError> {
    let response: UpdateShippingProfileResponse = request(
        UPDATE_SHIPPING_PROFILE_MUTATION,
        Some(TenantScopedVariables {
            tenant_id,
            extra: UpdateShippingProfileVariables {
                id,
                input: build_update_shipping_profile_input(draft),
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.update_shipping_profile)
}

pub async fn deactivate_shipping_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
) -> Result<ShippingProfile, ApiError> {
    let response: DeactivateShippingProfileResponse = request(
        DEACTIVATE_SHIPPING_PROFILE_MUTATION,
        Some(TenantScopedVariables {
            tenant_id,
            extra: ShippingProfileVariables { id },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.deactivate_shipping_profile)
}

pub async fn reactivate_shipping_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
) -> Result<ShippingProfile, ApiError> {
    let response: ReactivateShippingProfileResponse = request(
        REACTIVATE_SHIPPING_PROFILE_MUTATION,
        Some(TenantScopedVariables {
            tenant_id,
            extra: ShippingProfileVariables { id },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.reactivate_shipping_profile)
}

#[allow(dead_code)]
pub async fn preview_cart_promotion(
    cart_id: String,
    payload: CommerceCartPromotionDraft,
) -> Result<CommerceCartPromotionPreview, ApiError> {
    commerce_admin_preview_cart_promotion_native(cart_id, payload)
        .await
        .map_err(Into::into)
}

#[allow(dead_code)]
pub async fn apply_cart_promotion(
    cart_id: String,
    payload: CommerceCartPromotionDraft,
) -> Result<CommerceAdminCartSnapshot, ApiError> {
    commerce_admin_apply_cart_promotion_native(cart_id, payload)
        .await
        .map_err(Into::into)
}

fn build_create_shipping_profile_input(draft: ShippingProfileDraft) -> CreateShippingProfileInput {
    CreateShippingProfileInput {
        slug: draft.slug.trim().to_string(),
        translations: vec![ShippingProfileTranslationInput {
            locale: draft.locale,
            name: draft.name.trim().to_string(),
            description: optional_text(draft.description.as_str()),
        }],
        metadata: optional_json_text(draft.metadata_json.as_str()),
    }
}

fn build_update_shipping_profile_input(draft: ShippingProfileDraft) -> UpdateShippingProfileInput {
    UpdateShippingProfileInput {
        slug: optional_text(draft.slug.as_str()),
        translations: Some(vec![ShippingProfileTranslationInput {
            locale: draft.locale,
            name: draft.name.trim().to_string(),
            description: optional_text(draft.description.as_str()),
        }]),
        metadata: optional_json_text(draft.metadata_json.as_str()),
    }
}

fn optional_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn optional_json_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(feature = "ssr")]
fn ensure_permission(
    permissions: &[rustok_core::Permission],
    required: &[rustok_core::Permission],
    message: &str,
) -> Result<(), ServerFnError> {
    if required
        .iter()
        .any(|permission| permissions.iter().any(|value| value == permission))
    {
        Ok(())
    } else {
        Err(ServerFnError::new(format!("Permission denied: {message}")))
    }
}

#[cfg(feature = "ssr")]
fn parse_cart_id(value: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value.trim()).map_err(|_| ServerFnError::new("Invalid cart_id"))
}

#[cfg(feature = "ssr")]
fn parse_optional_line_item_id(
    value: &str,
    scope: &CommerceCartPromotionScope,
) -> Result<Option<uuid::Uuid>, ServerFnError> {
    let trimmed = value.trim();
    match scope {
        CommerceCartPromotionScope::Cart | CommerceCartPromotionScope::Shipping => {
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Err(ServerFnError::new(
                    "line_item_id is allowed only for line_item scope",
                ))
            }
        }
        CommerceCartPromotionScope::LineItem => {
            if trimmed.is_empty() {
                return Err(ServerFnError::new(
                    "line_item_id is required for line_item scope",
                ));
            }
            uuid::Uuid::parse_str(trimmed)
                .map(Some)
                .map_err(|_| ServerFnError::new("Invalid line_item_id"))
        }
    }
}

#[cfg(feature = "ssr")]
fn parse_decimal(value: &str, field_name: &str) -> Result<rust_decimal::Decimal, ServerFnError> {
    value
        .trim()
        .parse::<rust_decimal::Decimal>()
        .map_err(|_| ServerFnError::new(format!("Invalid {field_name}")))
}

#[cfg(feature = "ssr")]
fn parse_required_decimal(
    value: &str,
    field_name: &str,
) -> Result<rust_decimal::Decimal, ServerFnError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ServerFnError::new(format!(
            "{field_name} is required for the selected promotion kind"
        )));
    }
    parse_decimal(trimmed, field_name)
}

#[cfg(feature = "ssr")]
fn ensure_unused_decimal(value: &str, field_name: &str) -> Result<(), ServerFnError> {
    if value.trim().is_empty() {
        Ok(())
    } else {
        Err(ServerFnError::new(format!(
            "{field_name} must be omitted for the selected promotion kind"
        )))
    }
}

#[cfg(feature = "ssr")]
fn parse_metadata_json(value: &str) -> Result<serde_json::Value, ServerFnError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Ok(serde_json::Value::Object(Default::default()))
    } else {
        serde_json::from_str(trimmed)
            .map_err(|_| ServerFnError::new("Invalid JSON metadata payload"))
    }
}

#[cfg(feature = "ssr")]
fn normalize_source_id(value: &str) -> Result<String, ServerFnError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(ServerFnError::new("source_id is required"))
    } else {
        Ok(trimmed.to_string())
    }
}

#[cfg(feature = "ssr")]
async fn preview_cart_promotion_native_with_context(
    app_ctx: &loco_rs::app::AppContext,
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    cart_id: String,
    payload: CommerceCartPromotionDraft,
) -> Result<CommerceCartPromotionPreview, ServerFnError> {
    use rustok_cart::CartService;
    use rustok_core::Permission;

    ensure_permission(
        &auth.permissions,
        &[Permission::ORDERS_READ],
        "orders:read required",
    )?;

    let cart_id = parse_cart_id(&cart_id)?;
    let line_item_id = parse_optional_line_item_id(&payload.line_item_id, &payload.scope)?;
    let source_id = normalize_source_id(&payload.source_id)?;
    let service = CartService::new(app_ctx.db.clone());

    let preview = match payload.kind {
        CommerceCartPromotionKind::PercentageDiscount => {
            let discount_percent =
                parse_required_decimal(&payload.discount_percent, "discount_percent")?;
            ensure_unused_decimal(&payload.amount, "amount")?;
            match payload.scope {
                CommerceCartPromotionScope::Shipping => {
                    service
                        .preview_percentage_shipping_promotion(
                            tenant.id,
                            cart_id,
                            source_id.as_str(),
                            discount_percent,
                        )
                        .await
                }
                CommerceCartPromotionScope::Cart | CommerceCartPromotionScope::LineItem => {
                    service
                        .preview_percentage_promotion(
                            tenant.id,
                            cart_id,
                            line_item_id,
                            source_id.as_str(),
                            discount_percent,
                        )
                        .await
                }
            }
        }
        CommerceCartPromotionKind::FixedDiscount => {
            let amount = parse_required_decimal(&payload.amount, "amount")?;
            ensure_unused_decimal(&payload.discount_percent, "discount_percent")?;
            match payload.scope {
                CommerceCartPromotionScope::Shipping => {
                    service
                        .preview_fixed_shipping_promotion(
                            tenant.id,
                            cart_id,
                            source_id.as_str(),
                            amount,
                        )
                        .await
                }
                CommerceCartPromotionScope::Cart | CommerceCartPromotionScope::LineItem => {
                    service
                        .preview_fixed_promotion(
                            tenant.id,
                            cart_id,
                            line_item_id,
                            source_id.as_str(),
                            amount,
                        )
                        .await
                }
            }
        }
    }
    .map_err(ServerFnError::new)?;

    Ok(map_cart_promotion_preview(payload.scope, preview))
}

#[cfg(feature = "ssr")]
async fn apply_cart_promotion_native_with_context(
    app_ctx: &loco_rs::app::AppContext,
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    cart_id: String,
    payload: CommerceCartPromotionDraft,
) -> Result<CommerceAdminCartSnapshot, ServerFnError> {
    use rustok_cart::CartService;
    use rustok_core::Permission;

    ensure_permission(
        &auth.permissions,
        &[Permission::ORDERS_UPDATE],
        "orders:update required",
    )?;

    let cart_id = parse_cart_id(&cart_id)?;
    let line_item_id = parse_optional_line_item_id(&payload.line_item_id, &payload.scope)?;
    let source_id = normalize_source_id(&payload.source_id)?;
    let metadata = parse_metadata_json(&payload.metadata_json)?;
    let service = CartService::new(app_ctx.db.clone());

    let cart = match payload.kind {
        CommerceCartPromotionKind::PercentageDiscount => {
            let discount_percent =
                parse_required_decimal(&payload.discount_percent, "discount_percent")?;
            ensure_unused_decimal(&payload.amount, "amount")?;
            match payload.scope {
                CommerceCartPromotionScope::Shipping => {
                    service
                        .apply_percentage_shipping_promotion(
                            tenant.id,
                            cart_id,
                            source_id.as_str(),
                            discount_percent,
                            metadata,
                        )
                        .await
                }
                CommerceCartPromotionScope::Cart | CommerceCartPromotionScope::LineItem => {
                    service
                        .apply_percentage_promotion(
                            tenant.id,
                            cart_id,
                            line_item_id,
                            source_id.as_str(),
                            discount_percent,
                            metadata,
                        )
                        .await
                }
            }
        }
        CommerceCartPromotionKind::FixedDiscount => {
            let amount = parse_required_decimal(&payload.amount, "amount")?;
            ensure_unused_decimal(&payload.discount_percent, "discount_percent")?;
            match payload.scope {
                CommerceCartPromotionScope::Shipping => {
                    service
                        .apply_fixed_shipping_promotion(
                            tenant.id,
                            cart_id,
                            source_id.as_str(),
                            amount,
                            metadata,
                        )
                        .await
                }
                CommerceCartPromotionScope::Cart | CommerceCartPromotionScope::LineItem => {
                    service
                        .apply_fixed_promotion(
                            tenant.id,
                            cart_id,
                            line_item_id,
                            source_id.as_str(),
                            amount,
                            metadata,
                        )
                        .await
                }
            }
        }
    }
    .map_err(ServerFnError::new)?;

    Ok(map_cart_snapshot(cart))
}

#[cfg(feature = "ssr")]
fn map_cart_promotion_preview(
    scope: CommerceCartPromotionScope,
    preview: rustok_cart::services::cart::CartPromotionPreview,
) -> CommerceCartPromotionPreview {
    CommerceCartPromotionPreview {
        kind: match preview.kind {
            rustok_cart::services::cart::CartPromotionKind::PercentageDiscount => {
                CommerceCartPromotionKind::PercentageDiscount
            }
            rustok_cart::services::cart::CartPromotionKind::FixedDiscount => {
                CommerceCartPromotionKind::FixedDiscount
            }
        },
        scope,
        line_item_id: preview.line_item_id.map(|value| value.to_string()),
        currency_code: preview.currency_code,
        base_amount: preview.base_amount.normalize().to_string(),
        adjustment_amount: preview.adjustment_amount.normalize().to_string(),
        adjusted_amount: preview.adjusted_amount.normalize().to_string(),
    }
}

#[cfg(feature = "ssr")]
fn map_cart_snapshot(cart: rustok_cart::dto::CartResponse) -> CommerceAdminCartSnapshot {
    CommerceAdminCartSnapshot {
        id: cart.id.to_string(),
        currency_code: cart.currency_code,
        shipping_total: cart.shipping_total.normalize().to_string(),
        adjustment_total: cart.adjustment_total.normalize().to_string(),
        total_amount: cart.total_amount.normalize().to_string(),
        adjustments: cart
            .adjustments
            .into_iter()
            .map(|adjustment| crate::model::CommerceAdminCartAdjustment {
                id: adjustment.id.to_string(),
                line_item_id: adjustment.line_item_id.map(|value| value.to_string()),
                source_type: adjustment.source_type,
                source_id: adjustment.source_id,
                scope: adjustment
                    .metadata
                    .get("scope")
                    .and_then(|value| value.as_str())
                    .map(ToString::to_string),
                amount: adjustment.amount.normalize().to_string(),
                currency_code: adjustment.currency_code,
                metadata: adjustment.metadata.to_string(),
            })
            .collect(),
    }
}

#[server(prefix = "/api/fn", endpoint = "commerce/admin/preview-cart-promotion")]
async fn commerce_admin_preview_cart_promotion_native(
    cart_id: String,
    payload: CommerceCartPromotionDraft,
) -> Result<CommerceCartPromotionPreview, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        preview_cart_promotion_native_with_context(&app_ctx, &auth, &tenant, cart_id, payload).await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (cart_id, payload);
        Err(ServerFnError::new(
            "commerce/admin/preview-cart-promotion requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "commerce/admin/apply-cart-promotion")]
async fn commerce_admin_apply_cart_promotion_native(
    cart_id: String,
    payload: CommerceCartPromotionDraft,
) -> Result<CommerceAdminCartSnapshot, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        apply_cart_promotion_native_with_context(&app_ctx, &auth, &tenant, cart_id, payload).await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (cart_id, payload);
        Err(ServerFnError::new(
            "commerce/admin/apply-cart-promotion requires the `ssr` feature",
        ))
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;
    use loco_rs::app::{AppContext, SharedStore};
    use loco_rs::cache;
    use loco_rs::environment::Environment;
    use loco_rs::storage::{self, Storage};
    use loco_rs::tests_cfg::config::test_config;
    use rustok_api::{AuthContext, TenantContext};
    use rustok_cart::dto::{AddCartLineItemInput, CreateCartInput};
    use rustok_cart::CartService;
    use rustok_core::events::EventTransport;
    use rustok_core::Permission;
    use rustok_fulfillment::dto::CreateShippingOptionInput;
    use rustok_fulfillment::FulfillmentService;
    use rustok_test_utils::db::setup_test_db;
    use rustok_test_utils::MockEventTransport;
    use serde_json::json;
    use std::sync::Arc;

    mod support {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../rustok-commerce/tests/support.rs"
        ));
    }

    fn test_app_context(db: sea_orm::DatabaseConnection) -> AppContext {
        let shared_store = Arc::new(SharedStore::default());
        let event_transport: Arc<dyn EventTransport> = Arc::new(MockEventTransport::new());
        shared_store.insert(event_transport);

        AppContext {
            environment: Environment::Test,
            db,
            queue_provider: None,
            config: test_config(),
            mailer: None,
            storage: Storage::single(storage::drivers::mem::new()).into(),
            cache: Arc::new(cache::Cache::new(cache::drivers::null::new())),
            shared_store,
        }
    }

    fn test_tenant() -> TenantContext {
        TenantContext {
            id: uuid::Uuid::new_v4(),
            slug: "acme".to_string(),
            name: "Acme".to_string(),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        }
    }

    fn auth_with_permissions(permissions: Vec<Permission>) -> AuthContext {
        AuthContext {
            user_id: uuid::Uuid::new_v4(),
            session_id: uuid::Uuid::new_v4(),
            tenant_id: uuid::Uuid::new_v4(),
            permissions,
            client_id: None,
            scopes: vec![],
            grant_type: "password".to_string(),
        }
    }

    async fn seed_tenant_context(db: &sea_orm::DatabaseConnection, tenant: &TenantContext) {
        use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO tenants (id, name, slug, domain, settings, default_locale, is_active)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            vec![
                tenant.id.to_string().into(),
                tenant.name.clone().into(),
                tenant.slug.clone().into(),
                tenant.domain.clone().into(),
                tenant.settings.to_string().into(),
                tenant.default_locale.clone().into(),
                i32::from(tenant.is_active).into(),
            ],
        ))
        .await
        .expect("insert tenant");

        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO tenant_locales (id, tenant_id, locale, name, native_name, is_default, is_enabled, fallback_locale)
             VALUES (?, ?, ?, ?, ?, 1, 1, NULL)",
            vec![
                uuid::Uuid::new_v4().to_string().into(),
                tenant.id.to_string().into(),
                tenant.default_locale.clone().into(),
                "English".to_string().into(),
                "English".to_string().into(),
            ],
        ))
        .await
        .expect("insert tenant locale");
    }

    async fn create_shipping_profile_for_cart(
        db: &sea_orm::DatabaseConnection,
        tenant_id: uuid::Uuid,
    ) {
        use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

        rustok_commerce::entities::shipping_profile::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            slug: Set("default".to_string()),
            active: Set(true),
            metadata: Set(json!({})),
            created_at: Set(chrono::Utc::now().into()),
            updated_at: Set(chrono::Utc::now().into()),
        }
        .insert(db)
        .await
        .expect("insert shipping profile");

        rustok_commerce::entities::shipping_profile_translation::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            shipping_profile_id: Set(rustok_commerce::entities::shipping_profile::Entity::find()
                .filter(rustok_commerce::entities::shipping_profile::Column::TenantId.eq(tenant_id))
                .one(db)
                .await
                .expect("load shipping profile")
                .expect("shipping profile exists")
                .id),
            locale: Set("en".to_string()),
            name: Set("Default".to_string()),
            description: Set(Some("Default shipping profile".to_string())),
        }
        .insert(db)
        .await
        .expect("insert shipping profile translation");
    }

    async fn create_shipping_option_for_cart(
        db: &sea_orm::DatabaseConnection,
        tenant_id: uuid::Uuid,
    ) -> uuid::Uuid {
        let service = FulfillmentService::new(db.clone());
        let option = service
            .create_shipping_option(
                tenant_id,
                CreateShippingOptionInput {
                    provider_id: Some("manual".to_string()),
                    amount: rust_decimal::Decimal::new(999, 2),
                    currency_code: "EUR".to_string(),
                    allowed_shipping_profile_slugs: Some(vec!["default".to_string()]),
                    translations: vec![rustok_fulfillment::dto::ShippingOptionTranslationInput {
                        locale: "en".to_string(),
                        name: "Standard shipping".to_string(),
                    }],
                    metadata: json!({}),
                },
            )
            .await
            .expect("create shipping option");
        option.id
    }

    async fn seed_cart_with_shipping(
        db: &sea_orm::DatabaseConnection,
        tenant: &TenantContext,
    ) -> (uuid::Uuid, uuid::Uuid) {
        support::ensure_commerce_schema(db).await;
        seed_tenant_context(db, tenant).await;
        create_shipping_profile_for_cart(db, tenant.id).await;

        let cart_service = CartService::new(db.clone());
        let shipping_option_id = create_shipping_option_for_cart(db, tenant.id).await;
        let cart = cart_service
            .create_cart(
                tenant.id,
                CreateCartInput {
                    customer_id: None,
                    email: Some("cart@example.com".to_string()),
                    region_id: None,
                    country_code: Some("DE".to_string()),
                    locale_code: Some("en".to_string()),
                    selected_shipping_option_id: Some(shipping_option_id),
                    currency_code: "EUR".to_string(),
                    metadata: json!({}),
                },
            )
            .await
            .expect("create cart");
        let cart = cart_service
            .add_line_item(
                tenant.id,
                cart.id,
                AddCartLineItemInput {
                    product_id: None,
                    variant_id: None,
                    shipping_profile_slug: Some("default".to_string()),
                    sku: Some("SKU-1".to_string()),
                    title: "Operator line".to_string(),
                    quantity: 1,
                    unit_price: rust_decimal::Decimal::new(1999, 2),
                    metadata: json!({}),
                },
            )
            .await
            .expect("add line item");
        (cart.id, cart.line_items[0].id)
    }

    #[tokio::test]
    async fn preview_cart_promotion_native_with_context_supports_shipping_scope() {
        let db = setup_test_db().await;
        let app = test_app_context(db.clone());
        let tenant = test_tenant();
        let auth = auth_with_permissions(vec![Permission::ORDERS_READ]);
        let (cart_id, _) = seed_cart_with_shipping(&db, &tenant).await;

        let preview = preview_cart_promotion_native_with_context(
            &app,
            &auth,
            &tenant,
            cart_id.to_string(),
            CommerceCartPromotionDraft {
                kind: CommerceCartPromotionKind::PercentageDiscount,
                scope: CommerceCartPromotionScope::Shipping,
                line_item_id: String::new(),
                source_id: "promo-shipping-native".to_string(),
                discount_percent: "50".to_string(),
                amount: String::new(),
                metadata_json: String::new(),
            },
        )
        .await
        .expect("preview shipping promotion");

        assert_eq!(preview.kind, CommerceCartPromotionKind::PercentageDiscount);
        assert_eq!(preview.scope, CommerceCartPromotionScope::Shipping);
        assert_eq!(preview.line_item_id, None);
        assert_eq!(preview.currency_code, "EUR");
        assert_eq!(preview.base_amount, "9.99");
        assert_eq!(preview.adjustment_amount, "4.99");
        assert_eq!(preview.adjusted_amount, "5");
    }

    #[tokio::test]
    async fn apply_cart_promotion_native_with_context_snapshots_shipping_adjustment() {
        let db = setup_test_db().await;
        let app = test_app_context(db.clone());
        let tenant = test_tenant();
        let auth = auth_with_permissions(vec![Permission::ORDERS_UPDATE]);
        let (cart_id, _) = seed_cart_with_shipping(&db, &tenant).await;

        let cart = apply_cart_promotion_native_with_context(
            &app,
            &auth,
            &tenant,
            cart_id.to_string(),
            CommerceCartPromotionDraft {
                kind: CommerceCartPromotionKind::FixedDiscount,
                scope: CommerceCartPromotionScope::Shipping,
                line_item_id: String::new(),
                source_id: "promo-shipping-native".to_string(),
                discount_percent: String::new(),
                amount: "4.99".to_string(),
                metadata_json: "{\"campaign\":\"native-operator\"}".to_string(),
            },
        )
        .await
        .expect("apply shipping promotion");

        assert_eq!(cart.shipping_total, "9.99");
        assert_eq!(cart.adjustment_total, "4.99");
        assert_eq!(cart.total_amount, "24.99");
        assert_eq!(cart.adjustments.len(), 1);
        let adjustment = &cart.adjustments[0];
        assert_eq!(adjustment.source_type, "promotion");
        assert_eq!(
            adjustment.source_id.as_deref(),
            Some("promo-shipping-native")
        );
        assert_eq!(adjustment.scope.as_deref(), Some("shipping"));
        assert_eq!(adjustment.amount, "4.99");
        assert_eq!(adjustment.currency_code, "EUR");
        assert!(adjustment
            .metadata
            .contains("\"campaign\":\"native-operator\""));
        assert!(adjustment.metadata.contains("\"scope\":\"shipping\""));
    }

    #[tokio::test]
    async fn preview_cart_promotion_native_with_context_rejects_missing_line_item_target() {
        let db = setup_test_db().await;
        let app = test_app_context(db);
        let tenant = test_tenant();
        let auth = auth_with_permissions(vec![Permission::ORDERS_READ]);

        let error = preview_cart_promotion_native_with_context(
            &app,
            &auth,
            &tenant,
            uuid::Uuid::new_v4().to_string(),
            CommerceCartPromotionDraft {
                kind: CommerceCartPromotionKind::FixedDiscount,
                scope: CommerceCartPromotionScope::LineItem,
                line_item_id: String::new(),
                source_id: "promo-line-item".to_string(),
                discount_percent: String::new(),
                amount: "3.00".to_string(),
                metadata_json: String::new(),
            },
        )
        .await
        .expect_err("line item scope must require line_item_id");

        assert!(
            error
                .to_string()
                .contains("line_item_id is required for line_item scope"),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn apply_cart_promotion_native_with_context_enforces_orders_update_permission() {
        let db = setup_test_db().await;
        let app = test_app_context(db.clone());
        let tenant = test_tenant();
        let auth = auth_with_permissions(vec![Permission::ORDERS_READ]);
        let (cart_id, _) = seed_cart_with_shipping(&db, &tenant).await;

        let error = apply_cart_promotion_native_with_context(
            &app,
            &auth,
            &tenant,
            cart_id.to_string(),
            CommerceCartPromotionDraft {
                kind: CommerceCartPromotionKind::FixedDiscount,
                scope: CommerceCartPromotionScope::Shipping,
                line_item_id: String::new(),
                source_id: "promo-shipping-native".to_string(),
                discount_percent: String::new(),
                amount: "4.99".to_string(),
                metadata_json: String::new(),
            },
        )
        .await
        .expect_err("orders:update must be required");

        assert!(
            error
                .to_string()
                .contains("Permission denied: orders:update required"),
            "unexpected error: {error}"
        );
    }
}
