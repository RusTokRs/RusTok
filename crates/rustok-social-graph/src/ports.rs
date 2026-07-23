use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{
    PortActorKind, PortCallPolicy, PortContext, PortError, PortErrorKind,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::relation;
use crate::error::SocialGraphError;
use crate::model::SocialRelationKind;
use crate::service::SocialGraphService;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SocialGraphPairRequest {
    pub source_user_id: Uuid,
    pub target_user_id: Uuid,
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
        context.require_policy(PortCallPolicy::write())?;
        validate_source_actor(&context, command.source_user_id)?;
        let tenant_id = parse_tenant_id(&context)?;
        self.set_relation_state(
            tenant_id,
            command.source_user_id,
            command.target_user_id,
            command.relation_kind,
            command.active,
            command.expected_revision,
        )
        .await
        .map_err(map_owner_error)
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
        self.blocks_between(
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
        self.source_mutes_target(
            parse_tenant_id(&context)?,
            request.source_user_id,
            request.target_user_id,
        )
        .await
        .map_err(map_owner_error)
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
            "user actors may mutate only relations they own",
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
