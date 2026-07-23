use leptos::prelude::*;
use std::fmt::{Display, Formatter};

use crate::application_model::{
    CancelGroupMembershipApplicationCommand, GroupsStorefrontApplicationLifecycleResult,
    GroupsStorefrontApplicationMembership, GroupsStorefrontMembershipApplication,
    GroupsStorefrontMyApplicationQuery,
};

#[derive(Debug, Clone)]
pub struct NativeGroupsApplicationLifecycleError(pub String);

impl Display for NativeGroupsApplicationLifecycleError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl std::error::Error for NativeGroupsApplicationLifecycleError {}

impl From<ServerFnError> for NativeGroupsApplicationLifecycleError {
    fn from(value: ServerFnError) -> Self {
        Self(value.to_string())
    }
}

pub async fn load_my_group_membership_application(
    query: GroupsStorefrontMyApplicationQuery,
) -> Result<Option<GroupsStorefrontMembershipApplication>, NativeGroupsApplicationLifecycleError> {
    groups_storefront_my_application_native(query)
        .await
        .map_err(Into::into)
}

pub async fn cancel_group_membership_application(
    command: CancelGroupMembershipApplicationCommand,
) -> Result<GroupsStorefrontApplicationLifecycleResult, NativeGroupsApplicationLifecycleError> {
    groups_storefront_cancel_application_native(command)
        .await
        .map_err(Into::into)
}

#[server(prefix = "/api/fn", endpoint = "groups/storefront/applications/my")]
async fn groups_storefront_my_application_native(
    query: GroupsStorefrontMyApplicationQuery,
) -> Result<Option<GroupsStorefrontMembershipApplication>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            AuthContext, HostRuntimeContext, PortActor, PortContext, TenantContext,
            request::RequestContext,
        };
        use rustok_groups::{
            GroupApplicationLifecycleReadPort, GroupApplicationService,
            ReadMyGroupMembershipApplicationRequest,
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
            format!(
                "groups-storefront-application-lifecycle-native-{}",
                Uuid::new_v4()
            ),
        )
        .with_deadline(Duration::from_secs(5));
        for permission in auth.permissions {
            context = context.with_claim(permission.to_string());
        }
        GroupApplicationLifecycleReadPort::read_my_group_membership_application(
            &GroupApplicationService::new(runtime.db_clone()),
            context,
            ReadMyGroupMembershipApplicationRequest { group_id },
        )
        .await
        .map(|application| application.map(map_application))
        .map_err(|error| ServerFnError::new(format!("{}: {}", error.code, error.message)))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = query;
        Err(ServerFnError::new(
            "groups storefront application lifecycle native transport requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "groups/storefront/applications/cancel")]
async fn groups_storefront_cancel_application_native(
    command: CancelGroupMembershipApplicationCommand,
) -> Result<GroupsStorefrontApplicationLifecycleResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            AuthContext, HostRuntimeContext, PortActor, PortContext, TenantContext,
            request::RequestContext,
        };
        use rustok_groups::{
            CancelGroupMembershipApplicationRequest, GroupApplicationLifecycleCommandPort,
            GroupApplicationService,
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
        let application_id = Uuid::parse_str(&command.application_id)
            .map_err(|_| ServerFnError::new("application_id must be a UUID"))?;
        let mut context = PortContext::new(
            tenant.id.to_string(),
            PortActor::user(auth.user_id.to_string()),
            request.locale,
            format!(
                "groups-storefront-application-lifecycle-native-{}",
                Uuid::new_v4()
            ),
        )
        .with_deadline(Duration::from_secs(5))
        .with_idempotency_key(command.idempotency_key);
        for permission in auth.permissions {
            context = context.with_claim(permission.to_string());
        }
        let result = GroupApplicationLifecycleCommandPort::cancel_group_membership_application(
            &GroupApplicationService::new(runtime.db_clone()),
            context,
            CancelGroupMembershipApplicationRequest { application_id },
        )
        .await
        .map_err(|error| ServerFnError::new(format!("{}: {}", error.code, error.message)))?;
        Ok(GroupsStorefrontApplicationLifecycleResult {
            application: map_application(result.application),
            membership: GroupsStorefrontApplicationMembership {
                id: result.membership.id.to_string(),
                group_id: result.membership.group_id.to_string(),
                user_id: result.membership.user_id.to_string(),
                role: result.membership.role.as_str().to_string(),
                status: result.membership.status.as_str().to_string(),
            },
            group_version: result.group_version,
            replayed: result.replayed,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = command;
        Err(ServerFnError::new(
            "groups storefront application lifecycle native transport requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn map_application(
    value: rustok_groups::GroupMembershipApplication,
) -> GroupsStorefrontMembershipApplication {
    GroupsStorefrontMembershipApplication {
        id: value.id.to_string(),
        group_id: value.group_id.to_string(),
        user_id: value.user_id.to_string(),
        policy_id: value.policy_id.to_string(),
        policy_revision: value.policy_revision,
        policy_locale: value.policy_locale,
        status: value.status.as_str().to_string(),
        submitted_at: value.submitted_at.to_rfc3339(),
    }
}
