#[path = "transport/graphql_adapter.rs"]
mod graphql_adapter;
#[path = "transport/native_server_adapter.rs"]
mod native_server_adapter;

use rustok_ui_transport::{execute_selected_transport, UiTransportPath, UiTransportResult};

use crate::core::GroupsStorefrontTransportProfile;
use crate::model::{GroupsStorefrontDirectory, GroupsStorefrontFilters};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupsStorefrontTransportContext {
    pub profile: GroupsStorefrontTransportProfile,
    pub tenant_slug: Option<String>,
}

impl GroupsStorefrontTransportContext {
    pub fn native() -> Self {
        Self {
            profile: GroupsStorefrontTransportProfile::Native,
            tenant_slug: None,
        }
    }

    pub fn graphql(tenant_slug: Option<String>) -> Self {
        Self {
            profile: GroupsStorefrontTransportProfile::Graphql,
            tenant_slug,
        }
    }

    fn path(&self) -> UiTransportPath {
        match self.profile {
            GroupsStorefrontTransportProfile::Native => UiTransportPath::NativeServer,
            GroupsStorefrontTransportProfile::Graphql => UiTransportPath::Graphql,
        }
    }
}

pub async fn load_groups_storefront_directory(
    context: GroupsStorefrontTransportContext,
    filters: GroupsStorefrontFilters,
) -> UiTransportResult<GroupsStorefrontDirectory> {
    let tenant = context.tenant_slug.clone();
    let native_filters = filters.clone();
    execute_selected_transport(
        "groups.storefront.directory",
        context.path(),
        move || native_server_adapter::load_directory(native_filters),
        move || graphql_adapter::load_directory(tenant, filters),
    )
    .await
}

pub const GROUPS_STOREFRONT_TRANSPORT_FALLBACK_POLICY: &str = "never falls back";
