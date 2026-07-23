#[path = "transport/graphql_adapter.rs"]
mod graphql_adapter;
#[path = "transport/graphql_application_lifecycle_adapter.rs"]
mod graphql_application_lifecycle_adapter;
#[path = "transport/graphql_applications_adapter.rs"]
mod graphql_applications_adapter;
#[path = "transport/native_application_lifecycle_adapter.rs"]
mod native_application_lifecycle_adapter;
#[path = "transport/native_applications_adapter.rs"]
mod native_applications_adapter;
#[path = "transport/native_server_adapter.rs"]
mod native_server_adapter;

use rustok_ui_transport::{UiTransportPath, UiTransportResult, execute_selected_transport};

use crate::application_model::{
    CancelGroupMembershipApplicationCommand, GroupsStorefrontApplicationLifecycleResult,
    GroupsStorefrontApplicationPolicy, GroupsStorefrontApplicationPolicyQuery,
    GroupsStorefrontMembershipApplication, GroupsStorefrontMyApplicationQuery,
    GroupsStorefrontSubmitApplicationResult, SubmitGroupMembershipApplicationCommand,
};
use crate::core::GroupsStorefrontTransportProfile;
use crate::model::{
    AcceptGroupInvitationCommand, AcceptTargetedGroupInvitationCommand,
    GroupsStorefrontAcceptInvitationResult, GroupsStorefrontDirectory, GroupsStorefrontFilters,
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

pub async fn accept_groups_storefront_targeted_invitation(
    context: GroupsStorefrontTransportContext,
    command: AcceptTargetedGroupInvitationCommand,
) -> UiTransportResult<GroupsStorefrontAcceptInvitationResult> {
    let token = context.access_token.clone();
    let tenant = context.tenant_slug.clone();
    let native_command = command.clone();
    execute_selected_transport(
        "groups.storefront.targeted_invitation.accept",
        context.path(),
        move || native_server_adapter::accept_targeted_invitation(native_command),
        move || graphql_adapter::accept_targeted_invitation(token, tenant, command),
    )
    .await
}

pub async fn load_groups_storefront_application_policy(
    context: GroupsStorefrontTransportContext,
    query: GroupsStorefrontApplicationPolicyQuery,
) -> UiTransportResult<GroupsStorefrontApplicationPolicy> {
    let token = context.access_token.clone();
    let tenant = context.tenant_slug.clone();
    let native_query = query.clone();
    execute_selected_transport(
        "groups.storefront.applications.policy",
        context.path(),
        move || native_applications_adapter::load_group_application_policy(native_query),
        move || graphql_applications_adapter::load_group_application_policy(token, tenant, query),
    )
    .await
}

pub async fn load_groups_storefront_my_application(
    context: GroupsStorefrontTransportContext,
    query: GroupsStorefrontMyApplicationQuery,
) -> UiTransportResult<Option<GroupsStorefrontMembershipApplication>> {
    let token = context.access_token.clone();
    let tenant = context.tenant_slug.clone();
    let native_query = query.clone();
    execute_selected_transport(
        "groups.storefront.applications.my",
        context.path(),
        move || {
            native_application_lifecycle_adapter::load_my_group_membership_application(native_query)
        },
        move || {
            graphql_application_lifecycle_adapter::load_my_group_membership_application(
                token, tenant, query,
            )
        },
    )
    .await
}

pub async fn submit_groups_storefront_membership_application(
    context: GroupsStorefrontTransportContext,
    command: SubmitGroupMembershipApplicationCommand,
) -> UiTransportResult<GroupsStorefrontSubmitApplicationResult> {
    let token = context.access_token.clone();
    let tenant = context.tenant_slug.clone();
    let native_command = command.clone();
    execute_selected_transport(
        "groups.storefront.applications.submit_if_current",
        context.path(),
        move || native_applications_adapter::submit_group_membership_application(native_command),
        move || {
            graphql_applications_adapter::submit_group_membership_application(
                token, tenant, command,
            )
        },
    )
    .await
}

pub async fn cancel_groups_storefront_membership_application(
    context: GroupsStorefrontTransportContext,
    command: CancelGroupMembershipApplicationCommand,
) -> UiTransportResult<GroupsStorefrontApplicationLifecycleResult> {
    let token = context.access_token.clone();
    let tenant = context.tenant_slug.clone();
    let native_command = command.clone();
    execute_selected_transport(
        "groups.storefront.applications.cancel",
        context.path(),
        move || {
            native_application_lifecycle_adapter::cancel_group_membership_application(
                native_command,
            )
        },
        move || {
            graphql_application_lifecycle_adapter::cancel_group_membership_application(
                token, tenant, command,
            )
        },
    )
    .await
}

pub const GROUPS_STOREFRONT_TRANSPORT_FALLBACK_POLICY: &str = "never falls back";
