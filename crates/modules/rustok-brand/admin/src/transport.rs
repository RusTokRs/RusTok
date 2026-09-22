#[path = "transport/graphql_adapter.rs"]
mod graphql_adapter;
#[path = "transport/native_server_adapter.rs"]
mod native_server_adapter;

use rustok_ui_transport::{UiTransportPath, UiTransportResult, execute_selected_transport};

use crate::core::BrandAdminTransportProfile;
use crate::model::{
    BrandAdminCommand, BrandAdminCommandResult, BrandAdminDirectory, BrandAdminFilters,
    BrandAdminRecord,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrandAdminTransportContext {
    pub profile: BrandAdminTransportProfile,
    pub access_token: Option<String>,
    pub tenant_slug: Option<String>,
}

impl BrandAdminTransportContext {
    pub fn native() -> Self {
        Self {
            profile: BrandAdminTransportProfile::Native,
            access_token: None,
            tenant_slug: None,
        }
    }

    pub fn graphql(access_token: Option<String>, tenant_slug: Option<String>) -> Self {
        Self {
            profile: BrandAdminTransportProfile::Graphql,
            access_token,
            tenant_slug,
        }
    }

    fn path(&self) -> UiTransportPath {
        match self.profile {
            BrandAdminTransportProfile::Native => UiTransportPath::NativeServer,
            BrandAdminTransportProfile::Graphql => UiTransportPath::Graphql,
        }
    }
}

pub async fn load_brand_directory(
    context: BrandAdminTransportContext,
    filters: BrandAdminFilters,
) -> UiTransportResult<BrandAdminDirectory> {
    let graphql_token = context.access_token.clone();
    let graphql_tenant = context.tenant_slug.clone();
    let native_filters = filters.clone();
    execute_selected_transport(
        "brand.directory",
        context.path(),
        move || native_server_adapter::load_directory(native_filters),
        move || graphql_adapter::load_directory(graphql_token, graphql_tenant, filters),
    )
    .await
}

pub async fn load_brand_detail(
    context: BrandAdminTransportContext,
    brand_id: String,
) -> UiTransportResult<BrandAdminRecord> {
    let graphql_token = context.access_token.clone();
    let graphql_tenant = context.tenant_slug.clone();
    let native_id = brand_id.clone();
    execute_selected_transport(
        "brand.detail",
        context.path(),
        move || native_server_adapter::load_detail(native_id),
        move || graphql_adapter::load_detail(graphql_token, graphql_tenant, brand_id),
    )
    .await
}

pub async fn execute_brand_command(
    context: BrandAdminTransportContext,
    idempotency_key: String,
    command: BrandAdminCommand,
) -> UiTransportResult<BrandAdminCommandResult> {
    let graphql_token = context.access_token.clone();
    let graphql_tenant = context.tenant_slug.clone();
    let native_key = idempotency_key.clone();
    let native_command = command.clone();
    execute_selected_transport(
        "brand.command",
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

pub const BRAND_TRANSPORT_FALLBACK_POLICY: &str = "never falls back";
