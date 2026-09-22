use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError};
use rustok_core::ModuleRuntimeExtensions;
#[cfg(feature = "mod-social_graph")]
use rustok_index::SharedIndexQueryRuntime;
use rustok_notifications::{
    NotificationBlockReadPort, NotificationBlockReadRuntime, NotificationMuteReadPort,
    NotificationMuteReadRuntime, NotificationRecipientPolicy, NotificationRecipientPolicyDecision,
    NotificationRecipientPolicyError, NotificationRecipientPolicyRequest,
    NotificationRecipientPolicyRuntime, NotificationRecipientSuppression,
    NotificationRelationPolicyRequest,
};
use rustok_profiles::{
    ProfilePrivacyDecision, ProfilePrivacyReadPort, ProfilePrivacyReadRequest,
    ProfilePrivacyRuntime, ProfilePrivacyService,
};
#[cfg(feature = "mod-social_graph")]
use rustok_social_graph::{
    IndexPrivacyShadowFailureCode, IndexPrivacyShadowObservation, IndexPrivacyShadowObserver,
    IndexPrivacyShadowOperation, IndexPrivacyShadowOutcome, IndexShadowSocialGraphPrivacyReadPort,
};
use rustok_social_graph::{
    SocialGraphPairRequest, SocialGraphPrivacyReadPort, SocialGraphPrivacyRuntime,
    SocialGraphService,
};
#[cfg(feature = "mod-social_graph")]
use rustok_telemetry::social_graph_index_privacy_shadow_metrics::{
    SocialGraphIndexPrivacyShadowOperation as MetricOperation,
    SocialGraphIndexPrivacyShadowOutcome as MetricOutcome, record_failure, record_observation,
};
use sea_orm::DatabaseConnection;

pub const NOTIFICATION_CANDIDATE_WORKER_ENABLED_ENV: &str =
    "RUSTOK_NOTIFICATIONS_CANDIDATE_WORKER_ENABLED";
#[cfg(feature = "mod-social_graph")]
pub const SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_ENABLED_ENV: &str =
    "RUSTOK_SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_ENABLED";
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

#[cfg(feature = "mod-social_graph")]
struct TelemetryIndexPrivacyShadowObserver;

#[cfg(feature = "mod-social_graph")]
impl IndexPrivacyShadowObserver for TelemetryIndexPrivacyShadowObserver {
    fn observe(&self, observation: IndexPrivacyShadowObservation) {
        let operation = metric_operation(observation.operation);
        match (
            observation.outcome,
            observation.failure_code,
            observation.retryable,
        ) {
            (IndexPrivacyShadowOutcome::Error, Some(code), Some(retryable)) => record_failure(
                operation,
                code.as_str(),
                retryable,
                observation.comparison_duration,
            ),
            (IndexPrivacyShadowOutcome::Error, _, _) => record_failure(
                operation,
                IndexPrivacyShadowFailureCode::Other.as_str(),
                false,
                observation.comparison_duration,
            ),
            (outcome, _, _) => record_observation(
                operation,
                metric_outcome(outcome),
                observation.comparison_duration,
            ),
        }
    }
}

#[cfg(feature = "mod-social_graph")]
fn metric_operation(operation: IndexPrivacyShadowOperation) -> MetricOperation {
    match operation {
        IndexPrivacyShadowOperation::BlocksBetween => MetricOperation::BlocksBetween,
        IndexPrivacyShadowOperation::SourceMutesTarget => MetricOperation::SourceMutesTarget,
        IndexPrivacyShadowOperation::SourceFollowsTarget => MetricOperation::SourceFollowsTarget,
        IndexPrivacyShadowOperation::SourceFollowsTargets => MetricOperation::SourceFollowsTargets,
    }
}

#[cfg(feature = "mod-social_graph")]
fn metric_outcome(outcome: IndexPrivacyShadowOutcome) -> MetricOutcome {
    match outcome {
        IndexPrivacyShadowOutcome::MatchPositive => MetricOutcome::MatchPositive,
        IndexPrivacyShadowOutcome::MatchNegative => MetricOutcome::MatchNegative,
        IndexPrivacyShadowOutcome::FalseNegative => MetricOutcome::FalseNegative,
        IndexPrivacyShadowOutcome::FalsePositive => MetricOutcome::FalsePositive,
        IndexPrivacyShadowOutcome::MatchBatchEmpty => MetricOutcome::MatchBatchEmpty,
        IndexPrivacyShadowOutcome::MatchBatchNonempty => MetricOutcome::MatchBatchNonempty,
        IndexPrivacyShadowOutcome::BatchMissing => MetricOutcome::BatchMissing,
        IndexPrivacyShadowOutcome::BatchExtra => MetricOutcome::BatchExtra,
        IndexPrivacyShadowOutcome::BatchMixed => MetricOutcome::BatchMixed,
        IndexPrivacyShadowOutcome::Error => MetricOutcome::Error,
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
        let graph_port: Arc<dyn SocialGraphPrivacyReadPort> =
            Arc::new(SocialGraphService::new(db.clone()));
        Self::compose_with_graph(db, extensions, SocialGraphPrivacyRuntime::new(graph_port))
    }

    #[cfg(feature = "mod-social_graph")]
    pub fn compose_with_index_shadow_runtime(
        db: DatabaseConnection,
        extensions: &ModuleRuntimeExtensions,
        runtime: SharedIndexQueryRuntime,
    ) -> NotificationRecipientPolicyRuntime {
        let authoritative: Arc<dyn SocialGraphPrivacyReadPort> =
            Arc::new(SocialGraphService::new(db.clone()));
        let shadow: Arc<dyn SocialGraphPrivacyReadPort> =
            Arc::new(IndexShadowSocialGraphPrivacyReadPort::with_observer(
                authoritative,
                runtime,
                Arc::new(TelemetryIndexPrivacyShadowObserver),
            ));
        Self::compose_with_graph(db, extensions, SocialGraphPrivacyRuntime::new(shadow))
    }

    fn compose_with_graph(
        db: DatabaseConnection,
        extensions: &ModuleRuntimeExtensions,
        graph: SocialGraphPrivacyRuntime,
    ) -> NotificationRecipientPolicyRuntime {
        let profile_port: Arc<dyn ProfilePrivacyReadPort> =
            Arc::new(ProfilePrivacyService::new(db));
        let blocks = extensions
            .get::<NotificationBlockReadRuntime>()
            .cloned()
            .unwrap_or_else(|| {
                NotificationBlockReadRuntime::new(Arc::new(SocialGraphNotificationBlockAdapter {
                    graph: graph.clone(),
                }))
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

#[cfg(feature = "mod-social_graph")]
pub(crate) fn social_graph_index_privacy_shadow_enabled() -> Result<bool, String> {
    match std::env::var(SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_ENABLED_ENV) {
        Ok(value) => parse_bool(SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_ENABLED_ENV, &value),
        Err(std::env::VarError::NotPresent) => Ok(false),
        Err(error) => Err(format!(
            "failed to read {SOCIAL_GRAPH_INDEX_PRIVACY_SHADOW_ENABLED_ENV}: {error}"
        )),
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

#[cfg(feature = "mod-social_graph")]
fn parse_bool(variable: &str, value: &str) -> Result<bool, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "" | "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(format!(
            "{variable} must be one of true/false, 1/0, yes/no, or on/off"
        )),
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
