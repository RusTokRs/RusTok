use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError};
use rustok_core::SecurityContext;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::{ForumError, ForumResult};
use crate::services::ForumTopicAudienceViewer;

pub const FORUM_NOTIFICATION_RECIPIENT_CONTEXT_CAPABILITY: &str =
    "forum_notification_recipient_context";
pub const FORUM_NOTIFICATION_RECIPIENT_CONTEXT_CAPABILITY_UNAVAILABLE: &str =
    "FORUM_NOTIFICATION_RECIPIENT_CONTEXT_CAPABILITY_UNAVAILABLE";

/// Exact tenant-bound recipient identity requested by an asynchronous Forum
/// notification consumer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumNotificationRecipientContextRequest {
    pub tenant_id: Uuid,
    pub recipient_id: Uuid,
}

impl ForumNotificationRecipientContextRequest {
    pub fn new(tenant_id: Uuid, recipient_id: Uuid) -> ForumResult<Self> {
        if tenant_id.is_nil() {
            return Err(ForumError::Validation(
                "Forum notification recipient tenant must not be nil".to_string(),
            ));
        }
        if recipient_id.is_nil() {
            return Err(ForumError::Validation(
                "Forum notification recipient user must not be nil".to_string(),
            ));
        }
        Ok(Self {
            tenant_id,
            recipient_id,
        })
    }
}

/// Host-owned lookup for the exact recipient principal snapshot used by a
/// deferred notification operation.
///
/// The host returns a normal `PortContext`; Forum validates its tenant, actor,
/// deadline, role and permission claims before using it. Implementations must
/// not return a broader system or service principal for a user recipient.
#[async_trait]
pub trait ForumNotificationRecipientContextPort: Send + Sync {
    async fn resolve_forum_notification_recipient_context(
        &self,
        context: PortContext,
        request: ForumNotificationRecipientContextRequest,
    ) -> Result<PortContext, PortError>;
}

pub type SharedForumNotificationRecipientContextPort =
    Arc<dyn ForumNotificationRecipientContextPort>;

/// Validated recipient authority plus the same bounded context that may be
/// forwarded to trust, channel or group facts owners.
#[derive(Clone, Debug)]
pub struct ForumNotificationRecipientContext {
    pub security: SecurityContext,
    pub port_context: PortContext,
}

impl ForumNotificationRecipientContext {
    pub fn into_topic_viewer(self) -> ForumResult<ForumTopicAudienceViewer> {
        ForumTopicAudienceViewer::authenticated(self.security, self.port_context)
    }
}

/// Fail-closed recipient context composition for asynchronous notification
/// consumers. Provider absence is a typed capability gap; provider failures
/// preserve retryability through `ForumError::CapabilityFailure`.
#[derive(Clone, Default)]
pub struct ForumNotificationRecipientContextResolver {
    port: Option<SharedForumNotificationRecipientContextPort>,
}

impl ForumNotificationRecipientContextResolver {
    pub fn new(port: Option<SharedForumNotificationRecipientContextPort>) -> Self {
        Self { port }
    }

    pub async fn resolve(
        &self,
        caller_context: PortContext,
        tenant_id: Uuid,
        recipient_id: Uuid,
    ) -> ForumResult<ForumNotificationRecipientContext> {
        let request = ForumNotificationRecipientContextRequest::new(tenant_id, recipient_id)?;
        validate_caller_context(&caller_context, tenant_id)?;

        let Some(port) = &self.port else {
            return Err(ForumError::capability_unavailable(
                FORUM_NOTIFICATION_RECIPIENT_CONTEXT_CAPABILITY,
                FORUM_NOTIFICATION_RECIPIENT_CONTEXT_CAPABILITY_UNAVAILABLE,
            ));
        };

        let recipient_context = port
            .resolve_forum_notification_recipient_context(caller_context, request)
            .await
            .map_err(map_recipient_context_port_error)?;
        validate_recipient_context(&recipient_context, tenant_id, recipient_id)?;

        let security = SecurityContext::try_from_port_context(&recipient_context)
            .map_err(map_recipient_context_port_error)?;
        if security.user_id != Some(recipient_id) {
            return Err(ForumError::Validation(
                "Forum notification recipient authority does not match the requested user"
                    .to_string(),
            ));
        }

        Ok(ForumNotificationRecipientContext {
            security,
            port_context: recipient_context,
        })
    }
}

fn validate_caller_context(context: &PortContext, tenant_id: Uuid) -> ForumResult<()> {
    context
        .require_policy(PortCallPolicy::read())
        .map_err(map_recipient_context_port_error)?;
    validate_context_tenant(context, tenant_id, "caller")?;
    if !matches!(
        context.actor.kind,
        PortActorKind::System | PortActorKind::Service
    ) {
        return Err(ForumError::Validation(
            "Forum notification recipient lookup requires a system or service caller".to_string(),
        ));
    }
    Ok(())
}

fn validate_recipient_context(
    context: &PortContext,
    tenant_id: Uuid,
    recipient_id: Uuid,
) -> ForumResult<()> {
    context
        .require_policy(PortCallPolicy::read())
        .map_err(map_recipient_context_port_error)?;
    validate_context_tenant(context, tenant_id, "recipient")?;
    if context.actor.kind != PortActorKind::User {
        return Err(ForumError::Validation(
            "Forum notification recipient context requires a user actor".to_string(),
        ));
    }
    let actor_id = Uuid::parse_str(&context.actor.id).map_err(|_| {
        ForumError::Validation("Forum notification recipient context actor is invalid".to_string())
    })?;
    if actor_id != recipient_id {
        return Err(ForumError::Validation(
            "Forum notification recipient context actor does not match the requested user"
                .to_string(),
        ));
    }
    Ok(())
}

fn validate_context_tenant(context: &PortContext, tenant_id: Uuid, label: &str) -> ForumResult<()> {
    let context_tenant_id = Uuid::parse_str(&context.tenant_id).map_err(|_| {
        ForumError::Validation(format!(
            "Forum notification recipient {label} context tenant is invalid"
        ))
    })?;
    if context_tenant_id != tenant_id {
        return Err(ForumError::Validation(format!(
            "Forum notification recipient {label} context tenant does not match the request"
        )));
    }
    Ok(())
}

fn map_recipient_context_port_error(error: PortError) -> ForumError {
    ForumError::capability_failure(
        FORUM_NOTIFICATION_RECIPIENT_CONTEXT_CAPABILITY,
        error.code,
        error.message,
        error.retryable,
    )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rustok_api::{Permission, PortActor};
    use rustok_core::UserRole;

    use super::*;

    #[derive(Clone)]
    struct StaticRecipientContextPort {
        context: PortContext,
    }

    #[async_trait]
    impl ForumNotificationRecipientContextPort for StaticRecipientContextPort {
        async fn resolve_forum_notification_recipient_context(
            &self,
            _context: PortContext,
            _request: ForumNotificationRecipientContextRequest,
        ) -> Result<PortContext, PortError> {
            Ok(self.context.clone())
        }
    }

    fn caller_context(tenant_id: Uuid) -> PortContext {
        PortContext::new(
            tenant_id.to_string(),
            PortActor::system(),
            "en",
            "forum-notification-recipient-test",
        )
        .with_deadline(Duration::from_secs(5))
    }

    fn recipient_context(tenant_id: Uuid, recipient_id: Uuid) -> PortContext {
        PortContext::new(
            tenant_id.to_string(),
            PortActor::user(recipient_id.to_string()),
            "en",
            "forum-notification-recipient-result",
        )
        .with_claim(Permission::FORUM_TOPICS_READ.to_string())
        .with_role("customer")
        .with_deadline(Duration::from_secs(5))
    }

    #[tokio::test]
    async fn recipient_context_resolver_builds_exact_topic_viewer() {
        let tenant_id = Uuid::new_v4();
        let recipient_id = Uuid::new_v4();
        let resolver = ForumNotificationRecipientContextResolver::new(Some(Arc::new(
            StaticRecipientContextPort {
                context: recipient_context(tenant_id, recipient_id),
            },
        )));

        let resolved = resolver
            .resolve(caller_context(tenant_id), tenant_id, recipient_id)
            .await
            .expect("exact recipient context should resolve");
        assert_eq!(resolved.security.user_id, Some(recipient_id));
        assert_eq!(resolved.security.role, UserRole::Customer);
        assert!(
            resolved
                .into_topic_viewer()
                .expect("recipient authority should build an authenticated topic viewer")
                .is_authenticated()
        );
    }

    #[tokio::test]
    async fn recipient_context_resolver_rejects_foreign_actor() {
        let tenant_id = Uuid::new_v4();
        let recipient_id = Uuid::new_v4();
        let resolver = ForumNotificationRecipientContextResolver::new(Some(Arc::new(
            StaticRecipientContextPort {
                context: recipient_context(tenant_id, Uuid::new_v4()),
            },
        )));

        let error = resolver
            .resolve(caller_context(tenant_id), tenant_id, recipient_id)
            .await
            .expect_err("foreign recipient context must fail closed");
        assert_eq!(error.stable_code(), "FORUM_VALIDATION_FAILED");
    }

    #[tokio::test]
    async fn recipient_context_resolver_reports_missing_capability() {
        let tenant_id = Uuid::new_v4();
        let recipient_id = Uuid::new_v4();
        let error = ForumNotificationRecipientContextResolver::default()
            .resolve(caller_context(tenant_id), tenant_id, recipient_id)
            .await
            .expect_err("missing recipient context provider must be explicit");
        assert_eq!(
            error.stable_code(),
            FORUM_NOTIFICATION_RECIPIENT_CONTEXT_CAPABILITY_UNAVAILABLE
        );
    }
}
