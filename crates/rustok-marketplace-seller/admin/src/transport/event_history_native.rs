use leptos::prelude::*;
use std::fmt::{Display, Formatter};

use crate::model::{
    MarketplaceSellerAdminEvent, MarketplaceSellerAdminEventHistory,
};

#[derive(Debug, Clone)]
pub struct NativeMarketplaceSellerEventHistoryError(pub String);

impl Display for NativeMarketplaceSellerEventHistoryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl std::error::Error for NativeMarketplaceSellerEventHistoryError {}

impl From<ServerFnError> for NativeMarketplaceSellerEventHistoryError {
    fn from(value: ServerFnError) -> Self {
        Self(value.to_string())
    }
}

pub async fn load_event_history(
    seller_id: String,
    limit: u64,
) -> Result<MarketplaceSellerAdminEventHistory, NativeMarketplaceSellerEventHistoryError> {
    marketplace_seller_event_history_native(seller_id, limit)
        .await
        .map_err(Into::into)
}

#[server(prefix = "/api/fn", endpoint = "marketplace-seller/event-history")]
async fn marketplace_seller_event_history_native(
    seller_id: String,
    limit: u64,
) -> Result<MarketplaceSellerAdminEventHistory, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            request::RequestContext, AuthContext, HostRuntimeContext, Permission, TenantContext,
        };
        use rustok_marketplace_seller::{
            ListMarketplaceSellerEventsRequest, MarketplaceSellerReadPort,
            MarketplaceSellerService,
        };

        let runtime = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request = leptos_axum::extract::<RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        ensure_permission(&auth, Permission::MARKETPLACE_SELLERS_READ)?;
        ensure_tenant(&auth, &tenant)?;
        let seller_id = parse_uuid(seller_id.as_str(), "seller_id")?;
        let service = MarketplaceSellerService::new(runtime.db_clone());
        let events = MarketplaceSellerReadPort::list_seller_events(
            &service,
            port_context(&auth, &tenant, &request),
            ListMarketplaceSellerEventsRequest {
                seller_id,
                limit: limit.clamp(1, 200),
            },
        )
        .await
        .map_err(map_port_error)?;

        Ok(MarketplaceSellerAdminEventHistory {
            seller_id: seller_id.to_string(),
            items: events
                .into_iter()
                .map(|event| MarketplaceSellerAdminEvent {
                    id: event.id.to_string(),
                    seller_id: event.seller_id.to_string(),
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
        let _ = (seller_id, limit);
        Err(ServerFnError::new(
            "marketplace seller event history requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn ensure_permission(
    auth: &rustok_api::AuthContext,
    permission: rustok_api::Permission,
) -> Result<(), ServerFnError> {
    if !rustok_api::has_any_effective_permission(&auth.permissions, &[permission]) {
        return Err(ServerFnError::new(
            "Permission denied: marketplace seller read permission required",
        ));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
fn ensure_tenant(
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
) -> Result<(), ServerFnError> {
    if auth.tenant_id != tenant.id {
        return Err(ServerFnError::new(
            "Permission denied: marketplace seller tenant mismatch",
        ));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
fn port_context(
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    request: &rustok_api::request::RequestContext,
) -> rustok_api::PortContext {
    let mut context = rustok_api::PortContext::new(
        tenant.id.to_string(),
        rustok_api::PortActor::user(auth.user_id.to_string()),
        request.locale.clone(),
        format!("native-marketplace-seller-events-{}", uuid::Uuid::new_v4()),
    )
    .with_deadline(std::time::Duration::from_secs(5));
    if let Some(channel) = request.channel_slug.clone() {
        context = context.with_channel(channel);
    }
    context
}

#[cfg(feature = "ssr")]
fn parse_uuid(value: &str, field: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value.trim())
        .map_err(|_| ServerFnError::new(format!("Invalid {field}")))
}

#[cfg(feature = "ssr")]
fn map_port_error(error: rustok_api::PortError) -> ServerFnError {
    use rustok_api::PortErrorKind;
    let message = match error.kind {
        PortErrorKind::Validation | PortErrorKind::NotFound | PortErrorKind::Conflict => {
            error.message
        }
        PortErrorKind::Forbidden => "Permission denied: marketplace seller history".to_string(),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => {
            "Marketplace seller history is temporarily unavailable".to_string()
        }
        PortErrorKind::InvariantViolation => {
            "Marketplace seller history requires operator review".to_string()
        }
    };
    ServerFnError::new(message)
}
