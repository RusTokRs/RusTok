use leptos::prelude::*;
use std::fmt::{Display, Formatter};

use crate::model::{
    ChangeGroupRoleCommand, GroupsAdminDirectory, GroupsAdminFilters,
    GroupsAdminGovernanceResult, TransferGroupOwnershipCommand,
};

#[derive(Debug, Clone)]
pub struct NativeGroupsAdminError(pub String);

impl Display for NativeGroupsAdminError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl std::error::Error for NativeGroupsAdminError {}

impl From<ServerFnError> for NativeGroupsAdminError {
    fn from(value: ServerFnError) -> Self {
        Self(value.to_string())
    }
}

pub async fn load_directory(
    filters: GroupsAdminFilters,
) -> Result<GroupsAdminDirectory, NativeGroupsAdminError> {
    groups_admin_directory_native(filters).await.map_err(Into::into)
}

pub async fn change_group_role(
    command: ChangeGroupRoleCommand,
) -> Result<GroupsAdminGovernanceResult, NativeGroupsAdminError> {
    groups_admin_change_role_native(command)
        .await
        .map_err(Into::into)
}

pub async fn transfer_group_ownership(
    command: TransferGroupOwnershipCommand,
) -> Result<GroupsAdminGovernanceResult, NativeGroupsAdminError> {
    groups_admin_transfer_ownership_native(command)
        .await
        .map_err(Into::into)
}

#[server(prefix = "/api/fn", endpoint = "groups/admin/directory")]
async fn groups_admin_directory_native(
    filters: GroupsAdminFilters,
) -> Result<GroupsAdminDirectory, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            request::RequestContext, AuthContext, HostRuntimeContext, Permission, PortActor,
            PortContext, TenantContext,
        };
        use rustok_groups::{GroupSummaryReadPort, GroupsService, ListGroupsRequest};
        use std::time::Duration;
        use uuid::Uuid;

        let runtime = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request = leptos_axum::extract::<RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        if auth.tenant_id != tenant.id {
            return Err(ServerFnError::new("groups tenant mismatch"));
        }
        if !auth.permissions.iter().any(|permission| {
            matches!(
                *permission,
                Permission::GROUPS_LIST | Permission::GROUPS_READ | Permission::GROUPS_MANAGE
            )
        }) {
            return Err(ServerFnError::new("groups permission required"));
        }

        let mut context = PortContext::new(
            tenant.id.to_string(),
            PortActor::user(auth.user_id.to_string()),
            request.locale,
            format!("groups-admin-native-{}", Uuid::new_v4()),
        )
        .with_deadline(Duration::from_secs(5));
        for permission in auth.permissions {
            context = context.with_claim(permission.to_string());
        }
        let page = filters.page.max(1);
        let per_page = filters.per_page.clamp(1, 100);
        let response = GroupSummaryReadPort::list_groups(
            &GroupsService::new(runtime.db_clone()),
            context,
            ListGroupsRequest {
                page,
                per_page,
                search: filters.search,
                include_non_public: filters.include_non_public,
            },
        )
        .await
        .map_err(|error| ServerFnError::new(error.message))?;

        Ok(GroupsAdminDirectory {
            items: response
                .items
                .into_iter()
                .map(|group| crate::model::GroupsAdminListItem {
                    id: group.id.to_string(),
                    handle: group.handle,
                    title: group.title,
                    visibility: group.visibility.as_str().to_string(),
                    join_policy: group.join_policy.as_str().to_string(),
                    status: group.status.as_str().to_string(),
                    member_count: group.member_count,
                    effective_locale: group.effective_locale,
                })
                .collect(),
            total: response.total,
            page: response.page,
            per_page: response.per_page,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = filters;
        Err(ServerFnError::new(
            "groups admin native transport requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "groups/admin/governance/change-role"
)]
async fn groups_admin_change_role_native(
    command: ChangeGroupRoleCommand,
) -> Result<GroupsAdminGovernanceResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            request::RequestContext, AuthContext, HostRuntimeContext, PortActor, PortContext,
            TenantContext,
        };
        use rustok_groups::{
            ChangeGroupRoleRequest, GroupGovernanceCommandPort, GroupGovernanceService, GroupRole,
        };
        use std::time::Duration;
        use uuid::Uuid;

        let runtime = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request = leptos_axum::extract::<RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        if auth.tenant_id != tenant.id {
            return Err(ServerFnError::new("groups tenant mismatch"));
        }
        let group_id = Uuid::parse_str(&command.group_id)
            .map_err(|_| ServerFnError::new("group_id must be a UUID"))?;
        let target_user_id = Uuid::parse_str(&command.target_user_id)
            .map_err(|_| ServerFnError::new("target_user_id must be a UUID"))?;
        let role = match command.role {
            crate::model::GroupsAdminAssignableRole::Admin => GroupRole::Admin,
            crate::model::GroupsAdminAssignableRole::Moderator => GroupRole::Moderator,
            crate::model::GroupsAdminAssignableRole::Member => GroupRole::Member,
        };

        let mut context = PortContext::new(
            tenant.id.to_string(),
            PortActor::user(auth.user_id.to_string()),
            request.locale,
            format!("groups-admin-governance-native-{}", Uuid::new_v4()),
        )
        .with_deadline(Duration::from_secs(5))
        .with_idempotency_key(command.idempotency_key);
        for permission in auth.permissions {
            context = context.with_claim(permission.to_string());
        }
        let result = GroupGovernanceCommandPort::change_group_role(
            &GroupGovernanceService::new(runtime.db_clone()),
            context,
            ChangeGroupRoleRequest {
                group_id,
                target_user_id,
                role,
            },
        )
        .await
        .map_err(|error| ServerFnError::new(error.message))?;

        Ok(GroupsAdminGovernanceResult {
            group_id: result.group_id.to_string(),
            actor_user_id: result.actor_user_id.to_string(),
            target_user_id: result.target_user_id.to_string(),
            previous_role: result.previous_role.as_str().to_string(),
            current_role: result.current_role.as_str().to_string(),
            group_version: result.group_version,
            replayed: result.replayed,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = command;
        Err(ServerFnError::new(
            "groups admin governance native transport requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "groups/admin/governance/transfer-ownership"
)]
async fn groups_admin_transfer_ownership_native(
    command: TransferGroupOwnershipCommand,
) -> Result<GroupsAdminGovernanceResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            request::RequestContext, AuthContext, HostRuntimeContext, PortActor, PortContext,
            TenantContext,
        };
        use rustok_groups::{
            GroupGovernanceCommandPort, GroupGovernanceService, TransferGroupOwnershipRequest,
        };
        use std::time::Duration;
        use uuid::Uuid;

        let runtime = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request = leptos_axum::extract::<RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        if auth.tenant_id != tenant.id {
            return Err(ServerFnError::new("groups tenant mismatch"));
        }
        let group_id = Uuid::parse_str(&command.group_id)
            .map_err(|_| ServerFnError::new("group_id must be a UUID"))?;
        let new_owner_user_id = Uuid::parse_str(&command.new_owner_user_id)
            .map_err(|_| ServerFnError::new("new_owner_user_id must be a UUID"))?;

        let mut context = PortContext::new(
            tenant.id.to_string(),
            PortActor::user(auth.user_id.to_string()),
            request.locale,
            format!("groups-admin-governance-native-{}", Uuid::new_v4()),
        )
        .with_deadline(Duration::from_secs(5))
        .with_idempotency_key(command.idempotency_key);
        for permission in auth.permissions {
            context = context.with_claim(permission.to_string());
        }
        let result = GroupGovernanceCommandPort::transfer_group_ownership(
            &GroupGovernanceService::new(runtime.db_clone()),
            context,
            TransferGroupOwnershipRequest {
                group_id,
                new_owner_user_id,
            },
        )
        .await
        .map_err(|error| ServerFnError::new(error.message))?;

        Ok(GroupsAdminGovernanceResult {
            group_id: result.group_id.to_string(),
            actor_user_id: result.actor_user_id.to_string(),
            target_user_id: result.target_user_id.to_string(),
            previous_role: result.previous_role.as_str().to_string(),
            current_role: result.current_role.as_str().to_string(),
            group_version: result.group_version,
            replayed: result.replayed,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = command;
        Err(ServerFnError::new(
            "groups admin governance native transport requires the `ssr` feature",
        ))
    }
}
