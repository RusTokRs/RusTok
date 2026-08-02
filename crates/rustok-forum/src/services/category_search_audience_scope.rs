use rustok_api::PortContext;
use rustok_core::SecurityContext;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::audience::SharedForumAudienceFactsPort;
use crate::dto::CategoryTreeQuery;
use crate::error::ForumResult;

use super::category_audience_read::ForumCategoryAudienceReadService;
use super::category_search_scope::{
    ForumSearchCategoryScope, expand_search_category_scope_from_visible_tree,
};

/// Forum-owned category subtree scope after the complete delivered category
/// audience decision has been applied for the exact viewer.
pub struct ForumSearchCategoryAudienceScopeService {
    category_reads: ForumCategoryAudienceReadService,
}

impl ForumSearchCategoryAudienceScopeService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            category_reads: ForumCategoryAudienceReadService::new(db),
        }
    }

    pub fn with_audience_facts(
        db: DatabaseConnection,
        facts_port: SharedForumAudienceFactsPort,
    ) -> Self {
        Self {
            category_reads: ForumCategoryAudienceReadService::with_audience_facts(db, facts_port),
        }
    }

    /// Expands public category roots after the complete inherited category
    /// audience decision and archive pruning have already shaped the owner tree.
    pub async fn expand_public_visible_subtrees(
        &self,
        tenant_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        category_ids: &[Uuid],
    ) -> ForumResult<ForumSearchCategoryScope> {
        let tree = self
            .category_reads
            .tree_public_storefront_visible_with_locale_fallback(tenant_id, locale, fallback_locale)
            .await?;
        expand_search_category_scope_from_visible_tree(&tree.roots, category_ids)
    }

    /// Expands authenticated category roots through the exact request-bound
    /// audience context. Missing required owner facts fail closed in the
    /// canonical category audience read owner before IDs reach Search.
    pub async fn expand_authenticated_visible_subtrees(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        fallback_locale: Option<&str>,
        category_ids: &[Uuid],
    ) -> ForumResult<ForumSearchCategoryScope> {
        let tree = self
            .category_reads
            .tree_authenticated_owner_visible_with_audience_context(
                tenant_id,
                security,
                context,
                CategoryTreeQuery {
                    locale: None,
                    fallback_locale: fallback_locale.map(ToOwned::to_owned),
                },
            )
            .await?;
        expand_search_category_scope_from_visible_tree(&tree.roots, category_ids)
    }
}
