use std::time::Duration;

use async_graphql::{Context, FieldError, InputObject, MergedObject, Object, Result, SimpleObject};
use chrono::{DateTime, Utc};
use rustok_api::graphql::GraphQLError;
use rustok_api::request::RequestContext;
use rustok_api::{
    AuthContext, ChannelContext, HostRuntimeContext, PortActor, PortContext, PortError,
    PortErrorKind, TenantContext,
};
use uuid::Uuid;

use crate::graphql::GroupMembershipGql;
use crate::graphql_governance::GroupsMutationRoot as GroupsBaseMutationRoot;
use crate::graphql_localization::GroupsQueryRoot as GroupsBaseQueryRoot;
use crate::{
    AcceptGroupInvitationRequest, AcceptGroupInvitationResult,
    AcceptTargetedGroupInvitationRequest, CreateGroupInvitationRequest,
    CreateGroupInvitationResult, GroupInvitation, GroupInvitationCommandPort,
    GroupInvitationConnection, GroupInvitationReadPort, GroupInvitationService,
    GroupTargetedInvitationCommandPort, GroupTargetedInvitationService,
    ListGroupInvitationsRequest, RevokeGroupInvitationRequest, RevokeGroupInvitationResult,
};

const PORT_DEADLINE: Duration = Duration::from_secs(5);

#[derive(MergedObject, Default)]
pub struct GroupsQueryRoot(GroupsBaseQueryRoot, GroupsInvitationsQuery);

#[derive(MergedObject, Default)]
pub struct GroupsMutationRoot(GroupsBaseMutationRoot, GroupsInvitationsMutation);

#[derive(Default)]
pub struct GroupsInvitationsQuery;

#[Object]
impl GroupsInvitationsQuery {
    async fn group_invitations(
        &self,
        ctx: &Context<'_>,
        group_id: Uuid,
        page: Option<i32>,
        per_page: Option<i32>,
        include_inactive: Option<bool>,
    ) -> Result<GroupInvitationConnectionGql> {
        let auth = require_authenticated(ctx)?;
        let service = invitation_service(ctx)?;
        GroupInvitationReadPort::list_group_invitations(
            &service,
            port_context(ctx, auth, None)?,
            ListGroupInvitationsRequest {
                group_id,
                page: page.unwrap_or(1).max(1) as u64,
                per_page: per_page.unwrap_or(24).clamp(1, 100) as u64,
                include_inactive: include_inactive.unwrap_or(false),
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }
}

#[derive(Default)]
pub struct GroupsInvitationsMutation;

#[Object]
impl GroupsInvitationsMutation {
    async fn create_group_invitation(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        group_id: Uuid,
        input: CreateGroupInvitationInputGql,
    ) -> Result<CreateGroupInvitationResultGql> {
        let auth = require_authenticated(ctx)?;
        let expires_in_seconds = u64::try_from(input.expires_in_seconds).map_err(|_| {
            <FieldError as GraphQLError>::bad_user_input(
                "invitation expiry must be a positive number of seconds",
            )
        })?;
        let max_uses = u32::try_from(input.max_uses).map_err(|_| {
            <FieldError as GraphQLError>::bad_user_input(
                "invitation max uses must be a positive integer",
            )
        })?;
        let service = invitation_service(ctx)?;
        GroupInvitationCommandPort::create_group_invitation(
            &service,
            port_context(ctx, auth, Some(idempotency_key))?,
            CreateGroupInvitationRequest {
                group_id,
                target_user_id: input.target_user_id,
                expires_in_seconds,
                max_uses,
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn revoke_group_invitation(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        invitation_id: Uuid,
    ) -> Result<RevokeGroupInvitationResultGql> {
        let auth = require_authenticated(ctx)?;
        let service = invitation_service(ctx)?;
        GroupInvitationCommandPort::revoke_group_invitation(
            &service,
            port_context(ctx, auth, Some(idempotency_key))?,
            RevokeGroupInvitationRequest { invitation_id },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn accept_group_invitation(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        token: String,
    ) -> Result<AcceptGroupInvitationResultGql> {
        let auth = require_authenticated(ctx)?;
        let service = invitation_service(ctx)?;
        GroupInvitationCommandPort::accept_group_invitation(
            &service,
            port_context(ctx, auth, Some(idempotency_key))?,
            AcceptGroupInvitationRequest { token },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn accept_targeted_group_invitation(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        invitation_id: Uuid,
    ) -> Result<AcceptGroupInvitationResultGql> {
        let auth = require_authenticated(ctx)?;
        let service = targeted_invitation_service(ctx)?;
        GroupTargetedInvitationCommandPort::accept_targeted_group_invitation(
            &service,
            port_context(ctx, auth, Some(idempotency_key))?,
            AcceptTargetedGroupInvitationRequest { invitation_id },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }
}

#[derive(InputObject)]
pub struct CreateGroupInvitationInputGql {
    pub target_user_id: Option<Uuid>,
    pub expires_in_seconds: i64,
    pub max_uses: i64,
}

#[derive(SimpleObject)]
pub struct GroupInvitationGql {
    pub id: Uuid,
    pub group_id: Uuid,
    pub invited_by_user_id: Uuid,
    pub target_user_id: Option<Uuid>,
    pub max_uses: u32,
    pub use_count: u32,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub revoked_by_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub status: String,
}

#[derive(SimpleObject)]
pub struct GroupInvitationConnectionGql {
    pub items: Vec<GroupInvitationGql>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

#[derive(SimpleObject)]
pub struct CreateGroupInvitationResultGql {
    pub invitation: GroupInvitationGql,
    pub token: Option<String>,
    pub group_version: u64,
    pub replayed: bool,
}

#[derive(SimpleObject)]
pub struct RevokeGroupInvitationResultGql {
    pub invitation: GroupInvitationGql,
    pub group_version: u64,
    pub replayed: bool,
}

#[derive(SimpleObject)]
pub struct AcceptGroupInvitationResultGql {
    pub invitation_id: Uuid,
    pub group_id: Uuid,
    pub membership: GroupMembershipGql,
    pub group_version: u64,
    pub replayed: bool,
}

impl From<GroupInvitation> for GroupInvitationGql {
    fn from(value: GroupInvitation) -> Self {
        Self {
            id: value.id,
            group_id: value.group_id,
            invited_by_user_id: value.invited_by_user_id,
            target_user_id: value.target_user_id,
            max_uses: value.max_uses,
            use_count: value.use_count,
            expires_at: value.expires_at,
            revoked_at: value.revoked_at,
            revoked_by_user_id: value.revoked_by_user_id,
            created_at: value.created_at,
            status: value.status.as_str().to_string(),
        }
    }
}

impl From<GroupInvitationConnection> for GroupInvitationConnectionGql {
    fn from(value: GroupInvitationConnection) -> Self {
        Self {
            items: value.items.into_iter().map(Into::into).collect(),
            total: value.total,
            page: value.page,
            per_page: value.per_page,
        }
    }
}

impl From<CreateGroupInvitationResult> for CreateGroupInvitationResultGql {
    fn from(value: CreateGroupInvitationResult) -> Self {
        Self {
            invitation: value.invitation.into(),
            token: value.token,
            group_version: value.group_version,
            replayed: value.replayed,
        }
    }
}

impl From<RevokeGroupInvitationResult> for RevokeGroupInvitationResultGql {
    fn from(value: RevokeGroupInvitationResult) -> Self {
        Self {
            invitation: value.invitation.into(),
            group_version: value.group_version,
            replayed: value.replayed,
        }
    }
}

impl From<AcceptGroupInvitationResult> for AcceptGroupInvitationResultGql {
    fn from(value: AcceptGroupInvitationResult) -> Self {
        Self {
            invitation_id: value.invitation_id,
            group_id: value.group_id,
            membership: value.membership.into(),
            group_version: value.group_version,
            replayed: value.replayed,
        }
    }
}

fn invitation_service(ctx: &Context<'_>) -> Result<GroupInvitationService> {
    let runtime = ctx.data::<HostRuntimeContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error("Groups runtime is not registered")
    })?;
    Ok(GroupInvitationService::new(runtime.db_clone()))
}

fn targeted_invitation_service(ctx: &Context<'_>) -> Result<GroupTargetedInvitationService> {
    let runtime = ctx.data::<HostRuntimeContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error("Groups runtime is not registered")
    })?;
    Ok(GroupTargetedInvitationService::new(runtime.db_clone()))
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
    if auth.tenant_id != tenant.id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "groups tenant mismatch",
        ));
    }
    if idempotency_key
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(<FieldError as GraphQLError>::bad_user_input(
            "groups invitation idempotency key is required",
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
        format!("graphql-groups-invitations-{}", Uuid::new_v4()),
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
        PortErrorKind::Forbidden => <FieldError as GraphQLError>::permission_denied(&error.message),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => {
            <FieldError as GraphQLError>::internal_error(
                "Groups invitation service is temporarily unavailable",
            )
        }
        PortErrorKind::InvariantViolation => <FieldError as GraphQLError>::internal_error(
            "Groups invitation operation requires review",
        ),
    }
}
