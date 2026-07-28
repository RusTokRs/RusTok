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
const TENANT_MISMATCH_CODE: &str = "forum.user_trust_facts.tenant_mismatch";
const ACTOR_MISMATCH_CODE: &str = "forum.user_trust_facts.actor_mismatch";
const FALLBACK_UNAVAILABLE_CODE: &str = "forum.user_trust_facts.membership_provider_unavailable";
const FALLBACK_RESPONSE_CODE: &str = "forum.user_trust_facts.membership_response_invalid";
const STORAGE_UNAVAILABLE_CODE: &str = "forum.user_trust_facts.storage_unavailable";
const STORAGE_INVARIANT_CODE: &str = "forum.user_trust_facts.storage_invariant";

/// Forum-owned exact-actor adapter for the authoritative user trust projection.
///
/// Membership dimensions are delegated as one request with trust explicitly
/// disabled. A returned Channel or Groups membership already decides the
/// positive-selector union and is returned without a trust read. Only a bounded
/// confirmed membership miss reaches `forum_user_trust_states`. Missing trust
/// state is the canonical fail-closed level `0`; `forum_user_stats` is never read
/// or interpreted by this adapter.
#[derive(Clone)]
pub struct ForumUserTrustAudienceFactsPort {
    db: DatabaseConnection,
    membership_facts: Option<SharedForumAudienceFactsPort>,
}

impl ForumUserTrustAudienceFactsPort {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            membership_facts: None,
        }
    }

    pub fn with_membership_facts(
        db: DatabaseConnection,
        membership_facts: SharedForumAudienceFactsPort,
    ) -> Self {
        Self {
            db,
            membership_facts: Some(membership_facts),
        }
    }

    pub fn shared(
        db: DatabaseConnection,
        membership_facts: SharedForumAudienceFactsPort,
    ) -> SharedForumAudienceFactsPort {
        Arc::new(Self::with_membership_facts(db, membership_facts))
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

        if !request.channel_slugs.is_empty() || !request.group_ids.is_empty() {
            let membership_facts = self
                .resolve_membership_facts(context.clone(), &request)
                .await?;
            if !membership_facts.channel_memberships.is_empty()
                || !membership_facts.group_memberships.is_empty()
                || !request.include_trust_level
            {
                return Ok(membership_facts);
            }
        }

        let trust_level = if request.include_trust_level {
            Some(self.read_trust_level(&request).await?)
        } else {
            None
        };

        Ok(ForumAudienceFacts {
            tenant_id: request.tenant_id,
            user_id: request.user_id,
            trust_level,
            channel_memberships: Vec::new(),
            group_memberships: Vec::new(),
        })
    }
}

impl ForumUserTrustAudienceFactsPort {
    async fn resolve_membership_facts(
        &self,
        context: PortContext,
        request: &ForumAudienceFactsRequest,
    ) -> Result<ForumAudienceFacts, PortError> {
        let Some(membership_facts) = &self.membership_facts else {
            return Err(PortError::unavailable(
                FALLBACK_UNAVAILABLE_CODE,
                "Forum Channel or Groups audience facts are unavailable",
            ));
        };
        let membership_request = ForumAudienceFactsRequest {
            tenant_id: request.tenant_id,
            user_id: request.user_id,
            include_trust_level: false,
            channel_slugs: request.channel_slugs.clone(),
            group_ids: request.group_ids.clone(),
        };
        let facts = membership_facts
            .resolve_forum_audience_facts(context, membership_request.clone())
            .await?;
        facts
            .validate_for_request(&membership_request)
            .map_err(|_| {
                PortError::invariant_violation(
                    FALLBACK_RESPONSE_CODE,
                    "Forum membership facts returned an invalid bounded response",
                )
            })
    }

    async fn read_trust_level(
        &self,
        request: &ForumAudienceFactsRequest,
    ) -> Result<u8, PortError> {
        let state = forum_user_trust_state::Entity::find_by_id((request.tenant_id, request.user_id))
            .one(&self.db)
            .await
            .map_err(|_| {
                PortError::unavailable(
                    STORAGE_UNAVAILABLE_CODE,
                    "Forum user trust facts storage is unavailable",
                )
            })?;
        state
            .map(|state| validate_state(request, state))
            .transpose()
            .map(|level| level.unwrap_or(0))
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
