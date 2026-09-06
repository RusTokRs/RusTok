use rustok_api::{Action, PortContext, Resource};
use rustok_core::SecurityContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::audience::{
    FORUM_AUDIENCE_FACTS_CAPABILITY, FORUM_AUDIENCE_FACTS_CAPABILITY_UNAVAILABLE,
    ForumAudienceConstraints, ForumAudienceDecisionReason, ForumAudienceEvaluator,
    ForumAudienceFacts, ForumAudienceFactsResolver, SharedForumAudienceFactsPort,
};
use crate::entities::forum_topic;
use crate::error::{ForumError, ForumResult};

use super::rbac::enforce_scope;
use super::topic_reply_create_audience::load_topic_reply_create_audience_policy_for_topic;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumReplyCreateAudienceAuthorization {
    pub topic_id: Uuid,
    pub category_id: Uuid,
    pub evaluated_layers: usize,
    pub allowed: bool,
    /// Exact denying category layer. `None` on a denied result identifies the
    /// already-present `topic_id` as the final topic-local denying layer.
    pub denied_by_category_id: Option<Uuid>,
    pub reason: ForumAudienceDecisionReason,
}

/// Evaluates inherited category layers followed by the optional topic-local
/// reply-create narrowing before the raw reply owner prepares relations or
/// writes reply, body, counter, user-stat, or event rows.
pub struct ForumReplyCreateAudienceAuthorizationService {
    db: sea_orm::DatabaseConnection,
    facts_resolver: ForumAudienceFactsResolver,
}

impl ForumReplyCreateAudienceAuthorizationService {
    pub fn new(
        db: sea_orm::DatabaseConnection,
        facts_port: Option<SharedForumAudienceFactsPort>,
    ) -> Self {
        Self {
            db,
            facts_resolver: ForumAudienceFactsResolver::new(facts_port),
        }
    }

    pub fn without_facts_provider(db: sea_orm::DatabaseConnection) -> Self {
        Self::new(db, None)
    }

    pub async fn evaluate(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: &SecurityContext,
        context: Option<PortContext>,
    ) -> ForumResult<ForumReplyCreateAudienceAuthorization> {
        enforce_scope(security, Resource::ForumReplies, Action::Create)?;
        let topic = forum_topic::Entity::find_by_id(topic_id)
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ForumError::TopicNotFound(topic_id))?;
        let category_id = topic.category_id;
        let policy =
            load_topic_reply_create_audience_policy_for_topic(&self.db, tenant_id, &topic).await?;

        let mut evaluated_layers = 0usize;
        let mut last_reason = ForumAudienceDecisionReason::Unrestricted;
        for layer in policy.inherited_category_layers {
            evaluated_layers += 1;
            let facts = self
                .facts_for_layer(tenant_id, security, context.clone(), &layer.constraints)
                .await?;
            let decision =
                ForumAudienceEvaluator::decide(tenant_id, &layer.constraints, security, &facts)?;
            last_reason = decision.reason;
            if !decision.allowed {
                return Ok(ForumReplyCreateAudienceAuthorization {
                    topic_id,
                    category_id,
                    evaluated_layers,
                    allowed: false,
                    denied_by_category_id: Some(layer.category_id),
                    reason: decision.reason,
                });
            }
        }

        if let Some(constraints) = policy.configured_constraints {
            evaluated_layers += 1;
            let facts = self
                .facts_for_layer(tenant_id, security, context, &constraints)
                .await?;
            let decision =
                ForumAudienceEvaluator::decide(tenant_id, &constraints, security, &facts)?;
            last_reason = decision.reason;
            if !decision.allowed {
                return Ok(ForumReplyCreateAudienceAuthorization {
                    topic_id,
                    category_id,
                    evaluated_layers,
                    allowed: false,
                    denied_by_category_id: None,
                    reason: decision.reason,
                });
            }
        }

        Ok(ForumReplyCreateAudienceAuthorization {
            topic_id,
            category_id,
            evaluated_layers,
            allowed: true,
            denied_by_category_id: None,
            reason: last_reason,
        })
    }

    pub async fn require(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: &SecurityContext,
        context: Option<PortContext>,
    ) -> ForumResult<ForumReplyCreateAudienceAuthorization> {
        let authorization = self
            .evaluate(tenant_id, topic_id, security, context)
            .await?;
        if !authorization.allowed {
            return Err(ForumError::forbidden(
                "Forum reply creation is unavailable for the current audience",
            ));
        }
        Ok(authorization)
    }

    async fn facts_for_layer(
        &self,
        tenant_id: Uuid,
        security: &SecurityContext,
        context: Option<PortContext>,
        constraints: &ForumAudienceConstraints,
    ) -> ForumResult<ForumAudienceFacts> {
        if owner_facts_still_required(constraints, security) {
            let context = context.ok_or_else(|| {
                ForumError::capability_unavailable(
                    FORUM_AUDIENCE_FACTS_CAPABILITY,
                    FORUM_AUDIENCE_FACTS_CAPABILITY_UNAVAILABLE,
                )
            })?;
            return self
                .facts_resolver
                .resolve_for_constraints(tenant_id, context, security, constraints)
                .await;
        }

        Ok(security
            .user_id
            .map(|user_id| ForumAudienceFacts {
                tenant_id,
                user_id,
                ..ForumAudienceFacts::default()
            })
            .unwrap_or_default())
    }
}

fn owner_facts_still_required(
    constraints: &ForumAudienceConstraints,
    security: &SecurityContext,
) -> bool {
    if !constraints.requires_owner_facts() || security.is_public_read() {
        return false;
    }
    let Some(user_id) = security.user_id else {
        return false;
    };
    if constraints.deny_user_ids.binary_search(&user_id).is_ok()
        || constraints.allow_user_ids.binary_search(&user_id).is_ok()
        || constraints.roles_any.contains(&security.role)
    {
        return false;
    }
    true
}
