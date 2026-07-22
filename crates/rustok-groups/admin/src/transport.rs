#[path = "transport/graphql_adapter.rs"]
mod graphql_adapter;
#[path = "transport/graphql_application_lifecycle_adapter.rs"]
mod graphql_application_lifecycle_adapter;
#[path = "transport/graphql_applications_adapter.rs"]
mod graphql_applications_adapter;
#[path = "transport/graphql_invitations_adapter.rs"]
mod graphql_invitations_adapter;
#[path = "transport/graphql_policy_history_adapter.rs"]
mod graphql_policy_history_adapter;
#[path = "transport/graphql_policy_locale_adapter.rs"]
mod graphql_policy_locale_adapter;
#[path = "transport/native_application_lifecycle_adapter.rs"]
mod native_application_lifecycle_adapter;
#[path = "transport/native_applications_adapter.rs"]
mod native_applications_adapter;
#[path = "transport/native_invitations_adapter.rs"]
mod native_invitations_adapter;
#[path = "transport/native_localization_adapter.rs"]
mod native_localization_adapter;
#[path = "transport/native_policy_history_adapter.rs"]
mod native_policy_history_adapter;
#[path = "transport/native_policy_locale_adapter.rs"]
mod native_policy_locale_adapter;
#[path = "transport/native_server_adapter.rs"]
mod native_server_adapter;

use rustok_ui_transport::{execute_selected_transport, UiTransportPath, UiTransportResult};

use crate::application_model::{
    GroupsAdminApplicationPolicyLocaleCatalog, GroupsAdminApplicationPolicyLocaleCatalogQuery,
    GroupsAdminApplicationPolicyManagementView, GroupsAdminApplicationPolicyQuery,
    GroupsAdminApplicationPolicyRevisionConnection, GroupsAdminApplicationPolicyRevisionQuery,
    GroupsAdminMembershipApplicationConnection, GroupsAdminMembershipApplicationQuery,
    GroupsAdminReviewApplicationResult, GroupsAdminUpsertApplicationPolicyResult,
    ReopenGroupMembershipApplicationCommand, ReviewGroupMembershipApplicationCommand,
    UpsertGroupApplicationPolicyCommand,
};
use crate::core::GroupsAdminTransportProfile;
use crate::model::{
    ChangeGroupRoleCommand, CreateGroupInvitationCommand, DeleteGroupTranslationCommand,
    GroupsAdminCreateInvitationResult, GroupsAdminDeleteTranslationResult, GroupsAdminDirectory,
    GroupsAdminFilters, GroupsAdminGovernanceResult, GroupsAdminInvitationConnection,
    GroupsAdminInvitationQuery, GroupsAdminRevokeInvitationResult, GroupsAdminTranslation,
    GroupsAdminTranslationMutationResult, GroupsAdminTranslationQuery,
    RevokeGroupInvitationCommand, TransferGroupOwnershipCommand, UpsertGroupTranslationCommand,
};

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

pub async fn change_group_admin_role(
    context: GroupsAdminTransportContext,
    command: ChangeGroupRoleCommand,
) -> UiTransportResult<GroupsAdminGovernanceResult> {
    let token = context.access_token.clone();
    let tenant = context.tenant_slug.clone();
    let native_command = command.clone();
    execute_selected_transport(
        "groups.admin.governance.change_role",
        context.path(),
        move || native_server_adapter::change_group_role(native_command),
        move || graphql_adapter::change_group_role(token, tenant, command),
    )
    .await
}

pub async fn transfer_group_admin_ownership(
    context: GroupsAdminTransportContext,
    command: TransferGroupOwnershipCommand,
) -> UiTransportResult<GroupsAdminGovernanceResult> {
    let token = context.access_token.clone();
    let tenant = context.tenant_slug.clone();
    let native_command = command.clone();
    execute_selected_transport(
        "groups.admin.governance.transfer_ownership",
        context.path(),
        move || native_server_adapter::transfer_group_ownership(native_command),
        move || graphql_adapter::transfer_group_ownership(token, tenant, command),
    )
    .await
}

pub async fn load_group_admin_translations(
    context: GroupsAdminTransportContext,
    query: GroupsAdminTranslationQuery,
) -> UiTransportResult<Vec<GroupsAdminTranslation>> {
    let token = context.access_token.clone();
    let tenant = context.tenant_slug.clone();
    let native_query = query.clone();
    execute_selected_transport(
        "groups.admin.localization.list",
        context.path(),
        move || native_localization_adapter::load_group_translations(native_query),
        move || graphql_adapter::load_group_translations(token, tenant, query),
    )
    .await
}

pub async fn upsert_group_admin_translation(
    context: GroupsAdminTransportContext,
    command: UpsertGroupTranslationCommand,
) -> UiTransportResult<GroupsAdminTranslationMutationResult> {
    let token = context.access_token.clone();
    let tenant = context.tenant_slug.clone();
    let native_command = command.clone();
    execute_selected_transport(
        "groups.admin.localization.upsert",
        context.path(),
        move || native_localization_adapter::upsert_group_translation(native_command),
        move || graphql_adapter::upsert_group_translation(token, tenant, command),
    )
    .await
}

pub async fn delete_group_admin_translation(
    context: GroupsAdminTransportContext,
    command: DeleteGroupTranslationCommand,
) -> UiTransportResult<GroupsAdminDeleteTranslationResult> {
    let token = context.access_token.clone();
    let tenant = context.tenant_slug.clone();
    let native_command = command.clone();
    execute_selected_transport(
        "groups.admin.localization.delete",
        context.path(),
        move || native_localization_adapter::delete_group_translation(native_command),
        move || graphql_adapter::delete_group_translation(token, tenant, command),
    )
    .await
}

pub async fn load_group_admin_invitations(
    context: GroupsAdminTransportContext,
    query: GroupsAdminInvitationQuery,
) -> UiTransportResult<GroupsAdminInvitationConnection> {
    let token = context.access_token.clone();
    let tenant = context.tenant_slug.clone();
    let native_query = query.clone();
    execute_selected_transport(
        "groups.admin.invitations.list",
        context.path(),
        move || native_invitations_adapter::load_group_invitations(native_query),
        move || graphql_invitations_adapter::load_group_invitations(token, tenant, query),
    )
    .await
}

pub async fn create_group_admin_invitation(
    context: GroupsAdminTransportContext,
    command: CreateGroupInvitationCommand,
) -> UiTransportResult<GroupsAdminCreateInvitationResult> {
    let token = context.access_token.clone();
    let tenant = context.tenant_slug.clone();
    let native_command = command.clone();
    execute_selected_transport(
        "groups.admin.invitations.create",
        context.path(),
        move || native_invitations_adapter::create_group_invitation(native_command),
        move || graphql_invitations_adapter::create_group_invitation(token, tenant, command),
    )
    .await
}

pub async fn revoke_group_admin_invitation(
    context: GroupsAdminTransportContext,
    command: RevokeGroupInvitationCommand,
) -> UiTransportResult<GroupsAdminRevokeInvitationResult> {
    let token = context.access_token.clone();
    let tenant = context.tenant_slug.clone();
    let native_command = command.clone();
    execute_selected_transport(
        "groups.admin.invitations.revoke",
        context.path(),
        move || native_invitations_adapter::revoke_group_invitation(native_command),
        move || graphql_invitations_adapter::revoke_group_invitation(token, tenant, command),
    )
    .await
}

pub async fn load_group_admin_application_policy_locale_catalog(
    context: GroupsAdminTransportContext,
    query: GroupsAdminApplicationPolicyLocaleCatalogQuery,
) -> UiTransportResult<GroupsAdminApplicationPolicyLocaleCatalog> {
    let token = context.access_token.clone();
    let tenant = context.tenant_slug.clone();
    let native_query = query.clone();
    execute_selected_transport(
        "groups.admin.applications.policy.locales",
        context.path(),
        move || {
            native_policy_locale_adapter::load_group_application_policy_locale_catalog(
                native_query,
            )
        },
        move || {
            graphql_policy_locale_adapter::load_group_application_policy_locale_catalog(
                token, tenant, query,
            )
        },
    )
    .await
}

pub async fn load_group_admin_application_policy_for_management(
    context: GroupsAdminTransportContext,
    query: GroupsAdminApplicationPolicyQuery,
) -> UiTransportResult<GroupsAdminApplicationPolicyManagementView> {
    let token = context.access_token.clone();
    let tenant = context.tenant_slug.clone();
    let native_query = query.clone();
    execute_selected_transport(
        "groups.admin.applications.policy.management_read",
        context.path(),
        move || {
            native_policy_locale_adapter::load_group_application_policy_for_management(
                native_query,
            )
        },
        move || {
            graphql_policy_locale_adapter::load_group_application_policy_for_management(
                token, tenant, query,
            )
        },
    )
    .await
}

pub async fn upsert_group_admin_application_policy(
    context: GroupsAdminTransportContext,
    command: UpsertGroupApplicationPolicyCommand,
) -> UiTransportResult<GroupsAdminUpsertApplicationPolicyResult> {
    let token = context.access_token.clone();
    let tenant = context.tenant_slug.clone();
    let native_command = command.clone();
    execute_selected_transport(
        "groups.admin.applications.policy.upsert_if_current",
        context.path(),
        move || native_policy_locale_adapter::upsert_group_application_policy(native_command),
        move || {
            graphql_policy_locale_adapter::upsert_group_application_policy(token, tenant, command)
        },
    )
    .await
}

pub async fn load_group_admin_application_policy_revisions(
    context: GroupsAdminTransportContext,
    query: GroupsAdminApplicationPolicyRevisionQuery,
) -> UiTransportResult<GroupsAdminApplicationPolicyRevisionConnection> {
    let token = context.access_token.clone();
    let tenant = context.tenant_slug.clone();
    let native_query = query.clone();
    execute_selected_transport(
        "groups.admin.applications.policy.history",
        context.path(),
        move || {
            native_policy_history_adapter::load_group_application_policy_revisions(native_query)
        },
        move || {
            graphql_policy_history_adapter::load_group_application_policy_revisions(
                token, tenant, query,
            )
        },
    )
    .await
}

pub async fn load_group_admin_membership_applications(
    context: GroupsAdminTransportContext,
    query: GroupsAdminMembershipApplicationQuery,
) -> UiTransportResult<GroupsAdminMembershipApplicationConnection> {
    let token = context.access_token.clone();
    let tenant = context.tenant_slug.clone();
    let native_query = query.clone();
    execute_selected_transport(
        "groups.admin.applications.list",
        context.path(),
        move || native_applications_adapter::load_group_membership_applications(native_query),
        move || {
            graphql_applications_adapter::load_group_membership_applications(token, tenant, query)
        },
    )
    .await
}

pub async fn review_group_admin_membership_application(
    context: GroupsAdminTransportContext,
    command: ReviewGroupMembershipApplicationCommand,
) -> UiTransportResult<GroupsAdminReviewApplicationResult> {
    let token = context.access_token.clone();
    let tenant = context.tenant_slug.clone();
    let native_command = command.clone();
    execute_selected_transport(
        "groups.admin.applications.review",
        context.path(),
        move || native_applications_adapter::review_group_membership_application(native_command),
        move || {
            graphql_applications_adapter::review_group_membership_application(
                token, tenant, command,
            )
        },
    )
    .await
}

pub async fn reopen_group_admin_membership_application(
    context: GroupsAdminTransportContext,
    command: ReopenGroupMembershipApplicationCommand,
) -> UiTransportResult<GroupsAdminReviewApplicationResult> {
    let token = context.access_token.clone();
    let tenant = context.tenant_slug.clone();
    let native_command = command.clone();
    execute_selected_transport(
        "groups.admin.applications.reopen",
        context.path(),
        move || {
            native_application_lifecycle_adapter::reopen_group_membership_application(
                native_command,
            )
        },
        move || {
            graphql_application_lifecycle_adapter::reopen_group_membership_application(
                token, tenant, command,
            )
        },
    )
    .await
}

pub const GROUPS_ADMIN_TRANSPORT_FALLBACK_POLICY: &str = "never falls back";
