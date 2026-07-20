#[path = "transport/event_history_graphql.rs"]
mod event_history_graphql;
#[path = "transport/event_history_native.rs"]
mod event_history_native;
#[path = "transport/graphql_adapter.rs"]
mod graphql_adapter;
#[path = "transport/native_server_adapter.rs"]
mod native_server_adapter;

use rustok_ui_transport::{execute_selected_transport, UiTransportPath, UiTransportResult};

use crate::core::MarketplaceSellerAdminTransportProfile;
use crate::model::{
    MarketplaceSellerAdminCommand, MarketplaceSellerAdminCommandResult,
    MarketplaceSellerAdminDetail, MarketplaceSellerAdminDirectory,
    MarketplaceSellerAdminEventHistory, MarketplaceSellerAdminFilters,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarketplaceSellerAdminTransportContext {
    pub profile: MarketplaceSellerAdminTransportProfile,
    pub access_token: Option<String>,
    pub tenant_slug: Option<String>,
}

impl MarketplaceSellerAdminTransportContext {
    pub fn native() -> Self {
        Self {
            profile: MarketplaceSellerAdminTransportProfile::Native,
            access_token: None,
            tenant_slug: None,
        }
    }

    pub fn graphql(access_token: Option<String>, tenant_slug: Option<String>) -> Self {
        Self {
            profile: MarketplaceSellerAdminTransportProfile::Graphql,
            access_token,
            tenant_slug,
        }
    }

    fn path(&self) -> UiTransportPath {
        match self.profile {
            MarketplaceSellerAdminTransportProfile::Native => UiTransportPath::NativeServer,
            MarketplaceSellerAdminTransportProfile::Graphql => UiTransportPath::Graphql,
        }
    }
}

pub async fn load_marketplace_seller_directory(
    context: MarketplaceSellerAdminTransportContext,
    filters: MarketplaceSellerAdminFilters,
) -> UiTransportResult<MarketplaceSellerAdminDirectory> {
    let graphql_token = context.access_token.clone();
    let graphql_tenant = context.tenant_slug.clone();
    let native_filters = filters.clone();
    execute_selected_transport(
        "marketplace_seller.directory",
        context.path(),
        move || native_server_adapter::load_directory(native_filters),
        move || graphql_adapter::load_directory(graphql_token, graphql_tenant, filters),
    )
    .await
}

pub async fn load_marketplace_seller_detail(
    context: MarketplaceSellerAdminTransportContext,
    seller_id: String,
) -> UiTransportResult<MarketplaceSellerAdminDetail> {
    let graphql_token = context.access_token.clone();
    let graphql_tenant = context.tenant_slug.clone();
    let native_id = seller_id.clone();
    execute_selected_transport(
        "marketplace_seller.detail",
        context.path(),
        move || native_server_adapter::load_detail(native_id),
        move || graphql_adapter::load_detail(graphql_token, graphql_tenant, seller_id),
    )
    .await
}

pub async fn load_marketplace_seller_event_history(
    context: MarketplaceSellerAdminTransportContext,
    seller_id: String,
    limit: u64,
) -> UiTransportResult<MarketplaceSellerAdminEventHistory> {
    let graphql_token = context.access_token.clone();
    let graphql_tenant = context.tenant_slug.clone();
    let native_id = seller_id.clone();
    execute_selected_transport(
        "marketplace_seller.event_history",
        context.path(),
        move || event_history_native::load_event_history(native_id, limit),
        move || {
            event_history_graphql::load_event_history(
                graphql_token,
                graphql_tenant,
                seller_id,
                limit,
            )
        },
    )
    .await
}

pub async fn execute_marketplace_seller_command(
    context: MarketplaceSellerAdminTransportContext,
    idempotency_key: String,
    command: MarketplaceSellerAdminCommand,
) -> UiTransportResult<MarketplaceSellerAdminCommandResult> {
    let graphql_token = context.access_token.clone();
    let graphql_tenant = context.tenant_slug.clone();
    let native_key = idempotency_key.clone();
    let native_command = command.clone();
    execute_selected_transport(
        "marketplace_seller.command",
        context.path(),
        move || native_server_adapter::execute_command(native_key, native_command),
        move || {
            graphql_adapter::execute_command(
                graphql_token,
                graphql_tenant,
                idempotency_key,
                command,
            )
        },
    )
    .await
}

/// Marketplace seller UI never falls back to another transport implicitly.
/// The host-selected profile is the only path executed by the shared transport
/// runner; errors preserve the failed path for operator-visible diagnostics.
pub const MARKETPLACE_SELLER_TRANSPORT_FALLBACK_POLICY: &str = "never falls back";
