use async_trait::async_trait;
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError, PortErrorKind};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{SocialGraphError, SocialGraphPairRequest, SocialGraphService, SocialRelationKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SocialGraphFollowState {
    pub target_user_id: Uuid,
    pub following: bool,
    pub revision: Option<i64>,
}

#[async_trait]
pub trait SocialGraphFollowReadPort: Send + Sync {
    async fn source_follow_state(
        &self,
        context: PortContext,
        request: SocialGraphPairRequest,
    ) -> Result<SocialGraphFollowState, PortError>;
}

#[async_trait]
impl SocialGraphFollowReadPort for SocialGraphService {
    async fn source_follow_state(
        &self,
        context: PortContext,
        request: SocialGraphPairRequest,
    ) -> Result<SocialGraphFollowState, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        validate_source_actor(&context, request.source_user_id)?;
        if request.source_user_id == request.target_user_id {
            return Err(PortError::validation(
                "social_graph.self_relation",
                "social graph relation cannot target the source user",
            ));
        }

        let relation = self
            .relation_state(
                parse_tenant_id(&context)?,
                request.source_user_id,
                request.target_user_id,
                SocialRelationKind::Follow,
            )
            .await
            .map_err(map_owner_error)?;

        Ok(SocialGraphFollowState {
            target_user_id: request.target_user_id,
            following: relation.as_ref().is_some_and(|relation| relation.active),
            revision: relation.map(|relation| relation.revision),
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
