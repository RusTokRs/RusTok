#[path = "transport/graphql_adapter.rs"]
mod graphql_adapter;
#[path = "transport/native_server_adapter.rs"]
mod native_server_adapter;

use rustok_ui_transport::{UiTransportPath, UiTransportResult, execute_selected_transport};

use crate::core::MarketplaceListingAdminTransportProfile;
use crate::model::{
    MarketplaceListingAdminCommand, MarketplaceListingAdminCommandResult,
    MarketplaceListingAdminDetail, MarketplaceListingAdminDirectory,
    MarketplaceListingAdminFilters,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarketplaceListingAdminTransportContext {
    pub profile: MarketplaceListingAdminTransportProfile,
    pub access_token: Option<String>,
    pub tenant_slug: Option<String>,
    pub locale: Option<String>,
}

impl MarketplaceListingAdminTransportContext {
    pub fn native() -> Self {
        Self {
            profile: MarketplaceListingAdminTransportProfile::Native,
            access_token: None,
            tenant_slug: None,
            locale: None,
        }
    }

    pub fn graphql(
        access_token: Option<String>,
        tenant_slug: Option<String>,
        locale: Option<String>,
    ) -> Self {
        Self {
            profile: MarketplaceListingAdminTransportProfile::Graphql,
            access_token,
            tenant_slug,
            locale,
        }
    }

    fn path(&self) -> UiTransportPath {
        match self.profile {
            MarketplaceListingAdminTransportProfile::Native => UiTransportPath::NativeServer,
            MarketplaceListingAdminTransportProfile::Graphql => UiTransportPath::Graphql,
        }
    }
}

pub async fn load_marketplace_listing_directory(
    context: MarketplaceListingAdminTransportContext,
    filters: MarketplaceListingAdminFilters,
) -> UiTransportResult<MarketplaceListingAdminDirectory> {
    let graphql_token = context.access_token.clone();
    let graphql_tenant = context.tenant_slug.clone();
    let graphql_locale = context.locale.clone();
    let native_filters = filters.clone();
    execute_selected_transport(
        "marketplace_listing.directory",
        context.path(),
        move || native_server_adapter::load_directory(native_filters),
        move || {
            graphql_adapter::load_directory(graphql_token, graphql_tenant, graphql_locale, filters)
        },
    )
    .await
}

pub async fn load_marketplace_listing_detail(
    context: MarketplaceListingAdminTransportContext,
    listing_id: String,
) -> UiTransportResult<MarketplaceListingAdminDetail> {
    let graphql_token = context.access_token.clone();
    let graphql_tenant = context.tenant_slug.clone();
    let graphql_locale = context.locale.clone();
    let native_id = listing_id.clone();
    execute_selected_transport(
        "marketplace_listing.detail",
        context.path(),
        move || native_server_adapter::load_detail(native_id),
        move || {
            graphql_adapter::load_detail(graphql_token, graphql_tenant, graphql_locale, listing_id)
        },
    )
    .await
}

pub async fn execute_marketplace_listing_command(
    context: MarketplaceListingAdminTransportContext,
    idempotency_key: String,
    command: MarketplaceListingAdminCommand,
) -> UiTransportResult<MarketplaceListingAdminCommandResult> {
    let graphql_token = context.access_token.clone();
    let graphql_tenant = context.tenant_slug.clone();
    let graphql_locale = context.locale.clone();
    let native_key = idempotency_key.clone();
    let native_command = command.clone();
    execute_selected_transport(
        "marketplace_listing.command",
        context.path(),
        move || native_server_adapter::execute_command(native_key, native_command),
        move || {
            graphql_adapter::execute_command(
                graphql_token,
                graphql_tenant,
                graphql_locale,
                idempotency_key,
                command,
            )
        },
    )
    .await
}

/// Marketplace listing UI never falls back to another transport implicitly.
pub const MARKETPLACE_LISTING_TRANSPORT_FALLBACK_POLICY: &str = "never falls back";
