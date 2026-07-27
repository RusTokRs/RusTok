use rustok_api::{Action, PortContext, Resource};
use rustok_core::SecurityContext;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::audience::{
    FORUM_AUDIENCE_FACTS_CAPABILITY, FORUM_AUDIENCE_FACTS_CAPABILITY_UNAVAILABLE,
    ForumAudienceConstraints, ForumAudienceDecisionReason, ForumAudienceEvaluator,
    ForumAudienceFacts, ForumAudienceFactsResolver, SharedForumAudienceFactsPort,
};
use crate::error::{ForumError, ForumResult};

use super::category_topic_create_audience::load_category_topic_create_audience_policy;
use super::rbac::enforce_scope;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ForumTopicCreateAudienceAuthorization {
    pub category_id: Uuid,
    pub evaluated_layers: usize,
    pub allowed: bool,
    pub denied_by_category_id: Option<Uuid>,
    pub reason: ForumAudienceDecisionReason,
}

/// Evaluates the inherited category topic-create audience before any topic row,
/// translation, relation, counter, or domain event is written.
pub struct ForumTopicCreateAudienceAuthorizationService {
    db: sea_orm::DatabaseConnection,
    facts_resolver: ForumAudienceFactsResolver,
}

impl ForumTopicCreateAudienceAuthorizationService {
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
        category_id: Uuid,
        security: &SecurityContext,
        context: Option<PortContext>,
    ) -> ForumResult<ForumTopicCreateAudienceAuthorization> {
        enforce_scope(security, Resource::ForumTopics, Action::Create)?;
        let policy =
            load_category_topic_create_audience_policy(&self.db, tenant_id, category_id).await?;

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
                return Ok(ForumTopicCreateAudienceAuthorization {
                    category_id,
                    evaluated_layers,
                    allowed: false,
                    denied_by_category_id: Some(layer.category_id),
                    reason: decision.reason,
                });
            }
        }

        Ok(ForumTopicCreateAudienceAuthorization {
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
        category_id: Uuid,
        security: &SecurityContext,
        context: Option<PortContext>,
    ) -> ForumResult<ForumTopicCreateAudienceAuthorization> {
        let authorization = self
            .evaluate(tenant_id, category_id, security, context)
            .await?;
        if !authorization.allowed {
            return Err(ForumError::forbidden(
                "Forum topic creation is unavailable for the current audience",
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
