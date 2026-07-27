use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{Permission, PortActor, PortActorKind, PortCallPolicy, PortContext, PortError};
use rustok_core::{UserRole, UserStatus};
use rustok_forum::{
    ForumNotificationRecipientContextPort, ForumNotificationRecipientContextRequest,
    SharedForumNotificationRecipientContextPort,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::models::_entities::users;
use crate::services::rbac_service::RbacService;

const MAX_RECIPIENT_PERMISSION_CLAIMS: usize = 512;
const RECIPIENT_UNAVAILABLE_CODE: &str = "forum.notification_recipient_context.unavailable";
const RECIPIENT_DEPENDENCY_CODE: &str =
    "forum.notification_recipient_context.dependency_unavailable";

/// Server-owned adapter for the exact Forum notification-recipient principal
/// capability. Identity and RBAC state remain owned by the host; Forum receives
/// only one validated, tenant-bound `PortContext` snapshot.
#[derive(Clone)]
pub(crate) struct ServerForumNotificationRecipientContextPort {
    db: DatabaseConnection,
}

impl ServerForumNotificationRecipientContextPort {
    pub(crate) fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub(crate) fn shared(db: DatabaseConnection) -> SharedForumNotificationRecipientContextPort {
        Arc::new(Self::new(db))
    }
}

#[async_trait]
impl ForumNotificationRecipientContextPort for ServerForumNotificationRecipientContextPort {
    async fn resolve_forum_notification_recipient_context(
        &self,
        caller_context: PortContext,
        request: ForumNotificationRecipientContextRequest,
    ) -> Result<PortContext, PortError> {
        validate_request(&caller_context, &request)?;

        let active_recipient = users::Entity::find()
            .filter(users::Column::TenantId.eq(request.tenant_id))
            .filter(users::Column::Id.eq(request.recipient_id))
            .filter(users::Column::Status.eq(UserStatus::Active))
            .one(&self.db)
            .await
            .map_err(|_| dependency_unavailable())?;
        if active_recipient.is_none() {
            return Err(recipient_unavailable());
        }

        let permissions =
            RbacService::get_user_permissions(&self.db, &request.tenant_id, &request.recipient_id)
                .await
                .map_err(|_| dependency_unavailable())?;
        let role = RbacService::get_user_role(&self.db, &request.tenant_id, &request.recipient_id)
            .await
            .map_err(|_| dependency_unavailable())?;

        build_recipient_context(&caller_context, &request, role, permissions)
    }
}

fn validate_request(
    caller_context: &PortContext,
    request: &ForumNotificationRecipientContextRequest,
) -> Result<(), PortError> {
    caller_context.require_policy(PortCallPolicy::read())?;
    if request.tenant_id.is_nil() || request.recipient_id.is_nil() {
        return Err(PortError::validation(
            "forum.notification_recipient_context.invalid_request",
            "Forum notification recipient identity must not be nil",
        ));
    }
    if caller_context.tenant_id != request.tenant_id.to_string() {
        return Err(PortError::validation(
            "forum.notification_recipient_context.tenant_mismatch",
            "Forum notification recipient tenant does not match the caller context",
        ));
    }
    if !matches!(
        caller_context.actor.kind,
        PortActorKind::System | PortActorKind::Service
    ) {
        return Err(PortError::forbidden(
            "forum.notification_recipient_context.caller_forbidden",
            "Forum notification recipient lookup requires a system or service caller",
        ));
    }
    Ok(())
}

fn build_recipient_context(
    caller_context: &PortContext,
    request: &ForumNotificationRecipientContextRequest,
    role: UserRole,
    permissions: Vec<Permission>,
) -> Result<PortContext, PortError> {
    if permissions.is_empty() {
        return Err(PortError::forbidden(
            RECIPIENT_UNAVAILABLE_CODE,
            "Forum notification recipient has no active permission snapshot",
        ));
    }
    if permissions.len() > MAX_RECIPIENT_PERMISSION_CLAIMS {
        return Err(PortError::invariant_violation(
            "forum.notification_recipient_context.permission_bound_exceeded",
            "Forum notification recipient permission snapshot exceeded its bound",
        ));
    }

    let mut recipient_context = PortContext::new(
        request.tenant_id.to_string(),
        PortActor::user(request.recipient_id.to_string()),
        caller_context.locale.clone(),
        caller_context.correlation_id.clone(),
    )
    .with_role(role.to_string());
    for permission in permissions {
        recipient_context = recipient_context.with_claim(permission.to_string());
    }

    recipient_context.causation_id = caller_context.causation_id.clone();
    recipient_context.traceparent = caller_context.traceparent.clone();
    recipient_context.deadline_ms = caller_context.deadline_ms;
    Ok(recipient_context)
}

fn recipient_unavailable() -> PortError {
    PortError::not_found(
        RECIPIENT_UNAVAILABLE_CODE,
        "Forum notification recipient is unavailable",
    )
}

fn dependency_unavailable() -> PortError {
    PortError::unavailable(
        RECIPIENT_DEPENDENCY_CODE,
        "Forum notification recipient authority is temporarily unavailable",
    )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rustok_api::{PortErrorKind, Resource};

    use super::*;

    fn caller_context(tenant_id: uuid::Uuid) -> PortContext {
        PortContext::new(
            tenant_id.to_string(),
            PortActor::service("notifications"),
            "en",
            "recipient-context-test",
        )
        .with_causation_id("source-event")
        .with_traceparent("00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01")
        .with_deadline(Duration::from_secs(3))
    }

    #[test]
    fn recipient_context_preserves_bounded_read_metadata() {
        let tenant_id = uuid::Uuid::new_v4();
        let recipient_id = uuid::Uuid::new_v4();
        let caller = caller_context(tenant_id);
        let request = ForumNotificationRecipientContextRequest::new(tenant_id, recipient_id)
            .expect("request should be valid");

        let result = build_recipient_context(
            &caller,
            &request,
            UserRole::Customer,
            vec![Permission::new(
                Resource::ForumTopics,
                rustok_api::Action::Read,
            )],
        )
        .expect("recipient context should be built");

        assert_eq!(result.tenant_id, tenant_id.to_string());
        assert_eq!(result.actor, PortActor::user(recipient_id.to_string()));
        assert_eq!(result.roles, vec!["customer"]);
        assert_eq!(result.claims, vec!["forum_topics:read"]);
        assert_eq!(result.locale, caller.locale);
        assert_eq!(result.correlation_id, caller.correlation_id);
        assert_eq!(result.causation_id, caller.causation_id);
        assert_eq!(result.traceparent, caller.traceparent);
        assert_eq!(result.deadline_ms, caller.deadline_ms);
        assert!(result.idempotency_key.is_none());
    }

    #[test]
    fn recipient_context_rejects_empty_authority() {
        let tenant_id = uuid::Uuid::new_v4();
        let request =
            ForumNotificationRecipientContextRequest::new(tenant_id, uuid::Uuid::new_v4())
                .expect("request should be valid");
        let error = build_recipient_context(
            &caller_context(tenant_id),
            &request,
            UserRole::Customer,
            Vec::new(),
        )
        .expect_err("empty authority must fail closed");

        assert_eq!(error.kind, PortErrorKind::Forbidden);
    }

    #[test]
    fn recipient_context_rejects_user_caller() {
        let tenant_id = uuid::Uuid::new_v4();
        let recipient_id = uuid::Uuid::new_v4();
        let caller = PortContext::new(
            tenant_id.to_string(),
            PortActor::user(uuid::Uuid::new_v4().to_string()),
            "en",
            "recipient-context-user-test",
        )
        .with_deadline(Duration::from_secs(3));
        let request = ForumNotificationRecipientContextRequest::new(tenant_id, recipient_id)
            .expect("request should be valid");

        let error = validate_request(&caller, &request)
            .expect_err("user caller must not resolve another recipient authority");
        assert_eq!(error.kind, PortErrorKind::Forbidden);
    }
}
