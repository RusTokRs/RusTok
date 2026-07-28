use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError};
use sea_orm::{DatabaseConnection, EntityTrait};
use uuid::Uuid;

use crate::audience::{
    ForumAudienceFacts, ForumAudienceFactsPort, ForumAudienceFactsRequest,
    SharedForumAudienceFactsPort,
};
use crate::entities::forum_user_trust_state;

use super::user_trust::MAX_FORUM_USER_TRUST_LEVEL;

const INVALID_REQUEST_CODE: &str = "forum.user_trust_facts.invalid_request";
const UNSUPPORTED_REQUEST_CODE: &str = "forum.user_trust_facts.unsupported_request";
const TENANT_MISMATCH_CODE: &str = "forum.user_trust_facts.tenant_mismatch";
const ACTOR_MISMATCH_CODE: &str = "forum.user_trust_facts.actor_mismatch";
const STORAGE_UNAVAILABLE_CODE: &str = "forum.user_trust_facts.storage_unavailable";
const STORAGE_INVARIANT_CODE: &str = "forum.user_trust_facts.storage_invariant";

/// Forum-owned exact-actor adapter for the authoritative user trust projection.
///
/// The adapter accepts only a normalized trust-only request for the exact user
/// represented by the read-only `PortContext`. It reads `forum_user_trust_states`
/// directly because that table is the Forum owner projection. Missing state is
/// the canonical fail-closed trust level `0`; `forum_user_stats` is never read or
/// interpreted by this adapter.
#[derive(Clone)]
pub struct ForumUserTrustAudienceFactsPort {
    db: DatabaseConnection,
}

impl ForumUserTrustAudienceFactsPort {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub fn shared(db: DatabaseConnection) -> SharedForumAudienceFactsPort {
        Arc::new(Self::new(db))
    }
}

#[async_trait]
impl ForumAudienceFactsPort for ForumUserTrustAudienceFactsPort {
    async fn resolve_forum_audience_facts(
        &self,
        context: PortContext,
        request: ForumAudienceFactsRequest,
    ) -> Result<ForumAudienceFacts, PortError> {
        let request = normalize_request(request)?;
        validate_context(&context, &request)?;
        validate_trust_only_request(&request)?;

        let state = forum_user_trust_state::Entity::find_by_id((request.tenant_id, request.user_id))
            .one(&self.db)
            .await
            .map_err(|_| {
                PortError::unavailable(
                    STORAGE_UNAVAILABLE_CODE,
                    "Forum user trust facts storage is unavailable",
                )
            })?;
        let trust_level = state
            .map(|state| validate_state(&request, state))
            .transpose()?
            .unwrap_or(0);

        Ok(ForumAudienceFacts {
            tenant_id: request.tenant_id,
            user_id: request.user_id,
            trust_level: Some(trust_level),
            channel_memberships: Vec::new(),
            group_memberships: Vec::new(),
        })
    }
}

fn normalize_request(
    request: ForumAudienceFactsRequest,
) -> Result<ForumAudienceFactsRequest, PortError> {
    request.normalize().map_err(|_| {
        PortError::validation(
            INVALID_REQUEST_CODE,
            "Forum user trust facts request is invalid",
        )
    })
}

fn validate_context(
    context: &PortContext,
    request: &ForumAudienceFactsRequest,
) -> Result<(), PortError> {
    context.require_policy(PortCallPolicy::read())?;
    if context.tenant_id != request.tenant_id.to_string() {
        return Err(PortError::validation(
            TENANT_MISMATCH_CODE,
            "Forum user trust facts tenant does not match the caller context",
        ));
    }
    if context.actor.kind != PortActorKind::User
        || Uuid::parse_str(&context.actor.id).ok() != Some(request.user_id)
    {
        return Err(PortError::forbidden(
            ACTOR_MISMATCH_CODE,
            "Forum user trust facts require the exact requested user actor",
        ));
    }
    Ok(())
}

fn validate_trust_only_request(request: &ForumAudienceFactsRequest) -> Result<(), PortError> {
    if !request.include_trust_level
        || !request.channel_slugs.is_empty()
        || !request.group_ids.is_empty()
    {
        return Err(PortError::validation(
            UNSUPPORTED_REQUEST_CODE,
            "Forum user trust facts require one trust-only request",
        ));
    }
    Ok(())
}

fn validate_state(
    request: &ForumAudienceFactsRequest,
    state: forum_user_trust_state::Model,
) -> Result<u8, PortError> {
    if state.tenant_id != request.tenant_id
        || state.user_id != request.user_id
        || state.revision <= 0
    {
        return Err(PortError::invariant_violation(
            STORAGE_INVARIANT_CODE,
            "Forum user trust state returned an invalid identity or revision",
        ));
    }
    let trust_level = u8::try_from(state.trust_level).map_err(|_| {
        PortError::invariant_violation(
            STORAGE_INVARIANT_CODE,
            "Forum user trust state contains an invalid level",
        )
    })?;
    if trust_level > MAX_FORUM_USER_TRUST_LEVEL {
        return Err(PortError::invariant_violation(
            STORAGE_INVARIANT_CODE,
            "Forum user trust state contains an invalid level",
        ));
    }
    Ok(trust_level)
}
