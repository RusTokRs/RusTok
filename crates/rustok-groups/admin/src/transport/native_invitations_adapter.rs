use leptos::prelude::*;
use std::fmt::{Display, Formatter};

use crate::model::{
    CreateGroupInvitationCommand, GroupsAdminCreateInvitationResult, GroupsAdminInvitation,
    GroupsAdminInvitationConnection, GroupsAdminInvitationQuery,
    GroupsAdminRevokeInvitationResult, RevokeGroupInvitationCommand,
};

#[derive(Debug, Clone)]
pub struct NativeGroupsInvitationError(pub String);

impl Display for NativeGroupsInvitationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl std::error::Error for NativeGroupsInvitationError {}

impl From<ServerFnError> for NativeGroupsInvitationError {
    fn from(value: ServerFnError) -> Self {
        Self(value.to_string())
    }
}

pub async fn load_group_invitations(
    query: GroupsAdminInvitationQuery,
) -> Result<GroupsAdminInvitationConnection, NativeGroupsInvitationError> {
    groups_admin_invitations_native(query).await.map_err(Into::into)
}

pub async fn create_group_invitation(
    command: CreateGroupInvitationCommand,
) -> Result<GroupsAdminCreateInvitationResult, NativeGroupsInvitationError> {
    groups_admin_create_invitation_native(command)
        .await
        .map_err(Into::into)
}

pub async fn revoke_group_invitation(
    command: RevokeGroupInvitationCommand,
) -> Result<GroupsAdminRevokeInvitationResult, NativeGroupsInvitationError> {
    groups_admin_revoke_invitation_native(command)
        .await
        .map_err(Into::into)
}

#[server(
    prefix = "/api/fn",
    endpoint = "groups/admin/invitations/list"
)]
async fn groups_admin_invitations_native(
    query: GroupsAdminInvitationQuery,
) -> Result<GroupsAdminInvitationConnection, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            request::RequestContext, AuthContext, HostRuntimeContext, PortActor, PortContext,
            TenantContext,
        };
        use rustok_groups::{
            GroupInvitationReadPort, GroupInvitationService, ListGroupInvitationsRequest,
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
        let group_id = Uuid::parse_str(&query.group_id)
            .map_err(|_| ServerFnError::new("group_id must be a UUID"))?;
        let mut context = PortContext::new(
            tenant.id.to_string(),
            PortActor::user(auth.user_id.to_string()),
            request.locale,
            format!("groups-admin-invitations-native-{}", Uuid::new_v4()),
        )
        .with_deadline(Duration::from_secs(5));
        for permission in auth.permissions {
            context = context.with_claim(permission.to_string());
        }
        let result = GroupInvitationReadPort::list_group_invitations(
            &GroupInvitationService::new(runtime.db_clone()),
            context,
            ListGroupInvitationsRequest {
                group_id,
                page: query.page,
                per_page: query.per_page,
                include_inactive: query.include_inactive,
            },
        )
        .await
        .map_err(|error| ServerFnError::new(error.message))?;
        Ok(GroupsAdminInvitationConnection {
            items: result.items.into_iter().map(map_invitation).collect(),
            total: result.total,
            page: result.page,
            per_page: result.per_page,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = query;
        Err(ServerFnError::new(
            "groups admin invitation native transport requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "groups/admin/invitations/create"
)]
async fn groups_admin_create_invitation_native(
    command: CreateGroupInvitationCommand,
) -> Result<GroupsAdminCreateInvitationResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            request::RequestContext, AuthContext, HostRuntimeContext, PortActor, PortContext,
            TenantContext,
        };
        use rustok_groups::{
            CreateGroupInvitationRequest, GroupInvitationCommandPort, GroupInvitationService,
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
        let target_user_id = command
            .target_user_id
            .as_deref()
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|_| ServerFnError::new("target_user_id must be a UUID"))?;
        let mut context = PortContext::new(
            tenant.id.to_string(),
            PortActor::user(auth.user_id.to_string()),
            request.locale,
            format!("groups-admin-invitations-native-{}", Uuid::new_v4()),
        )
        .with_deadline(Duration::from_secs(5))
        .with_idempotency_key(command.idempotency_key);
        for permission in auth.permissions {
            context = context.with_claim(permission.to_string());
        }
        let result = GroupInvitationCommandPort::create_group_invitation(
            &GroupInvitationService::new(runtime.db_clone()),
            context,
            CreateGroupInvitationRequest {
                group_id,
                target_user_id,
                expires_in_seconds: command.expires_in_seconds,
                max_uses: command.max_uses,
            },
        )
        .await
        .map_err(|error| ServerFnError::new(error.message))?;
        Ok(GroupsAdminCreateInvitationResult {
            invitation: map_invitation(result.invitation),
            token: result.token,
            group_version: result.group_version,
            replayed: result.replayed,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = command;
        Err(ServerFnError::new(
            "groups admin invitation native transport requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "groups/admin/invitations/revoke"
)]
async fn groups_admin_revoke_invitation_native(
    command: RevokeGroupInvitationCommand,
) -> Result<GroupsAdminRevokeInvitationResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            request::RequestContext, AuthContext, HostRuntimeContext, PortActor, PortContext,
            TenantContext,
        };
        use rustok_groups::{
            GroupInvitationCommandPort, GroupInvitationService, RevokeGroupInvitationRequest,
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
        let invitation_id = Uuid::parse_str(&command.invitation_id)
            .map_err(|_| ServerFnError::new("invitation_id must be a UUID"))?;
        let mut context = PortContext::new(
            tenant.id.to_string(),
            PortActor::user(auth.user_id.to_string()),
            request.locale,
            format!("groups-admin-invitations-native-{}", Uuid::new_v4()),
        )
        .with_deadline(Duration::from_secs(5))
        .with_idempotency_key(command.idempotency_key);
        for permission in auth.permissions {
            context = context.with_claim(permission.to_string());
        }
        let result = GroupInvitationCommandPort::revoke_group_invitation(
            &GroupInvitationService::new(runtime.db_clone()),
            context,
            RevokeGroupInvitationRequest { invitation_id },
        )
        .await
        .map_err(|error| ServerFnError::new(error.message))?;
        Ok(GroupsAdminRevokeInvitationResult {
            invitation: map_invitation(result.invitation),
            group_version: result.group_version,
            replayed: result.replayed,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = command;
        Err(ServerFnError::new(
            "groups admin invitation native transport requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn map_invitation(value: rustok_groups::GroupInvitation) -> GroupsAdminInvitation {
    GroupsAdminInvitation {
        id: value.id.to_string(),
        group_id: value.group_id.to_string(),
        invited_by_user_id: value.invited_by_user_id.to_string(),
        target_user_id: value.target_user_id.map(|id| id.to_string()),
        max_uses: value.max_uses,
        use_count: value.use_count,
        expires_at: value.expires_at.to_rfc3339(),
        revoked_at: value.revoked_at.map(|date| date.to_rfc3339()),
        revoked_by_user_id: value.revoked_by_user_id.map(|id| id.to_string()),
        created_at: value.created_at.to_rfc3339(),
        status: value.status.as_str().to_string(),
    }
}
