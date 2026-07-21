use std::time::Duration;

use async_graphql::{Context, FieldError, MergedObject, Object, Result};
use rustok_api::graphql::GraphQLError;
use rustok_api::request::RequestContext;
use rustok_api::{
    AuthContext, ChannelContext, HostRuntimeContext, PortActor, PortContext, PortError,
    PortErrorKind, TenantContext,
};
use uuid::Uuid;

use crate::graphql_invitations::{
    AcceptGroupInvitationResultGql, GroupsMutationRoot as GroupsBaseMutationRoot,
};
use crate::{
    AcceptTargetedGroupInvitationRequest, GroupTargetedInvitationCommandPort,
    GroupTargetedInvitationService,
};

pub use crate::graphql_invitations::GroupsQueryRoot;

const PORT_DEADLINE: Duration = Duration::from_secs(5);

#[derive(MergedObject, Default)]
pub struct GroupsMutationRoot(GroupsBaseMutationRoot, GroupsTargetedInvitationsMutation);

#[derive(Default)]
pub struct GroupsTargetedInvitationsMutation;

#[Object]
impl GroupsTargetedInvitationsMutation {
    async fn accept_targeted_group_invitation(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        invitation_id: Uuid,
    ) -> Result<AcceptGroupInvitationResultGql> {
        let auth = require_authenticated(ctx)?;
        let runtime = ctx.data::<HostRuntimeContext>().map_err(|_| {
            <FieldError as GraphQLError>::internal_error("Groups runtime is not registered")
        })?;
        GroupTargetedInvitationCommandPort::accept_targeted_group_invitation(
            &GroupTargetedInvitationService::new(runtime.db_clone()),
            port_context(ctx, auth, idempotency_key)?,
            AcceptTargetedGroupInvitationRequest { invitation_id },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }
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
    let idempotency_key = idempotency_key.trim();
    if idempotency_key.is_empty() {
        return Err(<FieldError as GraphQLError>::bad_user_input(
            "groups targeted invitation idempotency key is required",
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
        format!("graphql-groups-targeted-invitations-{}", Uuid::new_v4()),
    )
    .with_deadline(PORT_DEADLINE)
    .with_idempotency_key(idempotency_key.to_string());
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
                "Groups targeted invitation service is temporarily unavailable",
            )
        }
        PortErrorKind::InvariantViolation => {
            <FieldError as GraphQLError>::internal_error(
                "Groups targeted invitation operation requires review",
            )
        }
    }
}
