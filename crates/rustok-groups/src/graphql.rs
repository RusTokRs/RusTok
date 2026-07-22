use std::time::Duration;

use async_graphql::{
    Context, Enum, ErrorExtensions, FieldError, InputObject, Json, Object, Result, SimpleObject,
};
use rustok_api::graphql::GraphQLError;
use rustok_api::request::RequestContext;
use rustok_api::{
    AuthContext, ChannelContext, HostRuntimeContext, Permission, PortActor, PortContext, PortError,
    PortErrorKind, TenantContext, has_any_effective_permission,
};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    CreateGroupInput, GroupCommandPort, GroupConnection, GroupDetails, GroupFeatureBinding,
    GroupJoinPolicy, GroupMembership, GroupMembershipReadPort, GroupRole, GroupStatus,
    GroupSummary, GroupSummaryReadPort, GroupVisibility, GroupsService, JoinGroupRequest,
    LeaveGroupRequest, ListGroupMembershipsRequest, ListGroupsRequest, ReadGroupRequest,
    SetGroupFeatureRequest,
};

const PORT_DEADLINE: Duration = Duration::from_secs(5);

#[derive(Default)]
pub struct GroupsQuery;

#[Object]
impl GroupsQuery {
    async fn groups(
        &self,
        ctx: &Context<'_>,
        page: Option<i32>,
        per_page: Option<i32>,
        search: Option<String>,
        include_non_public: Option<bool>,
    ) -> Result<GroupConnectionGql> {
        let include_non_public = include_non_public.unwrap_or(false);
        let auth = ctx.data_opt::<AuthContext>();
        if include_non_public {
            require_any_permission(ctx, &[Permission::GROUPS_LIST, Permission::GROUPS_READ])?;
        }
        let service = service(ctx)?;
        GroupSummaryReadPort::list_groups(
            &service,
            port_context(ctx, auth, None)?,
            ListGroupsRequest {
                page: page.unwrap_or(1).max(1) as u64,
                per_page: per_page.unwrap_or(24).clamp(1, 100) as u64,
                search: normalize_optional_text(search),
                include_non_public,
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn group(
        &self,
        ctx: &Context<'_>,
        id: Option<Uuid>,
        handle: Option<String>,
    ) -> Result<GroupDetailsGql> {
        let service = service(ctx)?;
        GroupSummaryReadPort::read_group(
            &service,
            port_context(ctx, ctx.data_opt::<AuthContext>(), None)?,
            ReadGroupRequest {
                group_id: id,
                handle: normalize_optional_text(handle),
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn group_memberships(
        &self,
        ctx: &Context<'_>,
        group_id: Uuid,
        page: Option<i32>,
        per_page: Option<i32>,
    ) -> Result<GroupMembershipConnectionGql> {
        let auth = require_authenticated(ctx)?;
        let service = service(ctx)?;
        GroupMembershipReadPort::list_memberships(
            &service,
            port_context(ctx, Some(auth), None)?,
            ListGroupMembershipsRequest {
                group_id,
                page: page.unwrap_or(1).max(1) as u64,
                per_page: per_page.unwrap_or(50).clamp(1, 100) as u64,
            },
        )
        .await
        .map(|connection| GroupMembershipConnectionGql {
            items: connection.items.into_iter().map(Into::into).collect(),
            total: connection.total,
            page: connection.page,
            per_page: connection.per_page,
        })
        .map_err(map_port_error)
    }
}

#[derive(Default)]
pub struct GroupsMutation;

#[Object]
impl GroupsMutation {
    async fn create_group(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        input: CreateGroupInputGql,
    ) -> Result<GroupDetailsGql> {
        let auth = require_any_permission(ctx, &[Permission::GROUPS_CREATE])?;
        let service = service(ctx)?;
        GroupCommandPort::create_group(
            &service,
            port_context(ctx, Some(auth), Some(idempotency_key))?,
            CreateGroupInput {
                handle: input.handle,
                locale: input.locale,
                title: input.title,
                summary: input.summary,
                body: input.body,
                visibility: input.visibility.into(),
                join_policy: input.join_policy.into(),
                category_id: input.category_id,
                avatar_media_id: input.avatar_media_id,
                cover_media_id: input.cover_media_id,
                metadata: input
                    .metadata
                    .map(|value| value.0)
                    .unwrap_or_else(empty_object),
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn join_group(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        group_id: Uuid,
    ) -> Result<GroupMembershipGql> {
        let auth = require_authenticated(ctx)?;
        let service = service(ctx)?;
        GroupCommandPort::join_group(
            &service,
            port_context(ctx, Some(auth), Some(idempotency_key))?,
            JoinGroupRequest { group_id },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn leave_group(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        group_id: Uuid,
    ) -> Result<GroupMembershipGql> {
        let auth = require_authenticated(ctx)?;
        let service = service(ctx)?;
        GroupCommandPort::leave_group(
            &service,
            port_context(ctx, Some(auth), Some(idempotency_key))?,
            LeaveGroupRequest { group_id },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    async fn set_group_feature(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        group_id: Uuid,
        input: SetGroupFeatureInputGql,
    ) -> Result<GroupFeatureBindingGql> {
        let auth = require_authenticated(ctx)?;
        let service = service(ctx)?;
        GroupCommandPort::set_group_feature(
            &service,
            port_context(ctx, Some(auth), Some(idempotency_key))?,
            SetGroupFeatureRequest {
                group_id,
                feature_key: input.feature_key,
                contract_version: input.contract_version,
                enabled: input.enabled,
                sort_order: input.sort_order.unwrap_or(0),
                configuration: input
                    .configuration
                    .map(|value| value.0)
                    .unwrap_or_else(empty_object),
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }
}

#[derive(InputObject)]
pub struct CreateGroupInputGql {
    pub handle: String,
    pub locale: String,
    pub title: String,
    pub summary: Option<String>,
    pub body: Option<String>,
    pub visibility: GroupVisibilityGql,
    pub join_policy: GroupJoinPolicyGql,
    pub category_id: Option<Uuid>,
    pub avatar_media_id: Option<Uuid>,
    pub cover_media_id: Option<Uuid>,
    pub metadata: Option<Json<Value>>,
}

#[derive(InputObject)]
pub struct SetGroupFeatureInputGql {
    pub feature_key: String,
    pub contract_version: String,
    pub enabled: bool,
    pub sort_order: Option<i32>,
    pub configuration: Option<Json<Value>>,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum GroupVisibilityGql {
    Public,
    Closed,
    Secret,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum GroupJoinPolicyGql {
    Open,
    Request,
    InviteOnly,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum GroupStatusGql {
    Active,
    Archived,
    Suspended,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum GroupRoleGql {
    Owner,
    Admin,
    Moderator,
    Member,
}

#[derive(SimpleObject)]
pub struct GroupSummaryGql {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub owner_user_id: Uuid,
    pub handle: String,
    pub visibility: GroupVisibilityGql,
    pub join_policy: GroupJoinPolicyGql,
    pub status: GroupStatusGql,
    pub title: String,
    pub summary: Option<String>,
    pub avatar_media_id: Option<Uuid>,
    pub cover_media_id: Option<Uuid>,
    pub member_count: u64,
    pub requested_locale: String,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
}

#[derive(SimpleObject)]
pub struct GroupDetailsGql {
    pub summary: GroupSummaryGql,
    pub body: Option<String>,
    pub viewer_membership: Option<GroupMembershipGql>,
    pub features: Vec<GroupFeatureBindingGql>,
}

#[derive(SimpleObject)]
pub struct GroupConnectionGql {
    pub items: Vec<GroupSummaryGql>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

#[derive(SimpleObject)]
pub struct GroupMembershipGql {
    pub id: Uuid,
    pub group_id: Uuid,
    pub user_id: Uuid,
    pub role: GroupRoleGql,
    pub status: String,
}

#[derive(SimpleObject)]
pub struct GroupMembershipConnectionGql {
    pub items: Vec<GroupMembershipGql>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

#[derive(SimpleObject)]
pub struct GroupFeatureBindingGql {
    pub id: Uuid,
    pub group_id: Uuid,
    pub feature_key: String,
    pub owner_module: String,
    pub contract_version: String,
    pub status: String,
    pub sort_order: i32,
    pub configuration: Json<Value>,
}

impl From<GroupConnection> for GroupConnectionGql {
    fn from(value: GroupConnection) -> Self {
        Self {
            items: value.items.into_iter().map(Into::into).collect(),
            total: value.total,
            page: value.page,
            per_page: value.per_page,
        }
    }
}

impl From<GroupSummary> for GroupSummaryGql {
    fn from(value: GroupSummary) -> Self {
        Self {
            id: value.id,
            tenant_id: value.tenant_id,
            owner_user_id: value.owner_user_id,
            handle: value.handle,
            visibility: value.visibility.into(),
            join_policy: value.join_policy.into(),
            status: value.status.into(),
            title: value.title,
            summary: value.summary,
            avatar_media_id: value.avatar_media_id,
            cover_media_id: value.cover_media_id,
            member_count: value.member_count,
            requested_locale: value.requested_locale,
            effective_locale: value.effective_locale,
            available_locales: value.available_locales,
        }
    }
}

impl From<GroupDetails> for GroupDetailsGql {
    fn from(value: GroupDetails) -> Self {
        Self {
            summary: value.summary.into(),
            body: value.body,
            viewer_membership: value.viewer_membership.map(Into::into),
            features: value.features.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<GroupMembership> for GroupMembershipGql {
    fn from(value: GroupMembership) -> Self {
        Self {
            id: value.id,
            group_id: value.group_id,
            user_id: value.user_id,
            role: value.role.into(),
            status: value.status.as_str().to_string(),
        }
    }
}

impl From<GroupFeatureBinding> for GroupFeatureBindingGql {
    fn from(value: GroupFeatureBinding) -> Self {
        Self {
            id: value.id,
            group_id: value.group_id,
            feature_key: value.feature_key,
            owner_module: value.owner_module,
            contract_version: value.contract_version,
            status: value.status.as_str().to_string(),
            sort_order: value.sort_order,
            configuration: Json(value.configuration),
        }
    }
}

impl From<GroupVisibilityGql> for GroupVisibility {
    fn from(value: GroupVisibilityGql) -> Self {
        match value {
            GroupVisibilityGql::Public => Self::Public,
            GroupVisibilityGql::Closed => Self::Closed,
            GroupVisibilityGql::Secret => Self::Secret,
        }
    }
}

impl From<GroupVisibility> for GroupVisibilityGql {
    fn from(value: GroupVisibility) -> Self {
        match value {
            GroupVisibility::Public => Self::Public,
            GroupVisibility::Closed => Self::Closed,
            GroupVisibility::Secret => Self::Secret,
        }
    }
}

impl From<GroupJoinPolicyGql> for GroupJoinPolicy {
    fn from(value: GroupJoinPolicyGql) -> Self {
        match value {
            GroupJoinPolicyGql::Open => Self::Open,
            GroupJoinPolicyGql::Request => Self::Request,
            GroupJoinPolicyGql::InviteOnly => Self::InviteOnly,
        }
    }
}

impl From<GroupJoinPolicy> for GroupJoinPolicyGql {
    fn from(value: GroupJoinPolicy) -> Self {
        match value {
            GroupJoinPolicy::Open => Self::Open,
            GroupJoinPolicy::Request => Self::Request,
            GroupJoinPolicy::InviteOnly => Self::InviteOnly,
        }
    }
}

impl From<GroupStatus> for GroupStatusGql {
    fn from(value: GroupStatus) -> Self {
        match value {
            GroupStatus::Active => Self::Active,
            GroupStatus::Archived => Self::Archived,
            GroupStatus::Suspended => Self::Suspended,
        }
    }
}

impl From<GroupRole> for GroupRoleGql {
    fn from(value: GroupRole) -> Self {
        match value {
            GroupRole::Owner => Self::Owner,
            GroupRole::Admin => Self::Admin,
            GroupRole::Moderator => Self::Moderator,
            GroupRole::Member => Self::Member,
        }
    }
}

fn service(ctx: &Context<'_>) -> Result<GroupsService> {
    let runtime = ctx.data::<HostRuntimeContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error("Groups runtime is not registered")
    })?;
    Ok(GroupsService::new(runtime.db_clone()))
}

fn require_authenticated<'a>(ctx: &'a Context<'a>) -> Result<&'a AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    require_tenant(ctx, auth)?;
    Ok(auth)
}

fn require_any_permission<'a>(
    ctx: &'a Context<'a>,
    required: &[Permission],
) -> Result<&'a AuthContext> {
    let auth = require_authenticated(ctx)?;
    if !has_any_effective_permission(&auth.permissions, required) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "groups permission required",
        ));
    }
    Ok(auth)
}

fn require_tenant<'a>(ctx: &'a Context<'a>, auth: &AuthContext) -> Result<&'a TenantContext> {
    let tenant = ctx.data::<TenantContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error("Groups tenant context is not registered")
    })?;
    if auth.tenant_id != tenant.id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "groups tenant mismatch",
        ));
    }
    Ok(tenant)
}

fn port_context(
    ctx: &Context<'_>,
    auth: Option<&AuthContext>,
    idempotency_key: Option<String>,
) -> Result<PortContext> {
    let tenant = ctx.data::<TenantContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error("Groups tenant context is not registered")
    })?;
    if let Some(auth) = auth {
        require_tenant(ctx, auth)?;
    }
    let locale = ctx
        .data::<RequestContext>()
        .map(|request| request.locale.clone())
        .or_else(|_| {
            ctx.data::<rustok_core::Locale>()
                .map(|locale| locale.as_str().to_string())
        })
        .unwrap_or_else(|_| tenant.default_locale.clone());
    let actor = auth
        .map(|auth| PortActor::user(auth.user_id.to_string()))
        .unwrap_or_else(|| PortActor::service("groups-public-graphql"));
    let mut context = PortContext::new(
        tenant.id.to_string(),
        actor,
        locale,
        format!("graphql-groups-{}", Uuid::new_v4()),
    )
    .with_deadline(PORT_DEADLINE);
    if let Some(auth) = auth {
        for permission in &auth.permissions {
            context = context.with_claim(permission.to_string());
        }
    }
    if let Ok(channel) = ctx.data::<ChannelContext>() {
        context = context.with_channel(channel.slug.clone());
    }
    if let Some(key) = idempotency_key {
        context = context.with_idempotency_key(key);
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
                "Groups service is temporarily unavailable",
            )
        }
        PortErrorKind::InvariantViolation => {
            <FieldError as GraphQLError>::internal_error("Groups operation requires review")
        }
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn empty_object() -> Value {
    serde_json::json!({})
}
