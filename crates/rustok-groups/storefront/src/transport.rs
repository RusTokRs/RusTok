#[path = "transport/graphql_adapter.rs"]
mod graphql_adapter;
#[path = "transport/native_server_adapter.rs"]
mod native_server_adapter;

use rustok_ui_transport::{execute_selected_transport, UiTransportPath, UiTransportResult};

use crate::core::GroupsStorefrontTransportProfile;
use crate::model::{
    AcceptGroupInvitationCommand, GroupsStorefrontAcceptInvitationResult,
    GroupsStorefrontDirectory, GroupsStorefrontFilters,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupsStorefrontTransportContext {
    pub profile: GroupsStorefrontTransportProfile,
    pub access_token: Option<String>,
    pub tenant_slug: Option<String>,
}

impl GroupsStorefrontTransportContext {
    pub fn native() -> Self {
        Self {
            profile: GroupsStorefrontTransportProfile::Native,
            access_token: None,
            tenant_slug: None,
        }
    }

    pub fn graphql(tenant_slug: Option<String>) -> Self {
        Self::graphql_with_access_token(None, tenant_slug)
    }

    pub fn graphql_with_access_token(
        access_token: Option<String>,
        tenant_slug: Option<String>,
    ) -> Self {
        Self {
            profile: GroupsStorefrontTransportProfile::Graphql,
            access_token,
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

pub async fn accept_groups_storefront_invitation(
    context: GroupsStorefrontTransportContext,
    command: AcceptGroupInvitationCommand,
) -> UiTransportResult<GroupsStorefrontAcceptInvitationResult> {
    let token = context.access_token.clone();
    let tenant = context.tenant_slug.clone();
    let native_command = command.clone();
    execute_selected_transport(
        "groups.storefront.invitation.accept",
        context.path(),
        move || native_server_adapter::accept_invitation(native_command),
        move || graphql_adapter::accept_invitation(token, tenant, command),
    )
    .await
}

pub const GROUPS_STOREFRONT_TRANSPORT_FALLBACK_POLICY: &str = "never falls back";
