use std::time::Duration;

use async_graphql::{
    Context, Enum, ErrorExtensions, FieldError, InputObject, Json, Object, Result, SimpleObject,
};
use chrono::{DateTime, FixedOffset};
use rustok_api::graphql::GraphQLError;
use rustok_api::request::RequestContext;
use rustok_api::{
    AuthContext, ChannelContext, HostRuntimeContext, Permission, PortActor, PortContext, PortError,
    PortErrorKind, TenantContext, has_any_effective_permission,
};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    AddMarketplaceSellerMemberInput, AddMarketplaceSellerMemberRequest,
    CreateMarketplaceSellerInput, ListMarketplaceSellerMembersRequest, ListMarketplaceSellersInput,
    MarketplaceSellerCommandPort, MarketplaceSellerMemberResponse, MarketplaceSellerMemberRole,
    MarketplaceSellerMemberStatus, MarketplaceSellerOnboardingStatus, MarketplaceSellerReadPort,
    MarketplaceSellerResponse, MarketplaceSellerService, MarketplaceSellerStatus,
    ReactivateMarketplaceSellerRequest, ReadMarketplaceSellerRequest,
    ReviewMarketplaceSellerOnboardingInput, ReviewMarketplaceSellerOnboardingRequest,
    SubmitMarketplaceSellerOnboardingInput, SubmitMarketplaceSellerOnboardingRequest,
    SuspendMarketplaceSellerInput, SuspendMarketplaceSellerRequest,
    UpdateMarketplaceSellerMemberInput, UpdateMarketplaceSellerMemberRequest,
    UpdateMarketplaceSellerProfileInput, UpdateMarketplaceSellerProfileRequest,
};

const PORT_DEADLINE: Duration = Duration::from_secs(5);

#[derive(Default)]
pub struct MarketplaceSellerQuery;

#[Object]
impl MarketplaceSellerQuery {
    async fn marketplace_sellers(
        &self,
        ctx: &Context<'_>,
        page: Option<i32>,
        per_page: Option<i32>,
        status: Option<MarketplaceSellerStatusGql>,
        onboarding_status: Option<MarketplaceSellerOnboardingStatusGql>,
        search: Option<String>,
    ) -> Result<MarketplaceSellerConnectionGql> {
        let auth = require_permissions(
            ctx,
            &[
                Permission::MARKETPLACE_SELLERS_LIST,
                Permission::MARKETPLACE_SELLERS_READ,
            ],
        )?;
        let page = page.unwrap_or(1).max(1) as u64;
        let per_page = per_page.unwrap_or(25).clamp(1, 100) as u64;
        let service = service(ctx)?;
        let result = MarketplaceSellerReadPort::list_sellers(
            &service,
            port_context(ctx, auth, None)?,
            ListMarketplaceSellersInput {
                page,
                per_page,
                status: status.map(Into::into),
                onboarding_status: onboarding_status.map(Into::into),
                search: normalize_optional_text(search),
            },
        )
        .await
        .map_err(map_port_error)?;
        Ok(MarketplaceSellerConnectionGql {
            items: result.items.into_iter().map(Into::into).collect(),
            total: result.total,
            page,
            per_page,
        })
    }

    async fn marketplace_seller(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> Result<MarketplaceSellerGql> {
        let auth = require_permissions(ctx, &[Permission::MARKETPLACE_SELLERS_READ])?;
        let service = service(ctx)?;
        MarketplaceSellerReadPort::read_seller(
            &service,
            port_context(ctx, auth, None)?,
            ReadMarketplaceSellerRequest { seller_id: id },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn marketplace_seller_members(
        &self,
        ctx: &Context<'_>,
        seller_id: Uuid,
    ) -> Result<Vec<MarketplaceSellerMemberGql>> {
        let auth = require_permissions(ctx, &[Permission::MARKETPLACE_SELLERS_READ])?;
        let service = service(ctx)?;
        MarketplaceSellerReadPort::list_members(
            &service,
            port_context(ctx, auth, None)?,
            ListMarketplaceSellerMembersRequest { seller_id },
        )
        .await
        .map(|items| items.into_iter().map(Into::into).collect())
        .map_err(map_port_error)
    }
}

#[derive(Default)]
pub struct MarketplaceSellerMutation;

#[Object]
impl MarketplaceSellerMutation {
    async fn create_marketplace_seller(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        input: MarketplaceSellerCreateInputGql,
    ) -> Result<MarketplaceSellerGql> {
        let auth = require_permissions(ctx, &[Permission::MARKETPLACE_SELLERS_CREATE])?;
        let service = service(ctx)?;
        MarketplaceSellerCommandPort::create_seller(
            &service,
            port_context(ctx, auth, Some(idempotency_key))?,
            CreateMarketplaceSellerInput {
                handle: input.handle,
                display_name: input.display_name,
                legal_name: input.legal_name,
                owner_user_id: input.owner_user_id,
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

    async fn update_marketplace_seller_profile(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        seller_id: Uuid,
        input: MarketplaceSellerProfileInputGql,
    ) -> Result<MarketplaceSellerGql> {
        let auth = require_permissions(ctx, &[Permission::MARKETPLACE_SELLERS_UPDATE])?;
        let service = service(ctx)?;
        MarketplaceSellerCommandPort::update_seller_profile(
            &service,
            port_context(ctx, auth, Some(idempotency_key))?,
            UpdateMarketplaceSellerProfileRequest {
                seller_id,
                input: UpdateMarketplaceSellerProfileInput {
                    display_name: input.display_name,
                    legal_name: input.legal_name,
                    metadata: input.metadata.map(|value| value.0),
                },
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn submit_marketplace_seller_onboarding(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        seller_id: Uuid,
        note: Option<String>,
    ) -> Result<MarketplaceSellerGql> {
        let auth = require_permissions(ctx, &[Permission::MARKETPLACE_SELLERS_UPDATE])?;
        let service = service(ctx)?;
        MarketplaceSellerCommandPort::submit_seller_onboarding(
            &service,
            port_context(ctx, auth, Some(idempotency_key))?,
            SubmitMarketplaceSellerOnboardingRequest {
                seller_id,
                input: SubmitMarketplaceSellerOnboardingInput { note },
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn review_marketplace_seller_onboarding(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        seller_id: Uuid,
        approved: bool,
        note: Option<String>,
    ) -> Result<MarketplaceSellerGql> {
        let auth = require_permissions(ctx, &[Permission::MARKETPLACE_SELLERS_MANAGE])?;
        let service = service(ctx)?;
        MarketplaceSellerCommandPort::review_seller_onboarding(
            &service,
            port_context(ctx, auth, Some(idempotency_key))?,
            ReviewMarketplaceSellerOnboardingRequest {
                seller_id,
                input: ReviewMarketplaceSellerOnboardingInput { approved, note },
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn suspend_marketplace_seller(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        seller_id: Uuid,
        reason: String,
    ) -> Result<MarketplaceSellerGql> {
        let auth = require_permissions(ctx, &[Permission::MARKETPLACE_SELLERS_MANAGE])?;
        let service = service(ctx)?;
        MarketplaceSellerCommandPort::suspend_seller(
            &service,
            port_context(ctx, auth, Some(idempotency_key))?,
            SuspendMarketplaceSellerRequest {
                seller_id,
                input: SuspendMarketplaceSellerInput { reason },
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn reactivate_marketplace_seller(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        seller_id: Uuid,
    ) -> Result<MarketplaceSellerGql> {
        let auth = require_permissions(ctx, &[Permission::MARKETPLACE_SELLERS_MANAGE])?;
        let service = service(ctx)?;
        MarketplaceSellerCommandPort::reactivate_seller(
            &service,
            port_context(ctx, auth, Some(idempotency_key))?,
            ReactivateMarketplaceSellerRequest { seller_id },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn add_marketplace_seller_member(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        seller_id: Uuid,
        input: MarketplaceSellerMemberCreateInputGql,
    ) -> Result<MarketplaceSellerMemberGql> {
        let auth = require_permissions(ctx, &[Permission::MARKETPLACE_SELLERS_UPDATE])?;
        let service = service(ctx)?;
        MarketplaceSellerCommandPort::add_seller_member(
            &service,
            port_context(ctx, auth, Some(idempotency_key))?,
            AddMarketplaceSellerMemberRequest {
                seller_id,
                input: AddMarketplaceSellerMemberInput {
                    user_id: input.user_id,
                    role: input.role.into(),
                    metadata: input
                        .metadata
                        .map(|value| value.0)
                        .unwrap_or_else(empty_object),
                },
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn update_marketplace_seller_member(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        seller_id: Uuid,
        member_id: Uuid,
        input: MarketplaceSellerMemberUpdateInputGql,
    ) -> Result<MarketplaceSellerMemberGql> {
        let auth = require_permissions(ctx, &[Permission::MARKETPLACE_SELLERS_UPDATE])?;
        let service = service(ctx)?;
        MarketplaceSellerCommandPort::update_seller_member(
            &service,
            port_context(ctx, auth, Some(idempotency_key))?,
            UpdateMarketplaceSellerMemberRequest {
                seller_id,
                member_id,
                input: UpdateMarketplaceSellerMemberInput {
                    role: input.role.map(Into::into),
                    status: input.status.map(Into::into),
                    metadata: input.metadata.map(|value| value.0),
                },
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }
}

#[derive(SimpleObject)]
pub struct MarketplaceSellerConnectionGql {
    pub items: Vec<MarketplaceSellerGql>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

#[derive(SimpleObject)]
pub struct MarketplaceSellerGql {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub handle: String,
    pub resolved_locale: String,
    pub display_name: String,
    pub legal_name: Option<String>,
    pub status: MarketplaceSellerStatusGql,
    pub onboarding_status: MarketplaceSellerOnboardingStatusGql,
    pub onboarding_note: Option<String>,
    pub suspension_reason: Option<String>,
    pub metadata: Json<Value>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub activated_at: Option<DateTime<FixedOffset>>,
    pub suspended_at: Option<DateTime<FixedOffset>>,
}

#[derive(SimpleObject)]
pub struct MarketplaceSellerMemberGql {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub seller_id: Uuid,
    pub user_id: Uuid,
    pub role: MarketplaceSellerMemberRoleGql,
    pub status: MarketplaceSellerMemberStatusGql,
    pub invited_by_actor_id: Option<Uuid>,
    pub accepted_at: Option<DateTime<FixedOffset>>,
    pub metadata: Json<Value>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(InputObject)]
pub struct MarketplaceSellerCreateInputGql {
    pub handle: String,
    pub display_name: String,
    pub legal_name: Option<String>,
    pub owner_user_id: Uuid,
    pub metadata: Option<Json<Value>>,
}

#[derive(InputObject)]
pub struct MarketplaceSellerProfileInputGql {
    pub display_name: Option<String>,
    pub legal_name: Option<String>,
    pub metadata: Option<Json<Value>>,
}

#[derive(InputObject)]
pub struct MarketplaceSellerMemberCreateInputGql {
    pub user_id: Uuid,
    pub role: MarketplaceSellerMemberRoleGql,
    pub metadata: Option<Json<Value>>,
}

#[derive(InputObject)]
pub struct MarketplaceSellerMemberUpdateInputGql {
    pub role: Option<MarketplaceSellerMemberRoleGql>,
    pub status: Option<MarketplaceSellerMemberStatusGql>,
    pub metadata: Option<Json<Value>>,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum MarketplaceSellerStatusGql {
    Draft,
    Active,
    Suspended,
    Closed,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum MarketplaceSellerOnboardingStatusGql {
    Draft,
    Submitted,
    Approved,
    Rejected,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum MarketplaceSellerMemberRoleGql {
    Owner,
    Admin,
    Operations,
    Finance,
    Member,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum MarketplaceSellerMemberStatusGql {
    Invited,
    Active,
    Disabled,
}

impl From<MarketplaceSellerResponse> for MarketplaceSellerGql {
    fn from(value: MarketplaceSellerResponse) -> Self {
        Self {
            id: value.id,
            tenant_id: value.tenant_id,
            handle: value.handle,
            resolved_locale: value.resolved_locale,
            display_name: value.display_name,
            legal_name: value.legal_name,
            status: value.status.into(),
            onboarding_status: value.onboarding_status.into(),
            onboarding_note: value.onboarding_note,
            suspension_reason: value.suspension_reason,
            metadata: Json(value.metadata),
            created_at: value.created_at,
            updated_at: value.updated_at,
            activated_at: value.activated_at,
            suspended_at: value.suspended_at,
        }
    }
}

impl From<MarketplaceSellerMemberResponse> for MarketplaceSellerMemberGql {
    fn from(value: MarketplaceSellerMemberResponse) -> Self {
        Self {
            id: value.id,
            tenant_id: value.tenant_id,
            seller_id: value.seller_id,
            user_id: value.user_id,
            role: value.role.into(),
            status: value.status.into(),
            invited_by_actor_id: value.invited_by_actor_id,
            accepted_at: value.accepted_at,
            metadata: Json(value.metadata),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<MarketplaceSellerStatusGql> for MarketplaceSellerStatus {
    fn from(value: MarketplaceSellerStatusGql) -> Self {
        match value {
            MarketplaceSellerStatusGql::Draft => Self::Draft,
            MarketplaceSellerStatusGql::Active => Self::Active,
            MarketplaceSellerStatusGql::Suspended => Self::Suspended,
            MarketplaceSellerStatusGql::Closed => Self::Closed,
        }
    }
}

impl From<MarketplaceSellerStatus> for MarketplaceSellerStatusGql {
    fn from(value: MarketplaceSellerStatus) -> Self {
        match value {
            MarketplaceSellerStatus::Draft => Self::Draft,
            MarketplaceSellerStatus::Active => Self::Active,
            MarketplaceSellerStatus::Suspended => Self::Suspended,
            MarketplaceSellerStatus::Closed => Self::Closed,
        }
    }
}

impl From<MarketplaceSellerOnboardingStatusGql> for MarketplaceSellerOnboardingStatus {
    fn from(value: MarketplaceSellerOnboardingStatusGql) -> Self {
        match value {
            MarketplaceSellerOnboardingStatusGql::Draft => Self::Draft,
            MarketplaceSellerOnboardingStatusGql::Submitted => Self::Submitted,
            MarketplaceSellerOnboardingStatusGql::Approved => Self::Approved,
            MarketplaceSellerOnboardingStatusGql::Rejected => Self::Rejected,
        }
    }
}

impl From<MarketplaceSellerOnboardingStatus> for MarketplaceSellerOnboardingStatusGql {
    fn from(value: MarketplaceSellerOnboardingStatus) -> Self {
        match value {
            MarketplaceSellerOnboardingStatus::Draft => Self::Draft,
            MarketplaceSellerOnboardingStatus::Submitted => Self::Submitted,
            MarketplaceSellerOnboardingStatus::Approved => Self::Approved,
            MarketplaceSellerOnboardingStatus::Rejected => Self::Rejected,
        }
    }
}

impl From<MarketplaceSellerMemberRoleGql> for MarketplaceSellerMemberRole {
    fn from(value: MarketplaceSellerMemberRoleGql) -> Self {
        match value {
            MarketplaceSellerMemberRoleGql::Owner => Self::Owner,
            MarketplaceSellerMemberRoleGql::Admin => Self::Admin,
            MarketplaceSellerMemberRoleGql::Operations => Self::Operations,
            MarketplaceSellerMemberRoleGql::Finance => Self::Finance,
            MarketplaceSellerMemberRoleGql::Member => Self::Member,
        }
    }
}

impl From<MarketplaceSellerMemberRole> for MarketplaceSellerMemberRoleGql {
    fn from(value: MarketplaceSellerMemberRole) -> Self {
        match value {
            MarketplaceSellerMemberRole::Owner => Self::Owner,
            MarketplaceSellerMemberRole::Admin => Self::Admin,
            MarketplaceSellerMemberRole::Operations => Self::Operations,
            MarketplaceSellerMemberRole::Finance => Self::Finance,
            MarketplaceSellerMemberRole::Member => Self::Member,
        }
    }
}

impl From<MarketplaceSellerMemberStatusGql> for MarketplaceSellerMemberStatus {
    fn from(value: MarketplaceSellerMemberStatusGql) -> Self {
        match value {
            MarketplaceSellerMemberStatusGql::Invited => Self::Invited,
            MarketplaceSellerMemberStatusGql::Active => Self::Active,
            MarketplaceSellerMemberStatusGql::Disabled => Self::Disabled,
        }
    }
}

impl From<MarketplaceSellerMemberStatus> for MarketplaceSellerMemberStatusGql {
    fn from(value: MarketplaceSellerMemberStatus) -> Self {
        match value {
            MarketplaceSellerMemberStatus::Invited => Self::Invited,
            MarketplaceSellerMemberStatus::Active => Self::Active,
            MarketplaceSellerMemberStatus::Disabled => Self::Disabled,
        }
    }
}

fn service(ctx: &Context<'_>) -> Result<MarketplaceSellerService> {
    let runtime = ctx.data::<HostRuntimeContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error("Marketplace seller runtime is not registered")
    })?;
    Ok(MarketplaceSellerService::new(runtime.db_clone()))
}

fn require_permissions<'a>(
    ctx: &'a Context<'a>,
    required: &[Permission],
) -> Result<&'a AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    if !has_any_effective_permission(&auth.permissions, required) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "marketplace seller permission required",
        ));
    }
    require_tenant(ctx, auth)?;
    Ok(auth)
}

fn require_tenant<'a>(ctx: &'a Context<'a>, auth: &AuthContext) -> Result<&'a TenantContext> {
    let tenant = ctx.data::<TenantContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error(
            "Marketplace seller tenant context is not registered",
        )
    })?;
    if auth.tenant_id != tenant.id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "marketplace seller tenant mismatch",
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
    let locale = ctx
        .data::<RequestContext>()
        .map(|request| request.locale.clone())
        .or_else(|_| {
            ctx.data::<rustok_core::Locale>()
                .map(|locale| locale.as_str().to_string())
        })
        .unwrap_or_else(|_| tenant.default_locale.clone());
    let mut context = PortContext::new(
        tenant.id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        locale,
        format!("graphql-marketplace-seller-{}", Uuid::new_v4()),
    )
    .with_deadline(PORT_DEADLINE);
    if let Ok(channel) = ctx.data::<ChannelContext>() {
        context = context.with_channel(channel.slug.clone());
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
                "Marketplace seller service is temporarily unavailable",
            )
        }
        PortErrorKind::InvariantViolation => <FieldError as GraphQLError>::internal_error(
            "Marketplace seller command requires operator review",
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
