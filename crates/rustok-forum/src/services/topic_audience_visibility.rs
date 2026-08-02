use rustok_api::{PortActorKind, PortContext};
use rustok_core::{SecurityActorKind, SecurityContext};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::audience::{
    ForumAudienceConstraints, ForumAudienceEvaluator, ForumAudienceFacts,
    ForumAudienceFactsResolver, SharedForumAudienceFactsPort,
};
use crate::error::{ForumError, ForumResult};
use crate::services::topic_audience::{find_topic, load_policy_for_topic};
use crate::services::topic_visibility::{ForumTopicVisibilityScope, ForumTopicVisibilityService};

/// Exact viewer identity used while composing persisted category/topic audience layers.
#[derive(Clone, Debug)]
pub struct ForumTopicAudienceViewer {
    security: SecurityContext,
    port_context: Option<PortContext>,
}

impl ForumTopicAudienceViewer {
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
                "Forum topic audience authenticated viewer cannot use public security".to_string(),
            ));
        }
        if security.actor_kind != SecurityActorKind::User {
            return Err(ForumError::Validation(
                "Forum topic audience authenticated viewer requires a user security actor"
                    .to_string(),
            ));
        }
        let user_id = security.user_id.ok_or_else(|| {
            ForumError::Validation(
                "Forum topic audience authenticated viewer requires a user identity".to_string(),
            )
        })?;
        if port_context.actor.kind != PortActorKind::User {
            return Err(ForumError::Validation(
                "Forum topic audience facts context requires a user actor".to_string(),
            ));
        }
        let context_user_id = Uuid::parse_str(&port_context.actor.id).map_err(|_| {
            ForumError::Validation(
                "Forum topic audience facts context actor is invalid".to_string(),
            )
        })?;
        if context_user_id != user_id {
            return Err(ForumError::Validation(
                "Forum topic audience facts context actor does not match the viewer".to_string(),
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

/// Exact topic visibility composition through the current owner and every
/// normalized richer category/topic audience layer.
pub struct ForumTopicAudienceVisibilityService {
    db: DatabaseConnection,
    facts_resolver: ForumAudienceFactsResolver,
}

impl ForumTopicAudienceVisibilityService {
    pub fn new(db: DatabaseConnection, facts_port: Option<SharedForumAudienceFactsPort>) -> Self {
        Self {
            db,
            facts_resolver: ForumAudienceFactsResolver::new(facts_port),
        }
    }

    pub fn without_facts_provider(db: DatabaseConnection) -> Self {
        Self::new(db, None)
    }

    /// Missing, foreign, closed, category-denied, route-channel-denied or
    /// richer-audience-denied topics all resolve to `false` without exposing
    /// which policy rejected the target.
    pub async fn is_topic_visible(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        channel_slug: Option<&str>,
        viewer: &ForumTopicAudienceViewer,
    ) -> ForumResult<bool> {
        self.validate_viewer_context(tenant_id, viewer)?;

        let scope = ForumTopicVisibilityScope::storefront_for_viewer(
            channel_slug,
            viewer.is_authenticated(),
        )?;
        if !ForumTopicVisibilityService::new(self.db.clone())
            .is_topic_visible(tenant_id, topic_id, &scope)
            .await?
        {
            return Ok(false);
        }

        self.policy_allows(tenant_id, topic_id, viewer).await
    }

    /// Exact owner-read visibility for a topic and resources owned by that topic.
    ///
    /// Unlike storefront visibility this intentionally does not require an open
    /// topic or a matching route channel. It preserves owner/admin reads while
    /// enforcing the inherited category floor and every richer category/topic
    /// audience layer. Missing and denied topics both resolve to `false`.
    pub async fn is_topic_owner_visible(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        viewer: &ForumTopicAudienceViewer,
    ) -> ForumResult<bool> {
        self.validate_viewer_context(tenant_id, viewer)?;
        if !ForumTopicVisibilityService::new(self.db.clone())
            .is_topic_category_visible_to_viewer(tenant_id, topic_id, viewer.is_authenticated())
            .await?
        {
            return Ok(false);
        }

        self.policy_allows(tenant_id, topic_id, viewer).await
    }

    async fn policy_allows(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        viewer: &ForumTopicAudienceViewer,
    ) -> ForumResult<bool> {
        let topic = match find_topic(&self.db, tenant_id, topic_id).await {
            Ok(topic) => topic,
            Err(ForumError::TopicNotFound(_)) => return Ok(false),
            Err(error) => return Err(error),
        };
        let policy = load_policy_for_topic(&self.db, tenant_id, &topic).await?;

        for layer in &policy.inherited_category_layers {
            if !self
                .constraints_allow(tenant_id, &layer.constraints, viewer)
                .await?
            {
                return Ok(false);
            }
        }
        if let Some(constraints) = &policy.configured_constraints
            && !self
                .constraints_allow(tenant_id, constraints, viewer)
                .await?
        {
            return Ok(false);
        }
        Ok(true)
    }

    fn validate_viewer_context(
        &self,
        tenant_id: Uuid,
        viewer: &ForumTopicAudienceViewer,
    ) -> ForumResult<()> {
        let Some(context) = viewer.port_context.as_ref() else {
            return Ok(());
        };
        let context_tenant_id = Uuid::parse_str(&context.tenant_id).map_err(|_| {
            ForumError::Validation(
                "Forum topic audience facts context tenant is invalid".to_string(),
            )
        })?;
        if context_tenant_id != tenant_id {
            return Err(ForumError::Validation(
                "Forum topic audience facts context tenant does not match the request".to_string(),
            ));
        }
        Ok(())
    }

    async fn constraints_allow(
        &self,
        tenant_id: Uuid,
        constraints: &ForumAudienceConstraints,
        viewer: &ForumTopicAudienceViewer,
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
                "Forum topic audience authenticated viewer is missing facts context".to_string(),
            )
        })?;
        let mut facts = self
            .facts_resolver
            .resolve_for_constraints(tenant_id, context.clone(), &viewer.security, constraints)
            .await?;

        // Local allow/deny/role resolution intentionally skips owner-port calls.
        // Bind an empty local result to the exact viewer so a nonmatching local
        // selector evaluates to NoMatch instead of looking like foreign facts.
        if facts == ForumAudienceFacts::default() {
            facts.tenant_id = tenant_id;
            facts.user_id = viewer
                .security
                .user_id
                .expect("authenticated viewer validated");
        }

        Ok(
            ForumAudienceEvaluator::decide(tenant_id, constraints, &viewer.security, &facts)?
                .allowed,
        )
    }
}
