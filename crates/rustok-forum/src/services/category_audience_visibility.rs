use rustok_api::{PortActorKind, PortContext};
use rustok_core::{SecurityActorKind, SecurityContext};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::audience::{
    ForumAudienceConstraints, ForumAudienceEvaluator, ForumAudienceFacts,
    ForumAudienceFactsResolver, SharedForumAudienceFactsPort,
};
use crate::error::{ForumError, ForumResult};

use super::category_audience::load_category_audience_policy;
use super::category_visibility::ForumCategoryVisibilityPolicyService;

/// Exact viewer identity used while composing inherited category audience layers.
#[derive(Clone, Debug)]
pub struct ForumCategoryAudienceViewer {
    security: SecurityContext,
    port_context: Option<PortContext>,
}

impl ForumCategoryAudienceViewer {
    pub fn public() -> Self {
        Self {
            security: SecurityContext::public_read(),
            port_context: None,
        }
    }

    pub fn authenticated(
        security: SecurityContext,
        port_context: PortContext,
    ) -> ForumResult<Self> {
        if security.is_public_read() {
            return Err(ForumError::Validation(
                "Forum category audience authenticated viewer cannot use public security"
                    .to_string(),
            ));
        }
        if security.actor_kind != SecurityActorKind::User {
            return Err(ForumError::Validation(
                "Forum category audience authenticated viewer requires a user security actor"
                    .to_string(),
            ));
        }
        let user_id = security.user_id.ok_or_else(|| {
            ForumError::Validation(
                "Forum category audience authenticated viewer requires a user identity".to_string(),
            )
        })?;
        if port_context.actor.kind != PortActorKind::User {
            return Err(ForumError::Validation(
                "Forum category audience facts context requires a user actor".to_string(),
            ));
        }
        let context_user_id = Uuid::parse_str(&port_context.actor.id).map_err(|_| {
            ForumError::Validation(
                "Forum category audience facts context actor is invalid".to_string(),
            )
        })?;
        if context_user_id != user_id {
            return Err(ForumError::Validation(
                "Forum category audience facts context actor does not match the viewer".to_string(),
            ));
        }
        Ok(Self {
            security,
            port_context: Some(port_context),
        })
    }

    pub fn is_authenticated(&self) -> bool {
        !self.security.is_public_read()
    }
}

/// Exact category visibility through the inherited public/authenticated floor and
/// every normalized richer category audience layer.
pub struct ForumCategoryAudienceVisibilityService {
    db: DatabaseConnection,
    facts_resolver: ForumAudienceFactsResolver,
}

impl ForumCategoryAudienceVisibilityService {
    pub fn new(db: DatabaseConnection, facts_port: Option<SharedForumAudienceFactsPort>) -> Self {
        Self {
            db,
            facts_resolver: ForumAudienceFactsResolver::new(facts_port),
        }
    }

    pub fn without_facts_provider(db: DatabaseConnection) -> Self {
        Self::new(db, None)
    }

    /// Missing, foreign, base-floor-denied and richer-audience-denied categories
    /// all resolve to `false` without exposing which policy rejected the target.
    pub async fn is_category_visible(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        viewer: &ForumCategoryAudienceViewer,
    ) -> ForumResult<bool> {
        self.validate_viewer_context(tenant_id, viewer)?;

        if !ForumCategoryVisibilityPolicyService::new(self.db.clone())
            .is_category_visible_to_viewer(tenant_id, category_id, viewer.is_authenticated())
            .await?
        {
            return Ok(false);
        }

        let policy = match load_category_audience_policy(&self.db, tenant_id, category_id).await {
            Ok(policy) => policy,
            Err(ForumError::CategoryNotFound(_)) => return Ok(false),
            Err(error) => return Err(error),
        };

        for layer in &policy.effective_layers {
            if !self
                .constraints_allow(tenant_id, &layer.constraints, viewer)
                .await?
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn validate_viewer_context(
        &self,
        tenant_id: Uuid,
        viewer: &ForumCategoryAudienceViewer,
    ) -> ForumResult<()> {
        let Some(context) = viewer.port_context.as_ref() else {
            return Ok(());
        };
        let context_tenant_id = Uuid::parse_str(&context.tenant_id).map_err(|_| {
            ForumError::Validation(
                "Forum category audience facts context tenant is invalid".to_string(),
            )
        })?;
        if context_tenant_id != tenant_id {
            return Err(ForumError::Validation(
                "Forum category audience facts context tenant does not match the request"
                    .to_string(),
            ));
        }
        Ok(())
    }

    async fn constraints_allow(
        &self,
        tenant_id: Uuid,
        constraints: &ForumAudienceConstraints,
        viewer: &ForumCategoryAudienceViewer,
    ) -> ForumResult<bool> {
        if viewer.security.is_public_read() {
            return Ok(ForumAudienceEvaluator::decide(
                tenant_id,
                constraints,
                &viewer.security,
                &ForumAudienceFacts::default(),
            )?
            .allowed);
        }

        let context = viewer.port_context.as_ref().ok_or_else(|| {
            ForumError::Validation(
                "Forum category audience authenticated viewer is missing facts context".to_string(),
            )
        })?;
        let mut facts = self
            .facts_resolver
            .resolve_for_constraints(tenant_id, context.clone(), &viewer.security, constraints)
            .await?;

        // Local allow/deny/role decisions intentionally avoid owner-port calls.
        // Bind the empty result to the exact viewer before evaluator validation.
        if facts == ForumAudienceFacts::default() {
            facts.tenant_id = tenant_id;
            facts.user_id = viewer
                .security
                .user_id
                .expect("authenticated category viewer validated");
        }

        Ok(
            ForumAudienceEvaluator::decide(tenant_id, constraints, &viewer.security, &facts)?
                .allowed,
        )
    }
}
