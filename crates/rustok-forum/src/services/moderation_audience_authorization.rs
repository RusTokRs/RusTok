use rustok_api::{Action, PortActorKind, PortContext, Resource};
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
use crate::entities::{forum_reply, forum_topic};
use crate::error::{ForumError, ForumResult};
use crate::moderation_transport::{
    current_moderation_audience_context, current_moderation_audience_facts,
};

use super::category_moderation_audience::load_category_moderation_audience_policy;
use super::rbac::enforce_scope;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumModerationAudienceAuthorization {
    pub category_id: Uuid,
    pub evaluated_layers: usize,
    pub allowed: bool,
    pub denied_by_category_id: Option<Uuid>,
    pub reason: ForumAudienceDecisionReason,
}

/// Evaluates inherited category moderation audience layers before a moderation
/// owner opens its write transaction or mutates topic, reply, counter, stat,
/// solution, journal, or outbox state.
pub struct ForumModerationAudienceAuthorizationService {
    db: sea_orm::DatabaseConnection,
    facts_resolver: ForumAudienceFactsResolver,
}

impl ForumModerationAudienceAuthorizationService {
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
        Self::new(db, current_moderation_audience_facts())
    }

    pub async fn evaluate_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: &SecurityContext,
        context: Option<PortContext>,
    ) -> ForumResult<ForumModerationAudienceAuthorization> {
        enforce_scope(security, Resource::ForumTopics, Action::Moderate)?;
        let context = exact_transport_context(tenant_id, security, context)?;
        let topic = forum_topic::Entity::find_by_id(topic_id)
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ForumError::TopicNotFound(topic_id))?;
        self.evaluate_category(tenant_id, topic.category_id, security, context)
            .await
    }

    pub async fn evaluate_reply(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        expected_topic_id: Uuid,
        security: &SecurityContext,
        context: Option<PortContext>,
    ) -> ForumResult<ForumModerationAudienceAuthorization> {
        enforce_scope(security, Resource::ForumReplies, Action::Moderate)?;
        let context = exact_transport_context(tenant_id, security, context)?;
        let reply = forum_reply::Entity::find_by_id(reply_id)
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ForumError::ReplyNotFound(reply_id))?;
        if reply.topic_id != expected_topic_id {
            return Err(ForumError::Validation(
                "Reply belongs to another topic".to_string(),
            ));
        }
        let topic = forum_topic::Entity::find_by_id(expected_topic_id)
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ForumError::TopicNotFound(expected_topic_id))?;
        self.evaluate_category(tenant_id, topic.category_id, security, context)
            .await
    }

    pub async fn require_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: &SecurityContext,
        context: Option<PortContext>,
    ) -> ForumResult<ForumModerationAudienceAuthorization> {
        let authorization = self
            .evaluate_topic(tenant_id, topic_id, security, context)
            .await?;
        self.require_allowed(authorization)
    }

    pub async fn require_reply(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        expected_topic_id: Uuid,
        security: &SecurityContext,
        context: Option<PortContext>,
    ) -> ForumResult<ForumModerationAudienceAuthorization> {
        let authorization = self
            .evaluate_reply(
                tenant_id,
                reply_id,
                expected_topic_id,
                security,
                context,
            )
            .await?;
        self.require_allowed(authorization)
    }

    async fn evaluate_category(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: &SecurityContext,
        context: Option<PortContext>,
    ) -> ForumResult<ForumModerationAudienceAuthorization> {
        let policy =
            load_category_moderation_audience_policy(&self.db, tenant_id, category_id).await?;
        let mut evaluated_layers = 0usize;
        let mut last_reason = ForumAudienceDecisionReason::Unrestricted;
        for layer in policy.effective_layers {
            evaluated_layers += 1;
            let facts = self
                .facts_for_layer(
                    tenant_id,
                    security,
                    context.clone(),
                    &layer.constraints,
                )
                .await?;
            let decision = ForumAudienceEvaluator::decide(
                tenant_id,
                &layer.constraints,
                security,
                &facts,
            )?;
            last_reason = decision.reason;
            if !decision.allowed {
                return Ok(ForumModerationAudienceAuthorization {
                    category_id,
                    evaluated_layers,
                    allowed: false,
                    denied_by_category_id: Some(layer.category_id),
                    reason: decision.reason,
                });
            }
        }

        Ok(ForumModerationAudienceAuthorization {
            category_id,
            evaluated_layers,
            allowed: true,
            denied_by_category_id: None,
            reason: last_reason,
        })
    }

    fn require_allowed(
        &self,
        authorization: ForumModerationAudienceAuthorization,
    ) -> ForumResult<ForumModerationAudienceAuthorization> {
        if !authorization.allowed {
            return Err(ForumError::forbidden(
                "Forum moderation is unavailable for the current audience",
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

fn exact_transport_context(
    tenant_id: Uuid,
    security: &SecurityContext,
    context: Option<PortContext>,
) -> ForumResult<Option<PortContext>> {
    let Some(context) = context.or_else(current_moderation_audience_context) else {
        return Ok(None);
    };

    if context.tenant_id != tenant_id.to_string() {
        return Err(ForumError::Validation(
            "Forum moderation transport tenant does not match the owner command".to_string(),
        ));
    }
    let Some(user_id) = security.user_id else {
        return Err(ForumError::Validation(
            "Forum moderation transport requires an authenticated user actor".to_string(),
        ));
    };
    if context.actor.kind != PortActorKind::User || context.actor.id != user_id.to_string() {
        return Err(ForumError::Validation(
            "Forum moderation transport actor does not match the owner command".to_string(),
        ));
    }

    Ok(Some(context))
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
