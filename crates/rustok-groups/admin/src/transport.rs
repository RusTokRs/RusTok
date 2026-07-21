#[path = "transport/graphql_adapter.rs"]
mod graphql_adapter;
#[path = "transport/native_server_adapter.rs"]
mod native_server_adapter;

use rustok_ui_transport::{execute_selected_transport, UiTransportPath, UiTransportResult};

use crate::core::GroupsAdminTransportProfile;
use crate::model::{
    ChangeGroupRoleCommand, DeleteGroupTranslationCommand, GroupsAdminDeleteTranslationResult,
    GroupsAdminDirectory, GroupsAdminFilters, GroupsAdminGovernanceResult, GroupsAdminTranslation,
    GroupsAdminTranslationMutationResult, GroupsAdminTranslationQuery,
    TransferGroupOwnershipCommand, UpsertGroupTranslationCommand,
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
        move || native_server_adapter::load_group_translations(native_query),
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
        move || native_server_adapter::upsert_group_translation(native_command),
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
        move || native_server_adapter::delete_group_translation(native_command),
        move || graphql_adapter::delete_group_translation(token, tenant, command),
    )
    .await
}

pub const GROUPS_ADMIN_TRANSPORT_FALLBACK_POLICY: &str = "never falls back";
