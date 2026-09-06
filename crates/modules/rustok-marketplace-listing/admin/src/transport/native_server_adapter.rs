use leptos::prelude::*;
use std::fmt::{Display, Formatter};

#[cfg(feature = "ssr")]
use crate::model::{
    MarketplaceListingAdminAction, MarketplaceListingAdminListItem, MarketplaceListingAdminRecord,
};
use crate::model::{
    MarketplaceListingAdminCommand, MarketplaceListingAdminCommandResult,
    MarketplaceListingAdminDetail, MarketplaceListingAdminDirectory,
    MarketplaceListingAdminFilters,
};

#[cfg(feature = "ssr")]
const MARKETPLACE_LISTING_ADMIN_NATIVE_OWNER: &str = "rustok_marketplace_listing.admin";
#[cfg(feature = "ssr")]
const MARKETPLACE_LISTING_ADMIN_NATIVE_OPERATION: &str = "native_request";
#[cfg(feature = "ssr")]
const MARKETPLACE_LISTING_ADMIN_NATIVE_BOUNDARY: &str =
    "marketplace_listing_admin_native_transport";

#[cfg(feature = "ssr")]
fn map_runtime_dependency_error(
    action: MarketplaceListingAdminAction,
    dependency: &'static str,
) -> ServerFnError {
    tracing::error!(
        owner = MARKETPLACE_LISTING_ADMIN_NATIVE_OWNER,
        owner_operation = MARKETPLACE_LISTING_ADMIN_NATIVE_OPERATION,
        action = ?action,
        dependency,
        code = "marketplace_listing.admin_runtime_unavailable",
        boundary = MARKETPLACE_LISTING_ADMIN_NATIVE_BOUNDARY,
        "marketplace listing admin native runtime dependency is unavailable"
    );
    ServerFnError::new("Marketplace listing service is temporarily unavailable")
}

#[cfg(feature = "ssr")]
fn map_auth_context_error<E: std::fmt::Display>(
    action: MarketplaceListingAdminAction,
    error: E,
) -> ServerFnError {
    tracing::error!(
        error = %error,
        owner = MARKETPLACE_LISTING_ADMIN_NATIVE_OWNER,
        owner_operation = MARKETPLACE_LISTING_ADMIN_NATIVE_OPERATION,
        action = ?action,
        code = "marketplace_listing.admin_auth_context_unavailable",
        boundary = MARKETPLACE_LISTING_ADMIN_NATIVE_BOUNDARY,
        "marketplace listing admin authentication context extraction failed"
    );
    ServerFnError::new("Marketplace listing request context is unavailable")
}

#[cfg(feature = "ssr")]
fn map_tenant_context_error<E: std::fmt::Display>(
    action: MarketplaceListingAdminAction,
    error: E,
) -> ServerFnError {
    tracing::error!(
        error = %error,
        owner = MARKETPLACE_LISTING_ADMIN_NATIVE_OWNER,
        owner_operation = MARKETPLACE_LISTING_ADMIN_NATIVE_OPERATION,
        action = ?action,
        code = "marketplace_listing.admin_tenant_context_unavailable",
        boundary = MARKETPLACE_LISTING_ADMIN_NATIVE_BOUNDARY,
        "marketplace listing admin tenant context extraction failed"
    );
    ServerFnError::new("Marketplace listing request context is unavailable")
}

#[cfg(feature = "ssr")]
fn map_request_context_error<E: std::fmt::Display>(
    action: MarketplaceListingAdminAction,
    tenant_id: uuid::Uuid,
    error: E,
) -> ServerFnError {
    tracing::error!(
        error = %error,
        owner = MARKETPLACE_LISTING_ADMIN_NATIVE_OWNER,
        owner_operation = MARKETPLACE_LISTING_ADMIN_NATIVE_OPERATION,
        action = ?action,
        tenant_id = %tenant_id,
        code = "marketplace_listing.admin_request_context_unavailable",
        boundary = MARKETPLACE_LISTING_ADMIN_NATIVE_BOUNDARY,
        "marketplace listing admin request context extraction failed"
    );
    ServerFnError::new("Marketplace listing request context is unavailable")
}

#[cfg(feature = "ssr")]
fn map_module_availability_error<E: std::fmt::Display>(
    action: MarketplaceListingAdminAction,
    tenant_id: uuid::Uuid,
    request: &rustok_api::request::RequestContext,
    error: E,
) -> ServerFnError {
    tracing::error!(
        error = %error,
        owner = MARKETPLACE_LISTING_ADMIN_NATIVE_OWNER,
        owner_operation = MARKETPLACE_LISTING_ADMIN_NATIVE_OPERATION,
        action = ?action,
        tenant_id = %tenant_id,
        channel_id = ?request.channel_id,
        channel_slug = ?request.channel_slug,
        locale = %request.locale,
        code = "marketplace_listing.admin_module_availability_failed",
        boundary = MARKETPLACE_LISTING_ADMIN_NATIVE_BOUNDARY,
        "marketplace listing admin module availability check failed"
    );
    ServerFnError::new("Marketplace listing service is temporarily unavailable")
}

#[derive(Debug, Clone)]
pub struct NativeMarketplaceListingAdminError(pub String);

impl Display for NativeMarketplaceListingAdminError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl std::error::Error for NativeMarketplaceListingAdminError {}

impl From<ServerFnError> for NativeMarketplaceListingAdminError {
    fn from(value: ServerFnError) -> Self {
        Self(value.to_string())
    }
}

pub async fn load_directory(
    filters: MarketplaceListingAdminFilters,
) -> Result<MarketplaceListingAdminDirectory, NativeMarketplaceListingAdminError> {
    marketplace_listing_directory_native(filters)
        .await
        .map_err(Into::into)
}

pub async fn load_detail(
    listing_id: String,
) -> Result<MarketplaceListingAdminDetail, NativeMarketplaceListingAdminError> {
    marketplace_listing_detail_native(listing_id)
        .await
        .map_err(Into::into)
}

pub async fn execute_command(
    idempotency_key: String,
    command: MarketplaceListingAdminCommand,
) -> Result<MarketplaceListingAdminCommandResult, NativeMarketplaceListingAdminError> {
    marketplace_listing_command_native(idempotency_key, command)
        .await
        .map_err(Into::into)
}

#[server(prefix = "/api/fn", endpoint = "marketplace-listing/directory")]
async fn marketplace_listing_directory_native(
    filters: MarketplaceListingAdminFilters,
) -> Result<MarketplaceListingAdminDirectory, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_marketplace_listing::{
            ListMarketplaceListingsInput, MarketplaceListingReadPort,
        };

        let (runtime, context) = native_request(MarketplaceListingAdminAction::List, None).await?;
        let page = filters.page.max(1);
        let per_page = filters.per_page.clamp(1, 100);
        let response = MarketplaceListingReadPort::list_listings(
            runtime.ports(),
            context,
            ListMarketplaceListingsInput {
                page,
                per_page,
                seller_id: parse_optional_uuid(filters.seller_id, "seller_id")?,
                master_variant_id: parse_optional_uuid(
                    filters.master_variant_id,
                    "master_variant_id",
                )?,
                market_slug: normalize_optional_text(filters.market_slug),
                channel_slug: normalize_optional_text(filters.channel_slug),
                status: parse_status(filters.status.as_deref())?,
                approval_status: parse_approval_status(filters.approval_status.as_deref())?,
                search: normalize_optional_text(filters.search),
            },
        )
        .await
        .map_err(map_port_error)?;

        Ok(MarketplaceListingAdminDirectory {
            items: response.items.into_iter().map(map_list_item).collect(),
            total: response.total,
            page,
            per_page,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = filters;
        Err(ServerFnError::new(
            "marketplace listing directory requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "marketplace-listing/detail")]
async fn marketplace_listing_detail_native(
    listing_id: String,
) -> Result<MarketplaceListingAdminDetail, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_marketplace_listing::{
            ListMarketplaceListingEventsRequest, MarketplaceListingReadPort,
            ReadMarketplaceListingRequest,
        };

        let (runtime, context) = native_request(MarketplaceListingAdminAction::Read, None).await?;
        let listing_id = parse_uuid(listing_id.as_str(), "listing_id")?;
        let listing = MarketplaceListingReadPort::read_listing(
            runtime.ports(),
            context.clone(),
            ReadMarketplaceListingRequest { listing_id },
        )
        .await
        .map_err(map_port_error)?;
        let events = MarketplaceListingReadPort::list_listing_events(
            runtime.ports(),
            context,
            ListMarketplaceListingEventsRequest {
                listing_id,
                limit: 200,
            },
        )
        .await
        .map_err(map_port_error)?;

        Ok(MarketplaceListingAdminDetail {
            listing: map_listing(listing),
            events: events
                .into_iter()
                .map(|event| crate::model::MarketplaceListingAdminEvent {
                    id: event.id.to_string(),
                    listing_id: event.listing_id.to_string(),
                    actor_id: event.actor_id.map(|value| value.to_string()),
                    event_kind: event.event_kind.as_str().to_string(),
                    locale: event.locale,
                    provenance: event.provenance.as_str().to_string(),
                    note: event.note,
                    metadata: event.metadata,
                    created_at: event.created_at.to_rfc3339(),
                })
                .collect(),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = listing_id;
        Err(ServerFnError::new(
            "marketplace listing detail requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "marketplace-listing/command")]
async fn marketplace_listing_command_native(
    idempotency_key: String,
    command: MarketplaceListingAdminCommand,
) -> Result<MarketplaceListingAdminCommandResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_marketplace_listing::{
            CreateMarketplaceListingInput, MarketplaceListingCommandPort,
            ReviewMarketplaceListingInput, SuspendMarketplaceListingInput,
            UpdateMarketplaceListingTermsInput,
        };

        let idempotency_key = required_text(&idempotency_key, "idempotency_key")?;
        let (runtime, context) =
            native_request(command_action(&command), Some(idempotency_key)).await?;

        let listing = match command {
            MarketplaceListingAdminCommand::Create { draft } => {
                MarketplaceListingCommandPort::create_listing(
                    runtime.ports(),
                    context,
                    CreateMarketplaceListingInput {
                        seller_id: parse_uuid(draft.seller_id.as_str(), "seller_id")?,
                        master_variant_id: parse_uuid(
                            draft.master_variant_id.as_str(),
                            "master_variant_id",
                        )?,
                        seller_sku: required_text(draft.seller_sku.as_str(), "seller_sku")?
                            .to_string(),
                        market_slug: required_text(draft.market_slug.as_str(), "market_slug")?
                            .to_string(),
                        channel_slug: required_text(draft.channel_slug.as_str(), "channel_slug")?
                            .to_string(),
                        pricing_reference: normalize_optional_text(draft.pricing_reference),
                        inventory_reference: normalize_optional_text(draft.inventory_reference),
                        fulfillment_profile_slug: normalize_optional_text(
                            draft.fulfillment_profile_slug,
                        ),
                        metadata: object_or_empty(draft.metadata, "metadata")?,
                    },
                )
                .await
            }
            MarketplaceListingAdminCommand::UpdateTerms { listing_id, draft } => {
                MarketplaceListingCommandPort::update_listing_terms(
                    runtime.ports(),
                    context,
                    UpdateMarketplaceListingTermsInput {
                        listing_id: parse_uuid(listing_id.as_str(), "listing_id")?,
                        pricing_reference: normalize_optional_text(draft.pricing_reference),
                        inventory_reference: normalize_optional_text(draft.inventory_reference),
                        fulfillment_profile_slug: normalize_optional_text(
                            draft.fulfillment_profile_slug,
                        ),
                        metadata: object_or_empty(draft.metadata, "metadata")?,
                    },
                )
                .await
            }
            MarketplaceListingAdminCommand::SubmitForReview { listing_id } => {
                MarketplaceListingCommandPort::submit_listing_for_review(
                    runtime.ports(),
                    context,
                    listing_id_request(listing_id)?,
                )
                .await
            }
            MarketplaceListingAdminCommand::Review {
                listing_id,
                approved,
                note,
            } => {
                MarketplaceListingCommandPort::review_listing(
                    runtime.ports(),
                    context,
                    ReviewMarketplaceListingInput {
                        listing_id: parse_uuid(listing_id.as_str(), "listing_id")?,
                        approved,
                        note: normalize_optional_text(note),
                    },
                )
                .await
            }
            MarketplaceListingAdminCommand::Publish { listing_id } => {
                MarketplaceListingCommandPort::publish_listing(
                    runtime.ports(),
                    context,
                    listing_id_request(listing_id)?,
                )
                .await
            }
            MarketplaceListingAdminCommand::Suspend { listing_id, reason } => {
                MarketplaceListingCommandPort::suspend_listing(
                    runtime.ports(),
                    context,
                    SuspendMarketplaceListingInput {
                        listing_id: parse_uuid(listing_id.as_str(), "listing_id")?,
                        reason: required_text(reason.as_str(), "reason")?.to_string(),
                    },
                )
                .await
            }
            MarketplaceListingAdminCommand::Reactivate { listing_id } => {
                MarketplaceListingCommandPort::reactivate_listing(
                    runtime.ports(),
                    context,
                    listing_id_request(listing_id)?,
                )
                .await
            }
            MarketplaceListingAdminCommand::Archive { listing_id } => {
                MarketplaceListingCommandPort::archive_listing(
                    runtime.ports(),
                    context,
                    listing_id_request(listing_id)?,
                )
                .await
            }
        }
        .map_err(map_port_error)?;

        Ok(MarketplaceListingAdminCommandResult {
            listing: map_listing(listing),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (idempotency_key, command);
        Err(ServerFnError::new(
            "marketplace listing commands require the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
async fn native_request(
    action: MarketplaceListingAdminAction,
    idempotency_key: Option<&str>,
) -> Result<
    (
        rustok_marketplace_listing::MarketplaceListingRuntime,
        rustok_api::PortContext,
    ),
    ServerFnError,
> {
    use rustok_api::request::RequestContext;
    use rustok_api::{AuthContext, HostRuntimeContext, PortActor, TenantContext};

    let host = use_context::<HostRuntimeContext>()
        .ok_or_else(|| map_runtime_dependency_error(action, "HostRuntimeContext"))?;
    let runtime = host
        .shared_get::<rustok_marketplace_listing::MarketplaceListingRuntime>()
        .ok_or_else(|| map_runtime_dependency_error(action, "MarketplaceListingRuntime"))?;
    let auth = leptos_axum::extract::<AuthContext>()
        .await
        .map_err(|error| map_auth_context_error(action, error))?;
    let tenant = leptos_axum::extract::<TenantContext>()
        .await
        .map_err(|error| map_tenant_context_error(action, error))?;
    let request = leptos_axum::extract::<RequestContext>()
        .await
        .map_err(|error| map_request_context_error(action, tenant.id, error))?;

    if !rustok_api::has_effective_permission(&auth.permissions, &action.permission()) {
        return Err(ServerFnError::new(
            "Permission denied: marketplace listing permission required",
        ));
    }
    if auth.tenant_id != tenant.id
        || request.tenant_id != tenant.id
        || request.user_id != Some(auth.user_id)
    {
        return Err(ServerFnError::new(
            "Permission denied: marketplace listing request identity mismatch",
        ));
    }
    let module_enabled =
        rustok_api::is_tenant_module_enabled(host.db(), tenant.id, "marketplace_listing")
            .await
            .map_err(|error| map_module_availability_error(action, tenant.id, &request, error))?;
    if !module_enabled {
        return Err(ServerFnError::new(
            "Marketplace listing module is not enabled for this tenant",
        ));
    }

    let mut context = rustok_api::PortContext::new(
        tenant.id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        request.locale,
        format!("native-marketplace-listing-{}", uuid::Uuid::new_v4()),
    )
    .with_deadline(std::time::Duration::from_secs(5));
    if let Some(channel) = request.channel_slug {
        context = context.with_channel(channel);
    }
    if let Some(key) = idempotency_key {
        context = context.with_idempotency_key(key.to_string());
    }
    Ok((runtime, context))
}

#[cfg(feature = "ssr")]
fn command_action(command: &MarketplaceListingAdminCommand) -> MarketplaceListingAdminAction {
    match command {
        MarketplaceListingAdminCommand::Create { .. } => MarketplaceListingAdminAction::Create,
        MarketplaceListingAdminCommand::UpdateTerms { .. }
        | MarketplaceListingAdminCommand::SubmitForReview { .. } => {
            MarketplaceListingAdminAction::Update
        }
        MarketplaceListingAdminCommand::Review { .. }
        | MarketplaceListingAdminCommand::Suspend { .. } => MarketplaceListingAdminAction::Moderate,
        MarketplaceListingAdminCommand::Publish { .. }
        | MarketplaceListingAdminCommand::Reactivate { .. } => {
            MarketplaceListingAdminAction::Publish
        }
        MarketplaceListingAdminCommand::Archive { .. } => MarketplaceListingAdminAction::Manage,
    }
}

#[cfg(feature = "ssr")]
fn listing_id_request(
    listing_id: String,
) -> Result<rustok_marketplace_listing::MarketplaceListingIdRequest, ServerFnError> {
    Ok(rustok_marketplace_listing::MarketplaceListingIdRequest {
        listing_id: parse_uuid(listing_id.as_str(), "listing_id")?,
    })
}

#[cfg(feature = "ssr")]
fn map_list_item(
    listing: rustok_marketplace_listing::MarketplaceListingResponse,
) -> MarketplaceListingAdminListItem {
    MarketplaceListingAdminListItem {
        id: listing.id.to_string(),
        seller_id: listing.seller_id.to_string(),
        master_variant_id: listing.master_variant_id.to_string(),
        seller_sku: listing.seller_sku,
        market_slug: listing.market_slug,
        channel_slug: listing.channel_slug,
        status: listing.status.as_str().to_string(),
        approval_status: listing.approval_status.as_str().to_string(),
        current_terms_version: listing.current_terms_version,
    }
}

#[cfg(feature = "ssr")]
fn map_listing(
    listing: rustok_marketplace_listing::MarketplaceListingResponse,
) -> MarketplaceListingAdminRecord {
    let terms = listing.current_terms;
    MarketplaceListingAdminRecord {
        id: listing.id.to_string(),
        tenant_id: listing.tenant_id.to_string(),
        seller_id: listing.seller_id.to_string(),
        master_product_id: listing.master_product_id.to_string(),
        master_variant_id: listing.master_variant_id.to_string(),
        seller_sku: listing.seller_sku,
        market_slug: listing.market_slug,
        channel_slug: listing.channel_slug,
        status: listing.status.as_str().to_string(),
        approval_status: listing.approval_status.as_str().to_string(),
        current_terms_version: listing.current_terms_version,
        current_terms: crate::model::MarketplaceListingAdminTerms {
            id: terms.id.to_string(),
            listing_id: terms.listing_id.to_string(),
            version: terms.version,
            pricing_reference: terms.pricing_reference,
            inventory_reference: terms.inventory_reference,
            fulfillment_profile_slug: terms.fulfillment_profile_slug,
            metadata: terms.metadata,
            created_at: terms.created_at.to_rfc3339(),
        },
        metadata: listing.metadata,
        published_at: listing.published_at.map(|value| value.to_rfc3339()),
        approved_at: listing.approved_at.map(|value| value.to_rfc3339()),
        created_at: listing.created_at.to_rfc3339(),
        updated_at: listing.updated_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn parse_status(
    value: Option<&str>,
) -> Result<Option<rustok_marketplace_listing::MarketplaceListingStatus>, ServerFnError> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            rustok_marketplace_listing::MarketplaceListingStatus::parse(value.trim())
                .ok_or_else(|| ServerFnError::new("invalid marketplace listing status"))
        })
        .transpose()
}

#[cfg(feature = "ssr")]
fn parse_approval_status(
    value: Option<&str>,
) -> Result<Option<rustok_marketplace_listing::MarketplaceListingApprovalStatus>, ServerFnError> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            rustok_marketplace_listing::MarketplaceListingApprovalStatus::parse(value.trim())
                .ok_or_else(|| ServerFnError::new("invalid marketplace listing approval status"))
        })
        .transpose()
}

#[cfg(feature = "ssr")]
fn parse_uuid(value: &str, field: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value.trim())
        .map_err(|_| ServerFnError::new(format!("{field} must be a UUID")))
}

#[cfg(feature = "ssr")]
fn parse_optional_uuid(
    value: Option<String>,
    field: &str,
) -> Result<Option<uuid::Uuid>, ServerFnError> {
    value
        .and_then(|value| {
            let value = value.trim().to_string();
            (!value.is_empty()).then_some(value)
        })
        .map(|value| parse_uuid(value.as_str(), field))
        .transpose()
}

#[cfg(feature = "ssr")]
fn required_text<'a>(value: &'a str, field: &str) -> Result<&'a str, ServerFnError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ServerFnError::new(format!("{field} must not be empty")));
    }
    Ok(value)
}

#[cfg(feature = "ssr")]
fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

#[cfg(feature = "ssr")]
fn object_or_empty(
    value: serde_json::Value,
    field: &str,
) -> Result<serde_json::Value, ServerFnError> {
    match value {
        serde_json::Value::Null => Ok(serde_json::json!({})),
        serde_json::Value::Object(_) => Ok(value),
        _ => Err(ServerFnError::new(format!("{field} must be a JSON object"))),
    }
}

#[cfg(feature = "ssr")]
fn map_port_error(error: rustok_api::PortError) -> ServerFnError {
    use rustok_api::PortErrorKind;
    let message = match error.kind {
        PortErrorKind::Validation | PortErrorKind::NotFound | PortErrorKind::Conflict => {
            error.message
        }
        PortErrorKind::Forbidden => "Permission denied: marketplace listing operation".to_string(),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => {
            "Marketplace listing service is temporarily unavailable".to_string()
        }
        PortErrorKind::InvariantViolation => {
            "Marketplace listing command requires operator review".to_string()
        }
    };
    ServerFnError::new(message)
}
