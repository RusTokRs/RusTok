#[path = "transport/graphql_adapter.rs"]
mod graphql_adapter;
#[path = "transport/native_server_adapter.rs"]
mod native_server_adapter;

use rustok_ui_transport::{UiTransportPath, UiTransportResult, execute_selected_transport};

use crate::core::BundleAdminTransportProfile;
use crate::model::{
    BundleAdminCommand, BundleAdminCommandResult, BundleAdminDirectory, BundleAdminFilters,
    BundleAdminRecord,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleAdminTransportContext {
    pub profile: BundleAdminTransportProfile,
    pub access_token: Option<String>,
    pub tenant_slug: Option<String>,
}

impl BundleAdminTransportContext {
    pub fn native() -> Self {
        Self {
            profile: BundleAdminTransportProfile::Native,
            access_token: None,
            tenant_slug: None,
        }
    }

    pub fn graphql(access_token: Option<String>, tenant_slug: Option<String>) -> Self {
        Self {
            profile: BundleAdminTransportProfile::Graphql,
            access_token,
            tenant_slug,
        }
    }

    fn path(&self) -> UiTransportPath {
        match self.profile {
            BundleAdminTransportProfile::Native => UiTransportPath::NativeServer,
            BundleAdminTransportProfile::Graphql => UiTransportPath::Graphql,
        }
    }
}

pub async fn load_bundle_directory(
    context: BundleAdminTransportContext,
    filters: BundleAdminFilters,
) -> UiTransportResult<BundleAdminDirectory> {
    let graphql_token = context.access_token.clone();
    let graphql_tenant = context.tenant_slug.clone();
    let native_filters = filters.clone();
    execute_selected_transport(
        "bundle.directory",
        context.path(),
        move || native_server_adapter::load_directory(native_filters),
        move || graphql_adapter::load_directory(graphql_token, graphql_tenant, filters),
    )
    .await
}

pub async fn load_bundle_detail(
    context: BundleAdminTransportContext,
    bundle_id: String,
) -> UiTransportResult<BundleAdminRecord> {
    let graphql_token = context.access_token.clone();
    let graphql_tenant = context.tenant_slug.clone();
    let native_id = bundle_id.clone();
    execute_selected_transport(
        "bundle.detail",
        context.path(),
        move || native_server_adapter::load_detail(native_id),
        move || graphql_adapter::load_detail(graphql_token, graphql_tenant, bundle_id),
    )
    .await
}

pub async fn execute_bundle_command(
    context: BundleAdminTransportContext,
    idempotency_key: String,
    command: BundleAdminCommand,
) -> UiTransportResult<BundleAdminCommandResult> {
    let graphql_token = context.access_token.clone();
    let graphql_tenant = context.tenant_slug.clone();
    let native_key = idempotency_key.clone();
    let native_command = command.clone();
    execute_selected_transport(
        "bundle.command",
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

pub const BUNDLE_TRANSPORT_FALLBACK_POLICY: &str = "never falls back";
