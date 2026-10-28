//! Blog-owned live audience authorization adapter for the generic Search SPI.
//!
//! The host wires the independent Blog and Search owners; the adapter never
//! reads Blog tables and Search never depends on the Blog implementation.

use async_trait::async_trait;
use rustok_api::{PortError, is_tenant_module_enabled};
use rustok_search::{SearchAudiencePolicyPort, SearchAudienceRequest};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct ServerBlogSearchResultEligibilityPort {
    db: DatabaseConnection,
}

impl ServerBlogSearchResultEligibilityPort {
    pub(crate) fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn enabled(&self, tenant_id: Uuid) -> Result<bool, PortError> {
        is_tenant_module_enabled(&self.db, tenant_id, "blog")
            .await
            .map_err(|error| {
                tracing::error!(%tenant_id, %error, "Blog Search module state unavailable");
                PortError::unavailable(
                    "blog.search.module_state_unavailable",
                    "Blog Search visibility is temporarily unavailable",
                )
            })
    }
}

#[async_trait]
impl SearchAudiencePolicyPort for ServerBlogSearchResultEligibilityPort {
    fn source_module(&self) -> &'static str {
        "blog"
    }

    async fn public_discovery_allowed(&self, tenant_id: Uuid) -> Result<bool, PortError> {
        self.enabled(tenant_id).await
    }

    async fn filter_eligible_candidates(
        &self,
        request: SearchAudienceRequest,
    ) -> Result<Vec<Uuid>, PortError> {
        if request.tenant_id.is_nil() {
            return Err(PortError::validation(
                "blog.search.tenant_required",
                "Blog Search requires a valid tenant",
            ));
        }
        if request.auth.as_ref().is_some_and(|auth| auth.tenant_id != request.tenant_id) {
            return Err(PortError::forbidden(
                "blog.search.auth_tenant_mismatch",
                "Blog Search auth tenant mismatch",
            ));
        }
        let context = request.request_context.as_ref().ok_or_else(|| {
            PortError::forbidden(
                "blog.search.request_context_required",
                "Blog Search requires a trusted request channel context",
            )
        })?;
        if context.tenant_id != request.tenant_id
            || request.auth.as_ref().is_some_and(|auth| context.user_id != Some(auth.user_id))
        {
            return Err(PortError::forbidden(
                "blog.search.request_context_mismatch",
                "Blog Search request identity mismatch",
            ));
        }

        if !self.enabled(request.tenant_id).await? {
            return Ok(Vec::new());
        }
        let post_ids = request.candidates.iter()
            .filter(|item| item.source_module == "blog" && item.entity_type == "blog_post")
            .map(|item| item.document_id)
            .collect::<Vec<_>>();

        rustok_blog::search_projection::filter_public_search_post_ids(
            &self.db,
            request.tenant_id,
            context.channel_slug.as_deref(),
            &post_ids,
        )
        .await
        .map_err(|error| {
            tracing::error!(tenant_id = %request.tenant_id, %error, "Blog Search owner visibility unavailable");
            PortError::unavailable(
                "blog.search.owner_visibility_unavailable",
                "Blog Search visibility is temporarily unavailable",
            )
        })
    }
}
