use std::time::Duration;

use async_graphql::{Context, FieldError, Object, Result, SimpleObject};
use rustok_api::graphql::GraphQLError;
use rustok_api::request::RequestContext;
use rustok_api::{
    AuthContext, ChannelContext, HostRuntimeContext, PortActor, PortContext, PortError,
    PortErrorKind, TenantContext,
};
use uuid::Uuid;

use crate::graphql::GroupMembershipGql;
use crate::graphql_applications::GroupMembershipApplicationGql;
use crate::{
    CancelGroupMembershipApplicationRequest, GroupApplicationLifecycleCommandPort,
    GroupApplicationLifecycleReadPort, GroupApplicationLifecycleResult, GroupApplicationService,
    ReadMyGroupMembershipApplicationRequest, ReopenGroupMembershipApplicationRequest,
};

const PORT_DEADLINE: Duration = Duration::from_secs(5);

#[derive(Default)]
pub struct GroupsApplicationLifecycleQuery;

#[Object]
impl GroupsApplicationLifecycleQuery {
    async fn my_group_membership_application(
        &self,
        ctx: &Context<'_>,
        group_id: Uuid,
    ) -> Result<Option<GroupMembershipApplicationGql>> {
        let auth = require_authenticated(ctx)?;
        GroupApplicationLifecycleReadPort::read_my_group_membership_application(
            &application_service(ctx)?,
            port_context(ctx, auth, None)?,
            ReadMyGroupMembershipApplicationRequest { group_id },
        )
        .await
        .map(|application| application.map(Into::into))
        .map_err(map_port_error)
    }
}

#[derive(Default)]
pub struct GroupsApplicationLifecycleMutation;

#[Object]
impl GroupsApplicationLifecycleMutation {
    async fn cancel_group_membership_application(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        application_id: Uuid,
    ) -> Result<GroupApplicationLifecycleResultGql> {
        let auth = require_authenticated(ctx)?;
        GroupApplicationLifecycleCommandPort::cancel_group_membership_application(
            &application_service(ctx)?,
            port_context(ctx, auth, Some(idempotency_key))?,
            CancelGroupMembershipApplicationRequest { application_id },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn reopen_group_membership_application(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        application_id: Uuid,
    ) -> Result<GroupApplicationLifecycleResultGql> {
        let auth = require_authenticated(ctx)?;
        GroupApplicationLifecycleCommandPort::reopen_group_membership_application(
            &application_service(ctx)?,
            port_context(ctx, auth, Some(idempotency_key))?,
            ReopenGroupMembershipApplicationRequest { application_id },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }
}

#[derive(SimpleObject)]
pub struct GroupApplicationLifecycleResultGql {
    pub application: GroupMembershipApplicationGql,
    pub membership: GroupMembershipGql,
    pub group_version: u64,
    pub replayed: bool,
}

impl From<GroupApplicationLifecycleResult> for GroupApplicationLifecycleResultGql {
    fn from(value: GroupApplicationLifecycleResult) -> Self {
        Self {
            application: value.application.into(),
            membership: value.membership.into(),
            group_version: value.group_version,
            replayed: value.replayed,
        }
    }
}

fn application_service(ctx: &Context<'_>) -> Result<GroupApplicationService> {
    let runtime = ctx.data::<HostRuntimeContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error("Groups runtime is not registered")
    })?;
    Ok(GroupApplicationService::new(runtime.db_clone()))
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
    idempotency_key: Option<String>,
) -> Result<PortContext> {
    let tenant = ctx.data::<TenantContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error("Groups tenant context is not registered")
    })?;
    if idempotency_key
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(<FieldError as GraphQLError>::bad_user_input(
            "groups application lifecycle idempotency key is required",
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
        format!("graphql-groups-application-lifecycle-{}", Uuid::new_v4()),
    )
    .with_deadline(PORT_DEADLINE);
    if let Some(idempotency_key) = idempotency_key {
        context = context.with_idempotency_key(idempotency_key);
    }
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
                "Groups membership application lifecycle is temporarily unavailable",
            )
        }
        PortErrorKind::InvariantViolation => {
            <FieldError as GraphQLError>::internal_error(
                "Groups membership application lifecycle requires review",
            )
        }
    }
}
