#[path = "transport/graphql_adapter.rs"]
mod graphql_adapter;
#[path = "transport/native_server_adapter.rs"]
mod native_server_adapter;

use rustok_ui_transport::{UiTransportPath, UiTransportResult, execute_selected_transport};

use crate::core::ProductRelationsTransportProfile;
use crate::model::{
    ProductRelationItem, ProductRelationsAdminCommand, ProductRelationsAdminCommandResult,
    ProductRelationsAdminFilters,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductRelationsTransportContext {
    pub profile: ProductRelationsTransportProfile,
    pub access_token: Option<String>,
    pub tenant_slug: Option<String>,
}

impl ProductRelationsTransportContext {
    pub fn native() -> Self {
        Self {
            profile: ProductRelationsTransportProfile::Native,
            access_token: None,
            tenant_slug: None,
        }
    }

    pub fn graphql(access_token: Option<String>, tenant_slug: Option<String>) -> Self {
        Self {
            profile: ProductRelationsTransportProfile::Graphql,
            access_token,
            tenant_slug,
        }
    }

    fn path(&self) -> UiTransportPath {
        match self.profile {
            ProductRelationsTransportProfile::Native => UiTransportPath::NativeServer,
            ProductRelationsTransportProfile::Graphql => UiTransportPath::Graphql,
        }
    }
}

pub async fn load_product_relations(
    context: ProductRelationsTransportContext,
    filters: ProductRelationsAdminFilters,
) -> UiTransportResult<Vec<ProductRelationItem>> {
    let graphql_token = context.access_token.clone();
    let graphql_tenant = context.tenant_slug.clone();
    let native_filters = filters.clone();
    execute_selected_transport(
        "product_relations.load",
        context.path(),
        move || native_server_adapter::load_relations(native_filters),
        move || graphql_adapter::load_relations(graphql_token, graphql_tenant, filters),
    )
    .await
}

pub async fn execute_product_relations_command(
    context: ProductRelationsTransportContext,
    idempotency_key: String,
    command: ProductRelationsAdminCommand,
) -> UiTransportResult<ProductRelationsAdminCommandResult> {
    let graphql_token = context.access_token.clone();
    let graphql_tenant = context.tenant_slug.clone();
    let native_key = idempotency_key.clone();
    let native_command = command.clone();
    execute_selected_transport(
        "product_relations.command",
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

pub const PRODUCT_RELATIONS_FALLBACK_POLICY: &str = "never falls back";
