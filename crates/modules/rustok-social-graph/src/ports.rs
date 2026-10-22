use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
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

impl SocialGraphPairRequest {
    pub fn new(source_user_id: Uuid, target_user_id: Uuid) -> Result<Self, PortError> {
        let request = Self {
            source_user_id,
            target_user_id,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<(), PortError> {
        if self.source_user_id.is_nil() {
            return Err(PortError::validation(
                "social_graph.source_user_id_invalid",
                "social graph source user identifier must not be nil",
            ));
        }
        if self.target_user_id.is_nil() {
            return Err(PortError::validation(
                "social_graph.target_user_id_invalid",
                "social graph target user identifier must not be nil",
            ));
        }
        if self.source_user_id == self.target_user_id {
            return Err(PortError::validation(
                "social_graph.self_relation",
                "social graph relation cannot target the source user",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SocialGraphFollowBatchRequest {
    pub source_user_id: Uuid,
    pub target_user_ids: Vec<Uuid>,
}

impl SocialGraphFollowBatchRequest {
    pub fn new(source_user_id: Uuid, target_user_ids: Vec<Uuid>) -> Result<Self, PortError> {
        let request = Self {
            source_user_id,
            target_user_ids,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<(), PortError> {
        if self.source_user_id.is_nil() {
            return Err(PortError::validation(
                "social_graph.source_user_id_invalid",
                "social graph source user identifier must not be nil",
            ));
        }
        if self.target_user_ids.len() > MAX_SOCIAL_GRAPH_FOLLOW_TARGETS {
            return Err(PortError::validation(
                "social_graph.follow_batch_too_large",
                "social graph follow reads accept at most 100 target users",
            ));
        }
        for target_user_id in &self.target_user_ids {
            if target_user_id.is_nil() {
                return Err(PortError::validation(
                    "social_graph.target_user_id_invalid",
                    "social graph target user identifier must not be nil",
                ));
            }
            if *target_user_id == self.source_user_id {
                return Err(PortError::validation(
                    "social_graph.self_relation",
                    "social graph relation cannot target the source user",
                ));
            }
        }
        Ok(())
    }
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

impl SetSocialRelationCommand {
    pub fn new(
        source_user_id: Uuid,
        target_user_id: Uuid,
        relation_kind: SocialRelationKind,
        active: bool,
        expected_revision: Option<i64>,
    ) -> Result<Self, PortError> {
        let command = Self {
            source_user_id,
            target_user_id,
            relation_kind,
            active,
            expected_revision,
        };
        command.validate()?;
        Ok(command)
    }

    pub fn validate(&self) -> Result<(), PortError> {
        if self.source_user_id.is_nil() {
            return Err(PortError::validation(
                "social_graph.source_user_id_invalid",
                "social graph source user identifier must not be nil",
            ));
        }
        if self.target_user_id.is_nil() {
            return Err(PortError::validation(
                "social_graph.target_user_id_invalid",
                "social graph target user identifier must not be nil",
            ));
        }
        if self.source_user_id == self.target_user_id {
            return Err(PortError::validation(
                "social_graph.self_relation",
                "social graph relation cannot target the source user",
            ));
        }
        if let Some(expected_revision) = self.expected_revision
            && expected_revision <= 0
        {
            return Err(PortError::validation(
                "social_graph.expected_revision_invalid",
                "social graph expected revision must be greater than zero",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SocialGraphReceiptCleanupCommand {
    pub completed_before_unix_seconds: i64,
    pub limit: u32,
    pub dry_run: bool,
}

impl SocialGraphReceiptCleanupCommand {
    pub fn new(
        completed_before_unix_seconds: i64,
        limit: u32,
        dry_run: bool,
    ) -> Result<Self, PortError> {
        let command = Self {
            completed_before_unix_seconds,
            limit,
            dry_run,
        };
        command.validate()?;
        Ok(command)
    }

    pub fn validate(&self) -> Result<(), PortError> {
        if self.limit == 0 || self.limit > MAX_SOCIAL_GRAPH_RECEIPT_CLEANUP_BATCH {
            return Err(PortError::validation(
                "social_graph.receipt_cleanup_limit_invalid",
                "social graph receipt cleanup limit must be between 1 and 1000",
            ));
        }
        let completed_before = match DateTime::<Utc>::from_timestamp(self.completed_before_unix_seconds, 0)
        {
            Some(value) => value,
            None => {
                return Err(PortError::validation(
                    "social_graph.receipt_cleanup_cutoff_invalid",
                    "social graph receipt cleanup cutoff must be a valid Unix timestamp",
                ));
            }
        };
        if completed_before >= Utc::now() {
            return Err(PortError::validation(
                "social_graph.receipt_cleanup_cutoff_future",
                "social graph receipt cleanup cutoff must be in the past",
            ));
        }
        Ok(())
    }
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

impl SocialGraphRelationEventReplayCommand {
    pub fn new(
        after_relation_id: Option<Uuid>,
        limit: u32,
        dry_run: bool,
    ) -> Result<Self, PortError> {
        let command = Self {
            after_relation_id,
            limit,
            dry_run,
        };
        command.validate()?;
        Ok(command)
    }

    pub fn validate(&self) -> Result<(), PortError> {
        if self.limit == 0 || self.limit > MAX_SOCIAL_GRAPH_RELATION_EVENT_REPLAY_BATCH {
            return Err(PortError::validation(
                "social_graph.relation_event_replay_limit_invalid",
                "social graph relation event replay limit must be between 1 and 1000",
            ));
        }
        if let Some(after) = self.after_relation_id
            && after.is_nil()
        {
            return Err(PortError::validation(
                "social_graph.relation_event_replay_cursor_invalid",
                "social graph after_relation_id must not be nil",
            ));
        }
        Ok(())
    }
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
        command.validate()?;
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
        request.validate()?;
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
        request.validate()?;
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
        request.validate()?;
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
        request.validate()?;
        validate_source_actor(&context, request.source_user_id)?;

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
    let tenant_id = Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "social_graph.tenant_id_invalid",
            "social graph ports require a valid tenant identifier",
        )
    })?;
    if tenant_id.is_nil() {
        return Err(PortError::validation(
            "social_graph.tenant_id_invalid",
            "social graph ports require a valid tenant identifier",
        ));
    }
    Ok(tenant_id)
}

pub(crate) fn validate_source_actor(
    context: &PortContext,
    source_user_id: Uuid,
) -> Result<(), PortError> {
    if matches!(&context.actor.kind, PortActorKind::User) {
        let actor_id = Uuid::parse_str(&context.actor.id).map_err(|_| {
            PortError::validation(
                "social_graph.actor_id_invalid",
                "user actor requires a valid UUID identifier",
            )
        })?;
        if actor_id.is_nil() || actor_id != source_user_id {
            return Err(PortError::forbidden(
                "social_graph.source_actor_mismatch",
                "user actors may mutate or read only relations they own",
            ));
        }
    }
    Ok(())
}

pub(crate) fn map_owner_error(error: SocialGraphError) -> PortError {
    match error {
        SocialGraphError::InvalidTenantId => PortError::validation(
            "social_graph.tenant_id_invalid",
            "social graph tenant identifier is invalid",
        ),
        SocialGraphError::InvalidUserId => PortError::validation(
            "social_graph.user_id_invalid",
            "social graph user identifier must not be nil",
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

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::PortActor;

    #[test]
    fn set_social_relation_command_validation() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        // Valid command
        let cmd = SetSocialRelationCommand::new(
            source,
            target,
            SocialRelationKind::Follow,
            true,
            Some(1),
        )
        .expect("valid command should pass");
        assert_eq!(cmd.source_user_id, source);

        // Nil source
        assert!(
            SetSocialRelationCommand::new(
                Uuid::nil(),
                target,
                SocialRelationKind::Follow,
                true,
                None,
            )
            .is_err()
        );

        // Nil target
        assert!(
            SetSocialRelationCommand::new(
                source,
                Uuid::nil(),
                SocialRelationKind::Follow,
                true,
                None,
            )
            .is_err()
        );

        // Self relation
        assert!(
            SetSocialRelationCommand::new(
                source,
                source,
                SocialRelationKind::Follow,
                true,
                None,
            )
            .is_err()
        );

        // Non-positive expected revision
        assert!(
            SetSocialRelationCommand::new(
                source,
                target,
                SocialRelationKind::Follow,
                true,
                Some(0),
            )
            .is_err()
        );
        assert!(
            SetSocialRelationCommand::new(
                source,
                target,
                SocialRelationKind::Follow,
                true,
                Some(-1),
            )
            .is_err()
        );
    }

    #[test]
    fn pair_request_validation() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        assert!(SocialGraphPairRequest::new(source, target).is_ok());
        assert!(SocialGraphPairRequest::new(Uuid::nil(), target).is_err());
        assert!(SocialGraphPairRequest::new(source, Uuid::nil()).is_err());
        assert!(SocialGraphPairRequest::new(source, source).is_err());
    }

    #[test]
    fn follow_batch_request_validation() {
        let source = Uuid::new_v4();
        let target1 = Uuid::new_v4();
        let target2 = Uuid::new_v4();

        assert!(SocialGraphFollowBatchRequest::new(source, vec![target1, target2]).is_ok());
        assert!(SocialGraphFollowBatchRequest::new(Uuid::nil(), vec![target1]).is_err());
        assert!(SocialGraphFollowBatchRequest::new(source, vec![Uuid::nil()]).is_err());
        assert!(SocialGraphFollowBatchRequest::new(source, vec![target1, source]).is_err());

        let excessive: Vec<Uuid> = (0..=MAX_SOCIAL_GRAPH_FOLLOW_TARGETS)
            .map(|_| Uuid::new_v4())
            .collect();
        assert!(SocialGraphFollowBatchRequest::new(source, excessive).is_err());
    }

    #[test]
    fn cleanup_and_replay_command_validation() {
        // Cleanup command validation
        assert!(SocialGraphReceiptCleanupCommand::new(100, 0, false).is_err());
        assert!(SocialGraphReceiptCleanupCommand::new(100, 1001, false).is_err());
        assert!(SocialGraphReceiptCleanupCommand::new(i64::MAX, 10, false).is_err());
        assert!(SocialGraphReceiptCleanupCommand::new(1_000_000, 10, false).is_ok());

        // Replay command validation
        assert!(SocialGraphRelationEventReplayCommand::new(None, 0, false).is_err());
        assert!(SocialGraphRelationEventReplayCommand::new(None, 1001, false).is_err());
        assert!(SocialGraphRelationEventReplayCommand::new(Some(Uuid::nil()), 10, false).is_err());
        assert!(SocialGraphRelationEventReplayCommand::new(Some(Uuid::new_v4()), 10, false).is_ok());
    }

    #[test]
    fn parse_tenant_id_and_source_actor_guards() {
        let valid_tenant = Uuid::new_v4();
        let valid_user = Uuid::new_v4();

        let ctx = PortContext::new(
            valid_tenant.to_string(),
            PortActor::user(valid_user.to_string()),
            "ru",
            "corr-1",
        );
        assert_eq!(parse_tenant_id(&ctx).unwrap(), valid_tenant);
        assert!(validate_source_actor(&ctx, valid_user).is_ok());
        assert!(validate_source_actor(&ctx, Uuid::new_v4()).is_err());

        let nil_tenant_ctx = PortContext::new(
            Uuid::nil().to_string(),
            PortActor::service("test"),
            "ru",
            "corr-2",
        );
        assert!(parse_tenant_id(&nil_tenant_ctx).is_err());

        let invalid_actor_ctx = PortContext::new(
            valid_tenant.to_string(),
            PortActor::user("not-a-uuid"),
            "ru",
            "corr-3",
        );
        assert!(validate_source_actor(&invalid_actor_ctx, valid_user).is_err());
    }
}
