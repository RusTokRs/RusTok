use leptos::prelude::*;
use std::fmt::{Display, Formatter};

use crate::model::{
    MarketplaceListingAdminAction, MarketplaceListingAdminCommand,
    MarketplaceListingAdminCommandResult, MarketplaceListingAdminDetail,
    MarketplaceListingAdminDirectory, MarketplaceListingAdminFilters,
    MarketplaceListingAdminListItem, MarketplaceListingAdminRecord,
};

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

#[cfg(feature = "ssr")]
pub trait MarketplaceListingAdminPorts:
    rustok_marketplace_listing::MarketplaceListingReadPort
    + rustok_marketplace_listing::MarketplaceListingCommandPort
{
}

#[cfg(feature = "ssr")]
impl<T> MarketplaceListingAdminPorts for T where
    T: rustok_marketplace_listing::MarketplaceListingReadPort
        + rustok_marketplace_listing::MarketplaceListingCommandPort
{
}

#[cfg(feature = "ssr")]
pub trait MarketplaceListingAdminRequestScope: Send + Sync {
    /// Platform RBAC remains host-owned, while the module-owned FFA maps every
    /// workflow to one concrete `marketplace_listings:*` permission.
    fn authorize(&self, permission: rustok_api::Permission) -> Result<(), String>;

    /// Must preserve canonical tenant, actor, effective locale, correlation,
    /// deadline, claims, roles, and optional command idempotency identity.
    fn port_context(
        &self,
        idempotency_key: Option<&str>,
    ) -> Result<rustok_api::PortContext, String>;
}

#[cfg(feature = "ssr")]
#[derive(Clone)]
pub struct MarketplaceListingAdminNativeRuntime {
    ports: std::sync::Arc<dyn MarketplaceListingAdminPorts>,
    request_scope: std::sync::Arc<dyn MarketplaceListingAdminRequestScope>,
}

#[cfg(feature = "ssr")]
impl MarketplaceListingAdminNativeRuntime {
    pub fn new(
        ports: std::sync::Arc<dyn MarketplaceListingAdminPorts>,
        request_scope: std::sync::Arc<dyn MarketplaceListingAdminRequestScope>,
    ) -> Self {
        Self {
            ports,
            request_scope,
        }
    }

    fn authorize(&self, action: MarketplaceListingAdminAction) -> Result<(), ServerFnError> {
        self.request_scope
            .authorize(action.permission())
            .map_err(ServerFnError::new)
    }

    fn context(
        &self,
        idempotency_key: Option<&str>,
    ) -> Result<rustok_api::PortContext, ServerFnError> {
        self.request_scope
            .port_context(idempotency_key)
            .map_err(ServerFnError::new)
    }
}

#[cfg(feature = "ssr")]
fn native_runtime() -> Result<MarketplaceListingAdminNativeRuntime, ServerFnError> {
    use_context::<MarketplaceListingAdminNativeRuntime>().ok_or_else(|| {
        ServerFnError::new("marketplace listing native runtime is not mounted in this host")
    })
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

        let runtime = native_runtime()?;
        runtime.authorize(MarketplaceListingAdminAction::List)?;
        let page = filters.page.max(1);
        let per_page = filters.per_page.clamp(1, 100);
        let response = MarketplaceListingReadPort::list_listings(
            runtime.ports.as_ref(),
            runtime.context(None)?,
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

        let runtime = native_runtime()?;
        runtime.authorize(MarketplaceListingAdminAction::Read)?;
        let listing_id = parse_uuid(listing_id.as_str(), "listing_id")?;
        let context = runtime.context(None)?;
        let listing = MarketplaceListingReadPort::read_listing(
            runtime.ports.as_ref(),
            context.clone(),
            ReadMarketplaceListingRequest { listing_id },
        )
        .await
        .map_err(map_port_error)?;
        let events = MarketplaceListingReadPort::list_listing_events(
            runtime.ports.as_ref(),
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

        let runtime = native_runtime()?;
        runtime.authorize(command_action(&command))?;
        let context = runtime.context(Some(required_text(&idempotency_key, "idempotency_key")?))?;

        let listing = match command {
            MarketplaceListingAdminCommand::Create { draft } => {
                MarketplaceListingCommandPort::create_listing(
                    runtime.ports.as_ref(),
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
                    runtime.ports.as_ref(),
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
                    runtime.ports.as_ref(),
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
                    runtime.ports.as_ref(),
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
                    runtime.ports.as_ref(),
                    context,
                    listing_id_request(listing_id)?,
                )
                .await
            }
            MarketplaceListingAdminCommand::Suspend { listing_id, reason } => {
                MarketplaceListingCommandPort::suspend_listing(
                    runtime.ports.as_ref(),
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
                    runtime.ports.as_ref(),
                    context,
                    listing_id_request(listing_id)?,
                )
                .await
            }
            MarketplaceListingAdminCommand::Archive { listing_id } => {
                MarketplaceListingCommandPort::archive_listing(
                    runtime.ports.as_ref(),
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

fn parse_uuid(value: &str, field: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value.trim())
        .map_err(|_| ServerFnError::new(format!("{field} must be a UUID")))
}

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

fn required_text<'a>(value: &'a str, field: &str) -> Result<&'a str, ServerFnError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ServerFnError::new(format!("{field} must not be empty")));
    }
    Ok(value)
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

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
