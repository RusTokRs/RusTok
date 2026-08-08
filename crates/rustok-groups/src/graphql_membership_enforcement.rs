use std::time::Duration;

use async_graphql::{Context, ErrorExtensions, FieldError, Object, Result, SimpleObject};
use chrono::{DateTime, Utc};
use rustok_api::graphql::GraphQLError;
use rustok_api::request::RequestContext;
use rustok_api::{
    AuthContext, ChannelContext, HostRuntimeContext, PortActor, PortContext, PortError,
    PortErrorKind, TenantContext,
};
use uuid::Uuid;

use crate::{
    GroupMembershipEnforcementCommandPort, GroupMembershipEnforcementCommandService,
    GroupMembershipEnforcementMutationResult, RevokeGroupMembershipSuspensionRequest,
    SuspendGroupMembershipRequest,
};

const PORT_DEADLINE: Duration = Duration::from_secs(5);
const DOMAIN_CODE_EXTENSION: &str = "domainCode";
const RETRYABLE_EXTENSION: &str = "retryable";

#[derive(Default)]
pub struct GroupsMembershipEnforcementMutation;

#[Object]
impl GroupsMembershipEnforcementMutation {
    /// Suspends one group membership through the canonical Groups owner command.
    ///
    /// Transport only establishes authenticated tenant/user context and forwards effective
    /// permissions as claims. Local owner/admin/moderator hierarchy, platform authority,
    /// owner/self protection, receipt replay and expected-revision CAS remain owner decisions.
    async fn suspend_group_membership(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        group_id: Uuid,
        target_user_id: Uuid,
        expected_membership_revision: i64,
        reason_code: String,
        effective_until: Option<DateTime<Utc>>,
    ) -> Result<GroupMembershipEnforcementMutationResultGql> {
        let auth = require_authenticated(ctx)?;
        let service = enforcement_service(ctx)?;
        GroupMembershipEnforcementCommandPort::suspend_membership(
            &service,
            port_context(ctx, auth, idempotency_key, "suspend")?,
            SuspendGroupMembershipRequest {
                group_id,
                target_user_id,
                expected_membership_revision,
                reason_code,
                effective_until,
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }

    /// Revokes one active direct-local suspension through the canonical Groups owner command.
    async fn revoke_group_membership_suspension(
        &self,
        ctx: &Context<'_>,
        idempotency_key: String,
        group_id: Uuid,
        target_user_id: Uuid,
        expected_membership_revision: i64,
        reason_code: String,
    ) -> Result<GroupMembershipEnforcementMutationResultGql> {
        let auth = require_authenticated(ctx)?;
        let service = enforcement_service(ctx)?;
        GroupMembershipEnforcementCommandPort::revoke_membership_suspension(
            &service,
            port_context(ctx, auth, idempotency_key, "revoke")?,
            RevokeGroupMembershipSuspensionRequest {
                group_id,
                target_user_id,
                expected_membership_revision,
                reason_code,
            },
        )
        .await
        .map(Into::into)
        .map_err(map_port_error)
    }
}

#[derive(SimpleObject)]
pub struct GroupMembershipEnforcementMutationResultGql {
    pub group_id: Uuid,
    pub membership_id: Uuid,
    pub user_id: Uuid,
    pub membership_revision: i64,
    pub group_version: i64,
    pub member_count: i64,
    pub effective_status: String,
    pub enforcement_revision: i64,
    pub effective_until: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub replayed: bool,
}

impl From<GroupMembershipEnforcementMutationResult>
    for GroupMembershipEnforcementMutationResultGql
{
    fn from(value: GroupMembershipEnforcementMutationResult) -> Self {
        Self {
            group_id: value.group_id,
            membership_id: value.membership_id,
            user_id: value.user_id,
            membership_revision: value.membership_revision,
            group_version: value.group_version,
            member_count: value.member_count,
            effective_status: value.effective_status.as_str().to_string(),
            enforcement_revision: value.enforcement_revision,
            effective_until: value.effective_until,
            revoked_at: value.revoked_at,
            replayed: value.replayed,
        }
    }
}

fn enforcement_service(ctx: &Context<'_>) -> Result<GroupMembershipEnforcementCommandService> {
    let runtime = ctx.data::<HostRuntimeContext>().map_err(|_| {
        <FieldError as GraphQLError>::internal_error("Groups runtime is not registered")
    })?;
    Ok(GroupMembershipEnforcementCommandService::new(
        runtime.db_clone(),
    ))
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
    operation: &str,
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
            "groups membership enforcement idempotency key is required",
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
        format!("graphql-groups-membership-enforcement-{operation}-{}", Uuid::new_v4()),
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
    let domain_code = error.code.clone();
    let retryable = error.retryable;
    let transport_error = match error.kind {
        PortErrorKind::Validation | PortErrorKind::Conflict => {
            <FieldError as GraphQLError>::bad_user_input(&error.message)
        }
        PortErrorKind::NotFound => <FieldError as GraphQLError>::not_found(&error.message),
        PortErrorKind::Forbidden => <FieldError as GraphQLError>::permission_denied(&error.message),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => {
            <FieldError as GraphQLError>::internal_error(
                "Groups membership enforcement is temporarily unavailable",
            )
        }
        PortErrorKind::InvariantViolation => <FieldError as GraphQLError>::internal_error(
            "Groups membership enforcement requires review",
        ),
    };

    // Keep the common GraphQL `code` extension as the transport classification while preserving
    // the stable owner PortError identity separately. Clients can branch on the domain code without
    // losing the platform-wide BAD_USER_INPUT/PERMISSION_DENIED/etc. contract.
    transport_error.extend_with(move |_, extensions| {
        extensions.set(DOMAIN_CODE_EXTENSION, domain_code);
        extensions.set(RETRYABLE_EXTENSION, retryable);
    })
}

#[cfg(test)]
mod tests {
    use async_graphql::ErrorExtensions;

    use super::*;

    #[test]
    fn graphql_conflict_preserves_transport_and_owner_codes() {
        let error = map_port_error(PortError::conflict(
            "groups.membership_enforcement_revision_conflict",
            "stale membership revision",
        ))
        .extend();
        let extensions = error.extensions.expect("mapped GraphQL error should carry extensions");
        let json = |key: &str| {
            extensions
                .get(key)
                .cloned()
                .and_then(|value| value.into_json().ok())
        };
        assert_eq!(
            json("code").and_then(|value| value.as_str().map(str::to_owned)),
            Some("BAD_USER_INPUT".to_string())
        );
        assert_eq!(
            json(DOMAIN_CODE_EXTENSION).and_then(|value| value.as_str().map(str::to_owned)),
            Some("groups.membership_enforcement_revision_conflict".to_string())
        );
        assert_eq!(
            json(RETRYABLE_EXTENSION).and_then(|value| value.as_bool()),
            Some(false)
        );
    }

    #[test]
    fn graphql_unavailable_keeps_owner_code_and_retryability() {
        let error = map_port_error(PortError::unavailable(
            "groups.persistence_unavailable",
            "private database diagnostic",
        ))
        .extend();
        let extensions = error.extensions.expect("mapped GraphQL error should carry extensions");
        let json = |key: &str| {
            extensions
                .get(key)
                .cloned()
                .and_then(|value| value.into_json().ok())
        };
        assert_eq!(
            json("code").and_then(|value| value.as_str().map(str::to_owned)),
            Some("INTERNAL_ERROR".to_string())
        );
        assert_eq!(
            json(DOMAIN_CODE_EXTENSION).and_then(|value| value.as_str().map(str::to_owned)),
            Some("groups.persistence_unavailable".to_string())
        );
        assert_eq!(
            json(RETRYABLE_EXTENSION).and_then(|value| value.as_bool()),
            Some(true)
        );
    }
}
