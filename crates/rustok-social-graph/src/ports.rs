use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError, PortErrorKind};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::relation;
use crate::error::SocialGraphError;
use crate::model::SocialRelationKind;
use crate::observability::{SocialGraphCommandOperation, SocialGraphCommandTimer};
use crate::receipts::SocialGraphCommandReceiptRequest;
use crate::service::SocialGraphService;

pub const MAX_SOCIAL_GRAPH_FOLLOW_TARGETS: usize = 100;
pub const MAX_SOCIAL_GRAPH_RECEIPT_CLEANUP_BATCH: u32 = 1_000;
pub const MAX_SOCIAL_GRAPH_RELATION_EVENT_REPLAY_BATCH: u32 = 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SocialGraphPairRequest {
    pub source_user_id: Uuid,
    pub target_user_id: Uuid,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SocialGraphFollowBatchRequest {
    pub source_user_id: Uuid,
    pub target_user_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SocialGraphFollowBatchResult {
    pub followed_target_user_ids: Vec<Uuid>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SetSocialRelationCommand {
    pub source_user_id: Uuid,
    pub target_user_id: Uuid,
    pub relation_kind: SocialRelationKind,
    pub active: bool,
    pub expected_revision: Option<i64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SocialGraphReceiptCleanupCommand {
    pub completed_before_unix_seconds: i64,
    pub limit: u32,
    pub dry_run: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SocialGraphReceiptCleanupResult {
    pub matched_receipts: u64,
    pub deleted_receipts: u64,
    pub oldest_retained_completed_at_unix_seconds: Option<i64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SocialGraphRelationEventReplayCommand {
    pub after_relation_id: Option<Uuid>,
    pub limit: u32,
    pub dry_run: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SocialGraphRelationEventReplayResult {
    pub selected_relations: u64,
    pub published_events: u64,
    pub next_after_relation_id: Option<Uuid>,
}

#[async_trait]
pub trait SocialGraphCommandPort: Send + Sync {
    async fn set_relation(
        &self,
        context: PortContext,
        command: SetSocialRelationCommand,
    ) -> Result<relation::Model, PortError>;
}

#[async_trait]
pub trait SocialGraphReceiptMaintenancePort: Send + Sync {
    async fn cleanup_completed_receipts(
        &self,
        context: PortContext,
        command: SocialGraphReceiptCleanupCommand,
    ) -> Result<SocialGraphReceiptCleanupResult, PortError>;
}

#[async_trait]
pub trait SocialGraphRelationEventMaintenancePort: Send + Sync {
    async fn replay_relation_state_events(
        &self,
        context: PortContext,
        command: SocialGraphRelationEventReplayCommand,
    ) -> Result<SocialGraphRelationEventReplayResult, PortError>;
}

#[async_trait]
pub trait SocialGraphPrivacyReadPort: Send + Sync {
    async fn blocks_between(
        &self,
        context: PortContext,
        request: SocialGraphPairRequest,
    ) -> Result<bool, PortError>;

    async fn source_mutes_target(
        &self,
        context: PortContext,
        request: SocialGraphPairRequest,
    ) -> Result<bool, PortError>;

    async fn source_follows_target(
        &self,
        context: PortContext,
        request: SocialGraphPairRequest,
    ) -> Result<bool, PortError>;

    async fn source_follows_targets(
        &self,
        context: PortContext,
        request: SocialGraphFollowBatchRequest,
    ) -> Result<SocialGraphFollowBatchResult, PortError>;
}

#[derive(Clone)]
pub struct SocialGraphPrivacyRuntime {
    port: Arc<dyn SocialGraphPrivacyReadPort>,
}

impl SocialGraphPrivacyRuntime {
    pub fn new(port: Arc<dyn SocialGraphPrivacyReadPort>) -> Self {
        Self { port }
    }

    pub fn port(&self) -> &dyn SocialGraphPrivacyReadPort {
        self.port.as_ref()
    }
}

#[async_trait]
impl SocialGraphCommandPort for SocialGraphService {
    async fn set_relation(
        &self,
        context: PortContext,
        command: SetSocialRelationCommand,
    ) -> Result<relation::Model, PortError> {
        let tenant_id = parse_tenant_id(&context)?;
        let timer = SocialGraphCommandTimer::start(
            SocialGraphCommandOperation::from_relation_state(command.relation_kind, command.active),
            tenant_id,
            command.source_user_id,
            command.target_user_id,
        );

        if let Err(error) = context.require_policy(PortCallPolicy::write()) {
            timer.finish_failure(&error.code, error.retryable);
            return Err(error);
        }
        if let Err(error) = validate_source_actor(&context, command.source_user_id) {
            timer.finish_failure(&error.code, error.retryable);
            return Err(error);
        }
        let idempotency_key = context.idempotency_key.clone().unwrap_or_default();
        let actor_id = Uuid::parse_str(&context.actor.id).ok();

        let result = self
            .set_relation_state_with_receipt(
                tenant_id,
                actor_id,
                SocialGraphCommandReceiptRequest {
                    source_user_id: command.source_user_id,
                    target_user_id: command.target_user_id,
                    relation_kind: command.relation_kind,
                    active: command.active,
                    expected_revision: command.expected_revision,
                },
                idempotency_key,
            )
            .await
            .map_err(map_owner_error);
        timer.finish_port_result(&result);
        result
    }
}

#[async_trait]
impl SocialGraphPrivacyReadPort for SocialGraphService {
    async fn blocks_between(
        &self,
        context: PortContext,
        request: SocialGraphPairRequest,
    ) -> Result<bool, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        SocialGraphService::blocks_between(
            self,
            parse_tenant_id(&context)?,
            request.source_user_id,
            request.target_user_id,
        )
        .await
        .map_err(map_owner_error)
    }

    async fn source_mutes_target(
        &self,
        context: PortContext,
        request: SocialGraphPairRequest,
    ) -> Result<bool, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        SocialGraphService::source_mutes_target(
            self,
            parse_tenant_id(&context)?,
            request.source_user_id,
            request.target_user_id,
        )
        .await
        .map_err(map_owner_error)
    }

    async fn source_follows_target(
        &self,
        context: PortContext,
        request: SocialGraphPairRequest,
    ) -> Result<bool, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        validate_source_actor(&context, request.source_user_id)?;
        SocialGraphService::source_follows_target(
            self,
            parse_tenant_id(&context)?,
            request.source_user_id,
            request.target_user_id,
        )
        .await
        .map_err(map_owner_error)
    }

    async fn source_follows_targets(
        &self,
        context: PortContext,
        request: SocialGraphFollowBatchRequest,
    ) -> Result<SocialGraphFollowBatchResult, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        validate_source_actor(&context, request.source_user_id)?;
        if request.target_user_ids.len() > MAX_SOCIAL_GRAPH_FOLLOW_TARGETS {
            return Err(PortError::validation(
                "social_graph.follow_batch_too_large",
                "social graph follow reads accept at most 100 target users",
            ));
        }

        let followed_target_user_ids = SocialGraphService::source_follows_targets(
            self,
            parse_tenant_id(&context)?,
            request.source_user_id,
            &request.target_user_ids,
        )
        .await
        .map_err(map_owner_error)?;

        Ok(SocialGraphFollowBatchResult {
            followed_target_user_ids,
        })
    }
}

pub(crate) fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "social_graph.tenant_id_invalid",
            "social graph ports require a valid tenant identifier",
        )
    })
}

fn validate_source_actor(context: &PortContext, source_user_id: Uuid) -> Result<(), PortError> {
    if matches!(&context.actor.kind, PortActorKind::User)
        && Uuid::parse_str(&context.actor.id).ok() != Some(source_user_id)
    {
        return Err(PortError::forbidden(
            "social_graph.source_actor_mismatch",
            "user actors may mutate or read only relations they own",
        ));
    }
    Ok(())
}

pub(crate) fn map_owner_error(error: SocialGraphError) -> PortError {
    match error {
        SocialGraphError::InvalidTenantId => PortError::validation(
            "social_graph.tenant_id_invalid",
            "social graph tenant identifier is invalid",
        ),
        SocialGraphError::SelfRelation => PortError::validation(
            "social_graph.self_relation",
            "social graph relation cannot target the source user",
        ),
        SocialGraphError::RevisionConflict => PortError::conflict(
            "social_graph.revision_conflict",
            "social graph relation revision changed before the command was applied",
        ),
        SocialGraphError::SourceActorMismatch => PortError::forbidden(
            "social_graph.source_actor_mismatch",
            "social graph command actor does not own the relation source",
        ),
        SocialGraphError::IdempotencyKeyInvalid => PortError::validation(
            "social_graph.idempotency_key_invalid",
            "social graph idempotency key must contain 1 to 191 bytes",
        ),
        SocialGraphError::IdempotencyConflict => PortError::conflict(
            "social_graph.idempotency_conflict",
            "social graph idempotency key is already bound to another command",
        ),
        SocialGraphError::CommandReceiptCorrupt => PortError::invariant_violation(
            "social_graph.command_receipt_corrupt",
            "social graph command receipt requires operator review",
        ),
        SocialGraphError::EventPublicationUnavailable => PortError::unavailable(
            "social_graph.event_publication_unavailable",
            "social graph relation event could not be persisted transactionally",
        ),
        SocialGraphError::Database(_) => PortError::new(
            PortErrorKind::Unavailable,
            "social_graph.storage_unavailable",
            "social graph storage is temporarily unavailable",
            true,
        ),
    }
}
