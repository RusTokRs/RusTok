#[path = "transport/graphql_adapter.rs"]
mod graphql_adapter;
#[path = "transport/native_server_adapter.rs"]
mod native_server_adapter;

use rustok_ui_transport::{execute_selected_transport, UiTransportPath, UiTransportResult};

use crate::core::GroupsAdminTransportProfile;
use crate::model::{GroupsAdminDirectory, GroupsAdminFilters};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupsAdminTransportContext {
    pub profile: GroupsAdminTransportProfile,
    pub access_token: Option<String>,
    pub tenant_slug: Option<String>,
}

impl GroupsAdminTransportContext {
    pub fn native() -> Self {
        Self {
            profile: GroupsAdminTransportProfile::Native,
            access_token: None,
            tenant_slug: None,
        }
    }

    pub fn graphql(access_token: Option<String>, tenant_slug: Option<String>) -> Self {
        Self {
            profile: GroupsAdminTransportProfile::Graphql,
            access_token,
            tenant_slug,
        }
    }

    fn path(&self) -> UiTransportPath {
        match self.profile {
            GroupsAdminTransportProfile::Native => UiTransportPath::NativeServer,
            GroupsAdminTransportProfile::Graphql => UiTransportPath::Graphql,
        }
    }
}

pub async fn load_groups_admin_directory(
    context: GroupsAdminTransportContext,
    filters: GroupsAdminFilters,
) -> UiTransportResult<GroupsAdminDirectory> {
    let token = context.access_token.clone();
    let tenant = context.tenant_slug.clone();
    let native_filters = filters.clone();
    execute_selected_transport(
        "groups.admin.directory",
        context.path(),
        move || native_server_adapter::load_directory(native_filters),
        move || graphql_adapter::load_directory(token, tenant, filters),
    )
    .await
}

pub const GROUPS_ADMIN_TRANSPORT_FALLBACK_POLICY: &str = "never falls back";
