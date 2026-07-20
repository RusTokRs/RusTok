use std::time::Duration;

use async_graphql::{Context, FieldError, Json, Object, Result, SimpleObject};
use rustok_api::graphql::GraphQLError;
use rustok_api::request::RequestContext;
use rustok_api::{
    has_any_effective_permission, AuthContext, ChannelContext, HostRuntimeContext, Permission,
    PortActor, PortContext, PortError, PortErrorKind, TenantContext,
};
use uuid::Uuid;

use crate::{
    ListMarketplaceSellerEventsRequest, MarketplaceSellerReadPort, MarketplaceSellerService,
};

const PORT_DEADLINE: Duration = Duration::from_secs(5);

#[derive(Default)]
pub struct MarketplaceSellerEventQuery;

#[Object]
impl MarketplaceSellerEventQuery {
    async fn marketplace_seller_events(
        &self,
        ctx: &Context<'_>,
        seller_id: Uuid,
        limit: i32,
    ) -> Result<Vec<MarketplaceSellerEventGql>> {
        let auth = require_permissions(ctx, &[Permission::MARKETPLACE_SELLERS_READ])?;
        let service = service(ctx)?;
        MarketplaceSellerReadPort::list_seller_events(
            &service,
            port_context(ctx, auth)?,
            ListMarketplaceSellerEventsRequest {
                seller_id,
                limit: limit.clamp(1, 200) as u64,
            },
        )
        .await
        .map(|events| events.into_iter().map(Into::into).collect())
        .map_err(map_port_error)
    }
}

#[derive(Clone, Debug, SimpleObject)]
pub struct MarketplaceSellerEventGql {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub seller_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub event_kind: String,
    pub locale: Option<String>,
    pub provenance: String,
    pub note: Option<String>,
    pub metadata: Json<serde_json::Value>,
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
}

impl From<crate::MarketplaceSellerEventResponse> for MarketplaceSellerEventGql {
    fn from(value: crate::MarketplaceSellerEventResponse) -> Self {
        Self {
            id: value.id,
            tenant_id: value.tenant_id,
            seller_id: value.seller_id,
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

fn service(ctx: &Context<'_>) -> Result<MarketplaceSellerService> {
    let runtime = ctx.data::<HostRuntimeContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error(
            "Marketplace seller runtime is not registered",
        )
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
            "marketplace seller read permission required",
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

fn port_context(ctx: &Context<'_>, auth: &AuthContext) -> Result<PortContext> {
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
        format!("graphql-marketplace-seller-events-{}", Uuid::new_v4()),
    )
    .with_deadline(PORT_DEADLINE);
    if let Ok(channel) = ctx.data::<ChannelContext>() {
        context = context.with_channel(channel.slug.clone());
    }
    Ok(context)
}

fn map_port_error(error: PortError) -> FieldError {
    match error.kind {
        PortErrorKind::Validation | PortErrorKind::Conflict => {
            <FieldError as GraphQLError>::bad_user_input(&error.message)
        }
        PortErrorKind::NotFound => <FieldError as GraphQLError>::not_found(&error.message),
        PortErrorKind::Forbidden => {
            <FieldError as GraphQLError>::permission_denied(&error.message)
        }
        PortErrorKind::Unavailable | PortErrorKind::Timeout => {
            <FieldError as GraphQLError>::internal_error(
                "Marketplace seller history is temporarily unavailable",
            )
        }
        PortErrorKind::InvariantViolation => <FieldError as GraphQLError>::internal_error(
            "Marketplace seller history requires operator review",
        ),
    }
}
