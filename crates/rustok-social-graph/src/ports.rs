use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError, PortErrorKind};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::relation;
use crate::error::SocialGraphError;
use crate::model::SocialRelationKind;
use crate::observability::{SocialGraphCommandOperation, SocialGraphCommandTimer};
use crate::service::SocialGraphService;

pub const MAX_SOCIAL_GRAPH_FOLLOW_TARGETS: usize = 100;

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

#[async_trait]
pub trait SocialGraphCommandPort: Send + Sync {
    async fn set_relation(
        &self,
        context: PortContext,
        command: SetSocialRelationCommand,
    ) -> Result<relation::Model, PortError>;
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

        let result = self
            .set_relation_state(
                tenant_id,
                command.source_user_id,
                command.target_user_id,
                command.relation_kind,
                command.active,
                command.expected_revision,
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

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
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

fn map_owner_error(error: SocialGraphError) -> PortError {
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
        SocialGraphError::Database(_) => PortError::new(
            PortErrorKind::Unavailable,
            "social_graph.storage_unavailable",
            "social graph storage is temporarily unavailable",
            true,
        ),
    }
}
