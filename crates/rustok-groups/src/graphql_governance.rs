use std::time::Duration;

use async_graphql::{Context, FieldError, MergedObject, Object, Result, SimpleObject};
use rustok_api::graphql::GraphQLError;
use rustok_api::request::RequestContext;
use rustok_api::{
    AuthContext, ChannelContext, HostRuntimeContext, PortActor, PortContext, PortError,
    PortErrorKind, TenantContext,
};
use uuid::Uuid;

use crate::graphql::{GroupRoleGql, GroupsMutation};
use crate::graphql_localization::GroupsLocalizationMutation;
use crate::{
    ChangeGroupRoleRequest, GroupGovernanceCommandPort, GroupGovernanceResult,
    GroupGovernanceService, TransferGroupOwnershipRequest,
};

const PORT_DEADLINE: Duration = Duration::from_secs(5);

#[derive(MergedObject, Default)]
pub struct GroupsMutationRoot(
    GroupsMutation,
    GroupsGovernanceMutation,
    GroupsLocalizationMutation,
);

#[derive(Default)]
pub struct GroupsGovernanceMutation;

#[Object]
impl GroupsGovernanceMutation {
    async fn change_group_role(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        group_id: Uuid,
        target_user_id: Uuid,
        role: GroupRoleGql,
    ) -> Result<GroupGovernanceResultGql> {
        let auth = require_authenticated(ctx)?;
        let service = governance_service(ctx)?;
        GroupGovernanceCommandPort::change_group_role(
            &service,
            port_context(ctx, auth, idempotency_key)?,
            ChangeGroupRoleRequest {
                group_id,
                target_user_id,
                role: role.into(),
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn transfer_group_ownership(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        group_id: Uuid,
        new_owner_user_id: Uuid,
    ) -> Result<GroupGovernanceResultGql> {
        let auth = require_authenticated(ctx)?;
        let service = governance_service(ctx)?;
        GroupGovernanceCommandPort::transfer_group_ownership(
            &service,
            port_context(ctx, auth, idempotency_key)?,
            TransferGroupOwnershipRequest {
                group_id,
                new_owner_user_id,
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }
}

#[derive(SimpleObject)]
pub struct GroupGovernanceResultGql {
    pub group_id: Uuid,
    pub actor_user_id: Uuid,
    pub target_user_id: Uuid,
    pub previous_role: GroupRoleGql,
    pub current_role: GroupRoleGql,
    pub group_version: u64,
    pub replayed: bool,
}

impl From<GroupGovernanceResult> for GroupGovernanceResultGql {
    fn from(value: GroupGovernanceResult) -> Self {
        Self {
            group_id: value.group_id,
            actor_user_id: value.actor_user_id,
            target_user_id: value.target_user_id,
            previous_role: value.previous_role.into(),
            current_role: value.current_role.into(),
            group_version: value.group_version,
            replayed: value.replayed,
        }
    }
}

fn governance_service(ctx: &Context<'_>) -> Result<GroupGovernanceService> {
    let runtime = ctx.data::<HostRuntimeContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error("Groups runtime is not registered")
    })?;
    Ok(GroupGovernanceService::new(runtime.db_clone()))
}

fn require_authenticated<'a>(ctx: &'a Context<'a>) -> Result<&'a AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    let tenant = ctx.data::<TenantContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error("Groups tenant context is not registered")
    })?;
    if auth.tenant_id != tenant.id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "groups tenant mismatch",
        ));
    }
    Ok(auth)
}

fn port_context(
    ctx: &Context<'_>,
    auth: &AuthContext,
    idempotency_key: String,
) -> Result<PortContext> {
    let tenant = ctx.data::<TenantContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error("Groups tenant context is not registered")
    })?;
    if auth.tenant_id != tenant.id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "groups tenant mismatch",
        ));
    }
    if idempotency_key.trim().is_empty() {
        return Err(<FieldError as GraphQLError>::bad_user_input(
            "groups governance idempotency key is required",
        ));
    }

    let locale = ctx
        .data::<RequestContext>()
        .map(|request| request.locale.clone())
        .or_else(|_| {
            ctx.data::<rustok_core::Locale>()
                .map(|locale| locale.as_str().to_string())
        })
        .unwrap_or_else(|_| tenant.default_locale.clone());
    let mut context = PortContext::new(
        tenant.id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        locale,
        format!("graphql-groups-governance-{}", Uuid::new_v4()),
    )
    .with_deadline(PORT_DEADLINE)
    .with_idempotency_key(idempotency_key);
    for permission in &auth.permissions {
        context = context.with_claim(permission.to_string());
    }
    if let Ok(channel) = ctx.data::<ChannelContext>() {
        context = context.with_channel(channel.slug.clone());
    }
    Ok(context)
}

fn map_port_error(error: PortError) -> FieldError {
    match error.kind {
        PortErrorKind::Validation | PortErrorKind::Conflict => {
            <FieldError as GraphQLError>::bad_user_input(&error.message)
        }
        PortErrorKind::NotFound => <FieldError as GraphQLError>::not_found(&error.message),
        PortErrorKind::Forbidden => {
            <FieldError as GraphQLError>::permission_denied(&error.message)
        }
        PortErrorKind::Unavailable | PortErrorKind::Timeout => {
            <FieldError as GraphQLError>::internal_error(
                "Groups governance service is temporarily unavailable",
            )
        }
        PortErrorKind::InvariantViolation => {
            <FieldError as GraphQLError>::internal_error(
                "Groups governance operation requires review",
            )
        }
    }
}
