use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError};
use rustok_core::ModuleRuntimeExtensions;
use rustok_notifications::{
    NotificationBlockReadRuntime, NotificationMuteReadRuntime, NotificationRecipientPolicy,
    NotificationRecipientPolicyDecision, NotificationRecipientPolicyError,
    NotificationRecipientPolicyRequest, NotificationRecipientPolicyRuntime,
    NotificationRecipientSuppression, NotificationRelationPolicyRequest,
};
use rustok_profiles::{
    ProfilePrivacyDecision, ProfilePrivacyReadPort, ProfilePrivacyReadRequest,
    ProfilePrivacyRuntime, ProfileService,
};
use sea_orm::DatabaseConnection;

const RECIPIENT_POLICY_DEADLINE: Duration = Duration::from_secs(2);
const RECIPIENT_POLICY_ACTOR: &str = "notifications-recipient-policy";

#[derive(Clone)]
pub struct ServerNotificationRecipientPolicy {
    profiles: ProfilePrivacyRuntime,
    blocks: Option<NotificationBlockReadRuntime>,
    mutes: Option<NotificationMuteReadRuntime>,
}

impl ServerNotificationRecipientPolicy {
    pub fn compose(
        db: DatabaseConnection,
        extensions: &ModuleRuntimeExtensions,
    ) -> NotificationRecipientPolicyRuntime {
        let profile_port: Arc<dyn ProfilePrivacyReadPort> = Arc::new(ProfileService::new(db));
        let blocks = extensions.get::<NotificationBlockReadRuntime>().cloned();
        let mutes = extensions.get::<NotificationMuteReadRuntime>().cloned();
        let relation_ports_ready = blocks.is_some() && mutes.is_some();
        let policy = Self {
            profiles: ProfilePrivacyRuntime::new(profile_port),
            blocks,
            mutes,
        };

        NotificationRecipientPolicyRuntime::new(Arc::new(policy), relation_ports_ready)
    }

    fn port_context(request: &NotificationRecipientPolicyRequest) -> PortContext {
        PortContext::new(
            request.tenant_id.to_string(),
            PortActor::service(RECIPIENT_POLICY_ACTOR),
            "und",
            format!(
                "notification-policy:{}:{}:{}",
                request.source_slug, request.source_event_id, request.recipient_id
            ),
        )
        .with_deadline(RECIPIENT_POLICY_DEADLINE)
    }
}

#[async_trait]
impl NotificationRecipientPolicy for ServerNotificationRecipientPolicy {
    async fn evaluate(
        &self,
        request: NotificationRecipientPolicyRequest,
    ) -> Result<NotificationRecipientPolicyDecision, NotificationRecipientPolicyError> {
        let context = Self::port_context(&request);
        match self
            .profiles
            .port()
            .evaluate_profile_privacy(
                context.clone(),
                ProfilePrivacyReadRequest {
                    recipient_id: request.recipient_id,
                    actor_id: request.actor_id,
                },
            )
            .await
            .map_err(map_port_error)?
        {
            ProfilePrivacyDecision::Allow => {}
            ProfilePrivacyDecision::RecipientUnavailable => {
                return Ok(NotificationRecipientPolicyDecision::Suppress {
                    reason: NotificationRecipientSuppression::RecipientUnavailable,
                });
            }
            ProfilePrivacyDecision::Restricted => {
                return Ok(NotificationRecipientPolicyDecision::Suppress {
                    reason: NotificationRecipientSuppression::ProfileRestricted,
                });
            }
        }

        let Some(actor_id) = request.actor_id else {
            return Ok(NotificationRecipientPolicyDecision::Allow);
        };
        let relation_request = NotificationRelationPolicyRequest {
            tenant_id: request.tenant_id,
            recipient_id: request.recipient_id,
            actor_id,
            source_slug: request.source_slug,
            notification_type: request.notification_type,
        };

        let blocks = self
            .blocks
            .as_ref()
            .ok_or_else(NotificationRecipientPolicyError::retryable)?;
        if blocks
            .port()
            .blocks_notification(context.clone(), relation_request.clone())
            .await
            .map_err(map_port_error)?
        {
            return Ok(NotificationRecipientPolicyDecision::Suppress {
                reason: NotificationRecipientSuppression::Blocked,
            });
        }

        let mutes = self
            .mutes
            .as_ref()
            .ok_or_else(NotificationRecipientPolicyError::retryable)?;
        if mutes
            .port()
            .mutes_notification(context, relation_request)
            .await
            .map_err(map_port_error)?
        {
            return Ok(NotificationRecipientPolicyDecision::Suppress {
                reason: NotificationRecipientSuppression::Muted,
            });
        }

        Ok(NotificationRecipientPolicyDecision::Allow)
    }
}

fn map_port_error(error: PortError) -> NotificationRecipientPolicyError {
    if error.retryable {
        NotificationRecipientPolicyError::retryable()
    } else {
        NotificationRecipientPolicyError::permanent()
    }
}
