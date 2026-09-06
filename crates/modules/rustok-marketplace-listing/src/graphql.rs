use std::time::Duration;

use async_graphql::{Context, Enum, FieldError, InputObject, Json, Object, Result, SimpleObject};
use chrono::{DateTime, FixedOffset};
use rustok_api::graphql::{GraphQLError, require_module_enabled};
use rustok_api::request::RequestContext;
use rustok_api::{
    AuthContext, Permission, PortActor, PortContext, PortError, PortErrorKind, TenantContext,
    has_any_effective_permission,
};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    CreateMarketplaceListingInput, ListMarketplaceListingEventsRequest,
    ListMarketplaceListingsInput, MarketplaceListingApprovalStatus, MarketplaceListingCommandPort,
    MarketplaceListingEligibilityProjection, MarketplaceListingEligibilityRequest,
    MarketplaceListingEventResponse, MarketplaceListingIdRequest, MarketplaceListingReadPort,
    MarketplaceListingResponse, MarketplaceListingRuntime, MarketplaceListingStatus,
    MarketplaceListingTermsResponse, ReadMarketplaceListingRequest, ReviewMarketplaceListingInput,
    SuspendMarketplaceListingInput, UpdateMarketplaceListingTermsInput,
};

const PORT_DEADLINE: Duration = Duration::from_secs(5);
const MODULE_SLUG: &str = "marketplace_listing";

pub fn graphql_runtime_data(
    inputs: &rustok_api::graphql::GraphqlRuntimeInputs,
) -> std::result::Result<MarketplaceListingRuntime, String> {
    inputs
        .shared_get::<MarketplaceListingRuntime>()
        .ok_or_else(|| "marketplace listing runtime is not composed".to_string())
}

#[derive(Default)]
pub struct MarketplaceListingQuery;

#[Object]
impl MarketplaceListingQuery {
    async fn marketplace_listings(
        &self,
        ctx: &Context<'_>,
        page: Option<i32>,
        per_page: Option<i32>,
        seller_id: Option<Uuid>,
        master_variant_id: Option<Uuid>,
        market_slug: Option<String>,
        channel_slug: Option<String>,
        status: Option<MarketplaceListingStatusGql>,
        approval_status: Option<MarketplaceListingApprovalStatusGql>,
        search: Option<String>,
    ) -> Result<MarketplaceListingConnectionGql> {
        let auth = require_permissions(ctx, &[Permission::MARKETPLACE_LISTINGS_LIST]).await?;
        let page = page.unwrap_or(1).max(1) as u64;
        let per_page = per_page.unwrap_or(25).clamp(1, 100) as u64;
        let runtime = runtime(ctx)?;
        let response = MarketplaceListingReadPort::list_listings(
            runtime.ports(),
            port_context(ctx, auth, None)?,
            ListMarketplaceListingsInput {
                page,
                per_page,
                seller_id,
                master_variant_id,
                market_slug: normalize_optional_text(market_slug),
                channel_slug: normalize_optional_text(channel_slug),
                status: status.map(Into::into),
                approval_status: approval_status.map(Into::into),
                search: normalize_optional_text(search),
            },
        )
        .await
        .map_err(map_port_error)?;
        Ok(MarketplaceListingConnectionGql {
            items: response.items.into_iter().map(Into::into).collect(),
            total: response.total,
            page,
            per_page,
        })
    }

    async fn marketplace_listing(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> Result<MarketplaceListingGql> {
        let auth = require_permissions(ctx, &[Permission::MARKETPLACE_LISTINGS_READ]).await?;
        let runtime = runtime(ctx)?;
        MarketplaceListingReadPort::read_listing(
            runtime.ports(),
            port_context(ctx, auth, None)?,
            ReadMarketplaceListingRequest { listing_id: id },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn marketplace_listing_events(
        &self,
        ctx: &Context<'_>,
        listing_id: Uuid,
        limit: Option<i32>,
    ) -> Result<Vec<MarketplaceListingEventGql>> {
        let auth = require_permissions(ctx, &[Permission::MARKETPLACE_LISTINGS_READ]).await?;
        let runtime = runtime(ctx)?;
        MarketplaceListingReadPort::list_listing_events(
            runtime.ports(),
            port_context(ctx, auth, None)?,
            ListMarketplaceListingEventsRequest {
                listing_id,
                limit: limit.unwrap_or(100).clamp(1, 200) as u64,
            },
        )
        .await
        .map(|events| events.into_iter().map(Into::into).collect())
        .map_err(map_port_error)
    }

    async fn marketplace_listing_eligibility(
        &self,
        ctx: &Context<'_>,
        master_variant_id: Uuid,
        market_slug: String,
        channel_slug: String,
    ) -> Result<Vec<MarketplaceListingEligibilityGql>> {
        let auth = require_permissions(ctx, &[Permission::MARKETPLACE_LISTINGS_READ]).await?;
        let runtime = runtime(ctx)?;
        MarketplaceListingReadPort::list_eligibility(
            runtime.ports(),
            port_context(ctx, auth, None)?,
            MarketplaceListingEligibilityRequest {
                master_variant_id,
                market_slug,
                channel_slug,
            },
        )
        .await
        .map(|items| items.into_iter().map(Into::into).collect())
        .map_err(map_port_error)
    }
}

#[derive(Default)]
pub struct MarketplaceListingMutation;

#[Object]
impl MarketplaceListingMutation {
    async fn create_marketplace_listing(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        input: MarketplaceListingCreateInputGql,
    ) -> Result<MarketplaceListingGql> {
        let auth = require_permissions(ctx, &[Permission::MARKETPLACE_LISTINGS_CREATE]).await?;
        let runtime = runtime(ctx)?;
        MarketplaceListingCommandPort::create_listing(
            runtime.ports(),
            port_context(ctx, auth, Some(idempotency_key))?,
            CreateMarketplaceListingInput {
                seller_id: input.seller_id,
                master_variant_id: input.master_variant_id,
                seller_sku: input.seller_sku,
                market_slug: input.market_slug,
                channel_slug: input.channel_slug,
                pricing_reference: normalize_optional_text(input.pricing_reference),
                inventory_reference: normalize_optional_text(input.inventory_reference),
                fulfillment_profile_slug: normalize_optional_text(input.fulfillment_profile_slug),
                metadata: input
                    .metadata
                    .map(|value| value.0)
                    .unwrap_or_else(empty_object),
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn update_marketplace_listing_terms(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        listing_id: Uuid,
        input: MarketplaceListingTermsInputGql,
    ) -> Result<MarketplaceListingGql> {
        let auth = require_permissions(ctx, &[Permission::MARKETPLACE_LISTINGS_UPDATE]).await?;
        let runtime = runtime(ctx)?;
        MarketplaceListingCommandPort::update_listing_terms(
            runtime.ports(),
            port_context(ctx, auth, Some(idempotency_key))?,
            UpdateMarketplaceListingTermsInput {
                listing_id,
                pricing_reference: normalize_optional_text(input.pricing_reference),
                inventory_reference: normalize_optional_text(input.inventory_reference),
                fulfillment_profile_slug: normalize_optional_text(input.fulfillment_profile_slug),
                metadata: input
                    .metadata
                    .map(|value| value.0)
                    .unwrap_or_else(empty_object),
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn submit_marketplace_listing_for_review(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        listing_id: Uuid,
    ) -> Result<MarketplaceListingGql> {
        execute_id_command(
            ctx,
            Permission::MARKETPLACE_LISTINGS_UPDATE,
            idempotency_key,
            listing_id,
            IdCommand::Submit,
        )
        .await
    }

    async fn review_marketplace_listing(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        listing_id: Uuid,
        approved: bool,
        note: Option<String>,
    ) -> Result<MarketplaceListingGql> {
        let auth = require_permissions(ctx, &[Permission::MARKETPLACE_LISTINGS_MODERATE]).await?;
        let runtime = runtime(ctx)?;
        MarketplaceListingCommandPort::review_listing(
            runtime.ports(),
            port_context(ctx, auth, Some(idempotency_key))?,
            ReviewMarketplaceListingInput {
                listing_id,
                approved,
                note: normalize_optional_text(note),
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn publish_marketplace_listing(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        listing_id: Uuid,
    ) -> Result<MarketplaceListingGql> {
        execute_id_command(
            ctx,
            Permission::MARKETPLACE_LISTINGS_PUBLISH,
            idempotency_key,
            listing_id,
            IdCommand::Publish,
        )
        .await
    }

    async fn suspend_marketplace_listing(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        listing_id: Uuid,
        reason: String,
    ) -> Result<MarketplaceListingGql> {
        let auth = require_permissions(ctx, &[Permission::MARKETPLACE_LISTINGS_MODERATE]).await?;
        let runtime = runtime(ctx)?;
        MarketplaceListingCommandPort::suspend_listing(
            runtime.ports(),
            port_context(ctx, auth, Some(idempotency_key))?,
            SuspendMarketplaceListingInput { listing_id, reason },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn reactivate_marketplace_listing(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        listing_id: Uuid,
    ) -> Result<MarketplaceListingGql> {
        execute_id_command(
            ctx,
            Permission::MARKETPLACE_LISTINGS_PUBLISH,
            idempotency_key,
            listing_id,
            IdCommand::Reactivate,
        )
        .await
    }

    async fn archive_marketplace_listing(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        listing_id: Uuid,
    ) -> Result<MarketplaceListingGql> {
        execute_id_command(
            ctx,
            Permission::MARKETPLACE_LISTINGS_MANAGE,
            idempotency_key,
            listing_id,
            IdCommand::Archive,
        )
        .await
    }
}

enum IdCommand {
    Submit,
    Publish,
    Reactivate,
    Archive,
}

async fn execute_id_command(
    ctx: &Context<'_>,
    permission: Permission,
    idempotency_key: String,
    listing_id: Uuid,
    command: IdCommand,
) -> Result<MarketplaceListingGql> {
    let auth = require_permissions(ctx, &[permission]).await?;
    let runtime = runtime(ctx)?;
    let context = port_context(ctx, auth, Some(idempotency_key))?;
    let request = MarketplaceListingIdRequest { listing_id };
    let result = match command {
        IdCommand::Submit => {
            MarketplaceListingCommandPort::submit_listing_for_review(
                runtime.ports(),
                context,
                request,
            )
            .await
        }
        IdCommand::Publish => {
            MarketplaceListingCommandPort::publish_listing(runtime.ports(), context, request).await
        }
        IdCommand::Reactivate => {
            MarketplaceListingCommandPort::reactivate_listing(runtime.ports(), context, request)
                .await
        }
        IdCommand::Archive => {
            MarketplaceListingCommandPort::archive_listing(runtime.ports(), context, request).await
        }
    };
    result.map(Into::into).map_err(map_port_error)
}

#[derive(SimpleObject)]
pub struct MarketplaceListingConnectionGql {
    pub items: Vec<MarketplaceListingGql>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

#[derive(SimpleObject)]
pub struct MarketplaceListingGql {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub seller_id: Uuid,
    pub master_product_id: Uuid,
    pub master_variant_id: Uuid,
    pub seller_sku: String,
    pub market_slug: String,
    pub channel_slug: String,
    pub status: MarketplaceListingStatusGql,
    pub approval_status: MarketplaceListingApprovalStatusGql,
    pub current_terms_version: i32,
    pub current_terms: MarketplaceListingTermsGql,
    pub metadata: Json<Value>,
    pub published_at: Option<DateTime<FixedOffset>>,
    pub approved_at: Option<DateTime<FixedOffset>>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(SimpleObject)]
pub struct MarketplaceListingTermsGql {
    pub id: Uuid,
    pub listing_id: Uuid,
    pub version: i32,
    pub pricing_reference: Option<String>,
    pub inventory_reference: Option<String>,
    pub fulfillment_profile_slug: Option<String>,
    pub metadata: Json<Value>,
    pub created_at: DateTime<FixedOffset>,
}

#[derive(SimpleObject)]
pub struct MarketplaceListingEventGql {
    pub id: Uuid,
    pub listing_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub event_kind: String,
    pub locale: Option<String>,
    pub provenance: String,
    pub note: Option<String>,
    pub metadata: Json<Value>,
    pub created_at: DateTime<FixedOffset>,
}

#[derive(SimpleObject)]
pub struct MarketplaceListingEligibilityGql {
    pub listing: MarketplaceListingGql,
    pub eligible: bool,
    pub reason_codes: Vec<String>,
}

#[derive(InputObject)]
pub struct MarketplaceListingCreateInputGql {
    pub seller_id: Uuid,
    pub master_variant_id: Uuid,
    pub seller_sku: String,
    pub market_slug: String,
    pub channel_slug: String,
    pub pricing_reference: Option<String>,
    pub inventory_reference: Option<String>,
    pub fulfillment_profile_slug: Option<String>,
    pub metadata: Option<Json<Value>>,
}

#[derive(InputObject)]
pub struct MarketplaceListingTermsInputGql {
    pub pricing_reference: Option<String>,
    pub inventory_reference: Option<String>,
    pub fulfillment_profile_slug: Option<String>,
    pub metadata: Option<Json<Value>>,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum MarketplaceListingStatusGql {
    Draft,
    PendingReview,
    Active,
    Suspended,
    Archived,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum MarketplaceListingApprovalStatusGql {
    Draft,
    Pending,
    Approved,
    Rejected,
}

impl From<MarketplaceListingResponse> for MarketplaceListingGql {
    fn from(value: MarketplaceListingResponse) -> Self {
        Self {
            id: value.id,
            tenant_id: value.tenant_id,
            seller_id: value.seller_id,
            master_product_id: value.master_product_id,
            master_variant_id: value.master_variant_id,
            seller_sku: value.seller_sku,
            market_slug: value.market_slug,
            channel_slug: value.channel_slug,
            status: value.status.into(),
            approval_status: value.approval_status.into(),
            current_terms_version: value.current_terms_version,
            current_terms: value.current_terms.into(),
            metadata: Json(value.metadata),
            published_at: value.published_at,
            approved_at: value.approved_at,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<MarketplaceListingTermsResponse> for MarketplaceListingTermsGql {
    fn from(value: MarketplaceListingTermsResponse) -> Self {
        Self {
            id: value.id,
            listing_id: value.listing_id,
            version: value.version,
            pricing_reference: value.pricing_reference,
            inventory_reference: value.inventory_reference,
            fulfillment_profile_slug: value.fulfillment_profile_slug,
            metadata: Json(value.metadata),
            created_at: value.created_at,
        }
    }
}

impl From<MarketplaceListingEventResponse> for MarketplaceListingEventGql {
    fn from(value: MarketplaceListingEventResponse) -> Self {
        Self {
            id: value.id,
            listing_id: value.listing_id,
            actor_id: value.actor_id,
            event_kind: value.event_kind.as_str().to_string(),
            locale: value.locale,
            provenance: value.provenance.as_str().to_string(),
            note: value.note,
            metadata: Json(value.metadata),
            created_at: value.created_at,
        }
    }
}

impl From<MarketplaceListingEligibilityProjection> for MarketplaceListingEligibilityGql {
    fn from(value: MarketplaceListingEligibilityProjection) -> Self {
        Self {
            listing: value.listing.into(),
            eligible: value.eligible,
            reason_codes: value.reason_codes,
        }
    }
}

impl From<MarketplaceListingStatusGql> for MarketplaceListingStatus {
    fn from(value: MarketplaceListingStatusGql) -> Self {
        match value {
            MarketplaceListingStatusGql::Draft => Self::Draft,
            MarketplaceListingStatusGql::PendingReview => Self::PendingReview,
            MarketplaceListingStatusGql::Active => Self::Active,
            MarketplaceListingStatusGql::Suspended => Self::Suspended,
            MarketplaceListingStatusGql::Archived => Self::Archived,
        }
    }
}

impl From<MarketplaceListingStatus> for MarketplaceListingStatusGql {
    fn from(value: MarketplaceListingStatus) -> Self {
        match value {
            MarketplaceListingStatus::Draft => Self::Draft,
            MarketplaceListingStatus::PendingReview => Self::PendingReview,
            MarketplaceListingStatus::Active => Self::Active,
            MarketplaceListingStatus::Suspended => Self::Suspended,
            MarketplaceListingStatus::Archived => Self::Archived,
        }
    }
}

impl From<MarketplaceListingApprovalStatusGql> for MarketplaceListingApprovalStatus {
    fn from(value: MarketplaceListingApprovalStatusGql) -> Self {
        match value {
            MarketplaceListingApprovalStatusGql::Draft => Self::Draft,
            MarketplaceListingApprovalStatusGql::Pending => Self::Pending,
            MarketplaceListingApprovalStatusGql::Approved => Self::Approved,
            MarketplaceListingApprovalStatusGql::Rejected => Self::Rejected,
        }
    }
}

impl From<MarketplaceListingApprovalStatus> for MarketplaceListingApprovalStatusGql {
    fn from(value: MarketplaceListingApprovalStatus) -> Self {
        match value {
            MarketplaceListingApprovalStatus::Draft => Self::Draft,
            MarketplaceListingApprovalStatus::Pending => Self::Pending,
            MarketplaceListingApprovalStatus::Approved => Self::Approved,
            MarketplaceListingApprovalStatus::Rejected => Self::Rejected,
        }
    }
}

fn runtime(ctx: &Context<'_>) -> Result<MarketplaceListingRuntime> {
    ctx.data::<MarketplaceListingRuntime>()
        .cloned()
        .map_err(|_| {
            <FieldError as GraphQLError>::internal_error(
                "Marketplace listing runtime is not registered",
            )
        })
}

async fn require_permissions<'a>(
    ctx: &'a Context<'a>,
    required: &[Permission],
) -> Result<&'a AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    if !has_any_effective_permission(&auth.permissions, required) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "marketplace listing permission required",
        ));
    }
    require_tenant(ctx, auth)?;
    require_module_enabled(ctx, MODULE_SLUG).await?;
    Ok(auth)
}

fn require_tenant<'a>(ctx: &'a Context<'a>, auth: &AuthContext) -> Result<&'a TenantContext> {
    let tenant = ctx.data::<TenantContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error(
            "Marketplace listing tenant context is not registered",
        )
    })?;
    if auth.tenant_id != tenant.id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "marketplace listing tenant mismatch",
        ));
    }
    Ok(tenant)
}

fn port_context(
    ctx: &Context<'_>,
    auth: &AuthContext,
    idempotency_key: Option<String>,
) -> Result<PortContext> {
    let tenant = require_tenant(ctx, auth)?;
    let request = ctx.data::<RequestContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error(
            "Marketplace listing request context is not registered",
        )
    })?;
    if request.tenant_id != tenant.id || request.user_id != Some(auth.user_id) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "marketplace listing request identity mismatch",
        ));
    }
    let mut context = PortContext::new(
        tenant.id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        request.locale.clone(),
        format!("graphql-marketplace-listing-{}", Uuid::new_v4()),
    )
    .with_deadline(PORT_DEADLINE);
    if let Some(channel) = request.channel_slug.clone() {
        context = context.with_channel(channel);
    }
    if let Some(key) = idempotency_key {
        context = context.with_idempotency_key(key);
    }
    Ok(context)
}

fn map_port_error(error: PortError) -> FieldError {
    match error.kind {
        PortErrorKind::Validation | PortErrorKind::Conflict => {
            <FieldError as GraphQLError>::bad_user_input(&error.message)
        }
        PortErrorKind::NotFound => <FieldError as GraphQLError>::not_found(&error.message),
        PortErrorKind::Forbidden => <FieldError as GraphQLError>::permission_denied(&error.message),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => {
            <FieldError as GraphQLError>::internal_error(
                "Marketplace listing service is temporarily unavailable",
            )
        }
        PortErrorKind::InvariantViolation => <FieldError as GraphQLError>::internal_error(
            "Marketplace listing command requires operator review",
        ),
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn empty_object() -> Value {
    serde_json::json!({})
}
