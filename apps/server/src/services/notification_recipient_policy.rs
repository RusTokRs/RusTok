use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError};
use rustok_core::ModuleRuntimeExtensions;
use rustok_notifications::{
    NotificationBlockReadPort, NotificationBlockReadRuntime, NotificationMuteReadPort,
    NotificationMuteReadRuntime, NotificationRecipientPolicy, NotificationRecipientPolicyDecision,
    NotificationRecipientPolicyError, NotificationRecipientPolicyRequest,
    NotificationRecipientPolicyRuntime, NotificationRecipientSuppression,
    NotificationRelationPolicyRequest,
};
use rustok_profiles::{
    ProfilePrivacyDecision, ProfilePrivacyReadPort, ProfilePrivacyReadRequest,
    ProfilePrivacyRuntime, ProfileService,
};
use rustok_social_graph::{
    SocialGraphPairRequest, SocialGraphPrivacyReadPort, SocialGraphPrivacyRuntime,
    SocialGraphService,
};
use sea_orm::DatabaseConnection;

pub const NOTIFICATION_CANDIDATE_WORKER_ENABLED_ENV: &str =
    "RUSTOK_NOTIFICATIONS_CANDIDATE_WORKER_ENABLED";
const RECIPIENT_POLICY_DEADLINE: Duration = Duration::from_secs(2);
const RECIPIENT_POLICY_ACTOR: &str = "notifications-recipient-policy";

#[derive(Clone)]
struct SocialGraphNotificationBlockAdapter {
    graph: SocialGraphPrivacyRuntime,
}

#[async_trait]
impl NotificationBlockReadPort for SocialGraphNotificationBlockAdapter {
    async fn blocks_notification(
        &self,
        context: PortContext,
        request: NotificationRelationPolicyRequest,
    ) -> Result<bool, PortError> {
        require_matching_tenant(&context, request.tenant_id)?;
        self.graph
            .port()
            .blocks_between(
                context,
                SocialGraphPairRequest {
                    source_user_id: request.recipient_id,
                    target_user_id: request.actor_id,
                },
            )
            .await
    }
}

#[derive(Clone)]
struct SocialGraphNotificationMuteAdapter {
    graph: SocialGraphPrivacyRuntime,
}

#[async_trait]
impl NotificationMuteReadPort for SocialGraphNotificationMuteAdapter {
    async fn mutes_notification(
        &self,
        context: PortContext,
        request: NotificationRelationPolicyRequest,
    ) -> Result<bool, PortError> {
        require_matching_tenant(&context, request.tenant_id)?;
        self.graph
            .port()
            .source_mutes_target(
                context,
                SocialGraphPairRequest {
                    source_user_id: request.recipient_id,
                    target_user_id: request.actor_id,
                },
            )
            .await
    }
}

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
        let profile_port: Arc<dyn ProfilePrivacyReadPort> =
            Arc::new(ProfileService::new(db.clone()));
        let graph_port: Arc<dyn SocialGraphPrivacyReadPort> =
            Arc::new(SocialGraphService::new(db));
        let graph = SocialGraphPrivacyRuntime::new(graph_port);
        let blocks = extensions
            .get::<NotificationBlockReadRuntime>()
            .cloned()
            .unwrap_or_else(|| {
                NotificationBlockReadRuntime::new(Arc::new(
                    SocialGraphNotificationBlockAdapter {
                        graph: graph.clone(),
                    },
                ))
            });
        let mutes = extensions
            .get::<NotificationMuteReadRuntime>()
            .cloned()
            .unwrap_or_else(|| {
                NotificationMuteReadRuntime::new(Arc::new(SocialGraphNotificationMuteAdapter {
                    graph,
                }))
            });
        let policy = Self {
            profiles: ProfilePrivacyRuntime::new(profile_port),
            blocks: Some(blocks),
            mutes: Some(mutes),
        };

        NotificationRecipientPolicyRuntime::new(Arc::new(policy), true)
            .with_candidate_worker_enabled(candidate_worker_enabled_from_environment())
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

fn candidate_worker_enabled_from_environment() -> bool {
    match std::env::var(NOTIFICATION_CANDIDATE_WORKER_ENABLED_ENV) {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "" | "0" | "false" | "no" | "off" => false,
            _ => {
                tracing::warn!(
                    variable = NOTIFICATION_CANDIDATE_WORKER_ENABLED_ENV,
                    value,
                    "Invalid notification candidate worker enable flag; keeping worker disabled"
                );
                false
            }
        },
        Err(std::env::VarError::NotPresent) => false,
        Err(error) => {
            tracing::warn!(
                variable = NOTIFICATION_CANDIDATE_WORKER_ENABLED_ENV,
                error = %error,
                "Notification candidate worker enable flag is unreadable; keeping worker disabled"
            );
            false
        }
    }
}

fn require_matching_tenant(context: &PortContext, tenant_id: uuid::Uuid) -> Result<(), PortError> {
    if context.tenant_id != tenant_id.to_string() {
        return Err(PortError::validation(
            "notifications.relation_tenant_mismatch",
            "notification relation policy tenant does not match port context",
        ));
    }
    Ok(())
}

fn map_port_error(error: PortError) -> NotificationRecipientPolicyError {
    if error.retryable {
        NotificationRecipientPolicyError::retryable()
    } else {
        NotificationRecipientPolicyError::permanent()
    }
}
