use leptos::prelude::*;
use std::fmt::{Display, Formatter};

use crate::model::{
    AcceptGroupInvitationCommand, GroupsStorefrontAcceptInvitationResult,
    GroupsStorefrontDirectory, GroupsStorefrontFilters, GroupsStorefrontMembership,
};

#[derive(Debug, Clone)]
pub struct NativeGroupsStorefrontError(pub String);

impl Display for NativeGroupsStorefrontError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl std::error::Error for NativeGroupsStorefrontError {}

impl From<ServerFnError> for NativeGroupsStorefrontError {
    fn from(value: ServerFnError) -> Self {
        Self(value.to_string())
    }
}

pub async fn load_directory(
    filters: GroupsStorefrontFilters,
) -> Result<GroupsStorefrontDirectory, NativeGroupsStorefrontError> {
    groups_storefront_directory_native(filters)
        .await
        .map_err(Into::into)
}

pub async fn accept_invitation(
    command: AcceptGroupInvitationCommand,
) -> Result<GroupsStorefrontAcceptInvitationResult, NativeGroupsStorefrontError> {
    groups_storefront_accept_invitation_native(command)
        .await
        .map_err(Into::into)
}

#[server(prefix = "/api/fn", endpoint = "groups/storefront/directory")]
async fn groups_storefront_directory_native(
    filters: GroupsStorefrontFilters,
) -> Result<GroupsStorefrontDirectory, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            request::RequestContext, HostRuntimeContext, PortActor, PortContext, TenantContext,
        };
        use rustok_groups::{GroupSummaryReadPort, GroupsService, ListGroupsRequest};
        use std::time::Duration;
        use uuid::Uuid;

        let runtime = expect_context::<HostRuntimeContext>();
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request = leptos_axum::extract::<RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        let context = PortContext::new(
            tenant.id.to_string(),
            PortActor::service("groups-public-native"),
            request.locale,
            format!("groups-storefront-native-{}", Uuid::new_v4()),
        )
        .with_deadline(Duration::from_secs(5));
        let response = GroupSummaryReadPort::list_groups(
            &GroupsService::new(runtime.db_clone()),
            context,
            ListGroupsRequest {
                page: filters.page.max(1),
                per_page: filters.per_page.clamp(1, 100),
                search: filters.search,
                include_non_public: false,
            },
        )
        .await
        .map_err(|error| ServerFnError::new(error.message))?;

        Ok(GroupsStorefrontDirectory {
            items: response
                .items
                .into_iter()
                .map(|group| crate::model::GroupsStorefrontListItem {
                    id: group.id.to_string(),
                    handle: group.handle,
                    title: group.title,
                    summary: group.summary,
                    visibility: group.visibility.as_str().to_string(),
                    join_policy: group.join_policy.as_str().to_string(),
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
            "groups storefront native transport requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "groups/storefront/invitations/accept"
)]
async fn groups_storefront_accept_invitation_native(
    command: AcceptGroupInvitationCommand,
) -> Result<GroupsStorefrontAcceptInvitationResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            request::RequestContext, AuthContext, HostRuntimeContext, PortActor, PortContext,
            TenantContext,
        };
        use rustok_groups::{
            AcceptGroupInvitationRequest, GroupInvitationCommandPort, GroupInvitationService,
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

        let mut context = PortContext::new(
            tenant.id.to_string(),
            PortActor::user(auth.user_id.to_string()),
            request.locale,
            format!("groups-storefront-invitations-native-{}", Uuid::new_v4()),
        )
        .with_deadline(Duration::from_secs(5))
        .with_idempotency_key(command.idempotency_key);
        for permission in auth.permissions {
            context = context.with_claim(permission.to_string());
        }

        let result = GroupInvitationCommandPort::accept_group_invitation(
            &GroupInvitationService::new(runtime.db_clone()),
            context,
            AcceptGroupInvitationRequest {
                token: command.token,
            },
        )
        .await
        .map_err(|error| ServerFnError::new(error.message))?;

        Ok(GroupsStorefrontAcceptInvitationResult {
            invitation_id: result.invitation_id.to_string(),
            group_id: result.group_id.to_string(),
            membership: map_membership(result.membership),
            group_version: result.group_version,
            replayed: result.replayed,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = command;
        Err(ServerFnError::new(
            "groups storefront invitation native transport requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn map_membership(value: rustok_groups::GroupMembership) -> GroupsStorefrontMembership {
    GroupsStorefrontMembership {
        id: value.id.to_string(),
        group_id: value.group_id.to_string(),
        user_id: value.user_id.to_string(),
        role: value.role.as_str().to_string(),
        status: value.status.as_str().to_string(),
    }
}
