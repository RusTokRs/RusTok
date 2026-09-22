use std::collections::HashSet;

use rustok_api::{Action, PortContext, Resource};
use rustok_core::SecurityContext;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::audience::SharedForumAudienceFactsPort;
use crate::dto::{
    CategoryListItem, CategoryResponse, CategoryTreeNode, CategoryTreeQuery, CategoryTreeResponse,
    MAX_FORUM_CATEGORY_TREE_NODES, MAX_FORUM_READ_LIMIT,
};
use crate::error::{ForumError, ForumResult};

use super::category_audience_visibility::{
    ForumCategoryAudienceViewer, ForumCategoryAudienceVisibilityService,
};
use super::category_owner::CategoryService;
use super::rbac::enforce_scope;

const FORUM_CATEGORY_AUDIENCE_SCAN_PAGE_SIZE: u64 = MAX_FORUM_READ_LIMIT;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ForumCategoryAudiencePage {
    pub items: Vec<CategoryListItem>,
    pub total: u64,
}

/// Exact owner for category single, list and tree reads.
///
/// Base public/authenticated visibility remains owned by `CategoryService`. This
/// service composes every richer inherited category layer before content reaches
/// a transport and derives output pagination from one allowed sequence.
pub struct ForumCategoryAudienceReadService {
    category_service: CategoryService,
    visibility: ForumCategoryAudienceVisibilityService,
}

impl ForumCategoryAudienceReadService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self::with_optional_audience_facts(db, None)
    }

    pub fn with_audience_facts(
        db: DatabaseConnection,
        facts_port: SharedForumAudienceFactsPort,
    ) -> Self {
        Self::with_optional_audience_facts(db, Some(facts_port))
    }

    fn with_optional_audience_facts(
        db: DatabaseConnection,
        facts_port: Option<SharedForumAudienceFactsPort>,
    ) -> Self {
        Self {
            category_service: CategoryService::new(db.clone()),
            visibility: ForumCategoryAudienceVisibilityService::new(db, facts_port),
        }
    }

    pub async fn get_authenticated_owner_visible_with_audience_context(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        category_id: Uuid,
        fallback_locale: Option<&str>,
    ) -> ForumResult<CategoryResponse> {
        self.get_authenticated_visible_with_audience_context(
            tenant_id,
            security,
            context,
            category_id,
            fallback_locale,
        )
        .await
    }

    pub async fn get_authenticated_storefront_visible_with_audience_context(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        category_id: Uuid,
        fallback_locale: Option<&str>,
    ) -> ForumResult<CategoryResponse> {
        self.get_authenticated_visible_with_audience_context(
            tenant_id,
            security,
            context,
            category_id,
            fallback_locale,
        )
        .await
    }

    async fn get_authenticated_visible_with_audience_context(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        category_id: Uuid,
        fallback_locale: Option<&str>,
    ) -> ForumResult<CategoryResponse> {
        enforce_scope(&security, Resource::ForumCategories, Action::Read)?;
        let locale = context_locale(&context, "selected category")?;
        let viewer = ForumCategoryAudienceViewer::authenticated(security.clone(), context)?;
        self.get_visible(
            tenant_id,
            security,
            viewer,
            category_id,
            &locale,
            fallback_locale,
        )
        .await
    }

    pub async fn get_public_storefront_visible_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<CategoryResponse> {
        self.get_visible(
            tenant_id,
            SecurityContext::public_read(),
            ForumCategoryAudienceViewer::public(),
            category_id,
            locale,
            fallback_locale,
        )
        .await
    }

    async fn get_visible(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        viewer: ForumCategoryAudienceViewer,
        category_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<CategoryResponse> {
        if !self
            .visibility
            .is_category_visible(tenant_id, category_id, &viewer)
            .await?
        {
            return Err(ForumError::CategoryNotFound(category_id));
        }
        self.category_service
            .get_with_locale_fallback(tenant_id, security, category_id, locale, fallback_locale)
            .await
    }

    pub async fn list_authenticated_owner_visible_with_audience_context(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        page: u64,
        per_page: u64,
        fallback_locale: Option<&str>,
    ) -> ForumResult<ForumCategoryAudiencePage> {
        self.list_authenticated_visible_with_audience_context(
            tenant_id,
            security,
            context,
            page,
            per_page,
            fallback_locale,
        )
        .await
    }

    pub async fn list_authenticated_storefront_visible_with_audience_context(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        page: u64,
        per_page: u64,
        fallback_locale: Option<&str>,
    ) -> ForumResult<ForumCategoryAudiencePage> {
        self.list_authenticated_visible_with_audience_context(
            tenant_id,
            security,
            context,
            page,
            per_page,
            fallback_locale,
        )
        .await
    }

    async fn list_authenticated_visible_with_audience_context(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        page: u64,
        per_page: u64,
        fallback_locale: Option<&str>,
    ) -> ForumResult<ForumCategoryAudiencePage> {
        enforce_scope(&security, Resource::ForumCategories, Action::List)?;
        let locale = context_locale(&context, "category list")?;
        let viewer = ForumCategoryAudienceViewer::authenticated(security.clone(), context)?;
        self.list_visible(
            tenant_id,
            security,
            viewer,
            &locale,
            page,
            per_page,
            fallback_locale,
        )
        .await
    }

    pub async fn list_public_storefront_visible_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        locale: &str,
        page: u64,
        per_page: u64,
        fallback_locale: Option<&str>,
    ) -> ForumResult<ForumCategoryAudiencePage> {
        self.list_visible(
            tenant_id,
            SecurityContext::public_read(),
            ForumCategoryAudienceViewer::public(),
            locale,
            page,
            per_page,
            fallback_locale,
        )
        .await
    }

    async fn list_visible(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        viewer: ForumCategoryAudienceViewer,
        locale: &str,
        page: u64,
        per_page: u64,
        fallback_locale: Option<&str>,
    ) -> ForumResult<ForumCategoryAudiencePage> {
        enforce_scope(&security, Resource::ForumCategories, Action::List)?;
        validate_page(page, per_page)?;

        let requested_start = page
            .saturating_sub(1)
            .checked_mul(per_page)
            .ok_or_else(|| {
                ForumError::Validation(
                    "Forum category audience page offset is too large".to_string(),
                )
            })?;
        let requested_end = requested_start.checked_add(per_page).ok_or_else(|| {
            ForumError::Validation("Forum category audience page range is too large".to_string())
        })?;

        let mut items = Vec::with_capacity(per_page as usize);
        let mut visible_total = 0_u64;
        let mut candidate_page = 1_u64;

        loop {
            let (candidates, candidate_total) = self
                .category_service
                .list_paginated_with_locale_fallback(
                    tenant_id,
                    security.clone(),
                    locale,
                    candidate_page,
                    FORUM_CATEGORY_AUDIENCE_SCAN_PAGE_SIZE,
                    fallback_locale,
                )
                .await?;

            if candidates.is_empty() {
                break;
            }

            for category in candidates {
                if self
                    .visibility
                    .is_category_visible(tenant_id, category.id, &viewer)
                    .await?
                {
                    if visible_total >= requested_start && visible_total < requested_end {
                        items.push(category);
                    }
                    visible_total = visible_total.saturating_add(1);
                }
            }

            let scanned = candidate_page.saturating_mul(FORUM_CATEGORY_AUDIENCE_SCAN_PAGE_SIZE);
            if scanned >= candidate_total {
                break;
            }
            candidate_page = candidate_page.saturating_add(1);
        }

        Ok(ForumCategoryAudiencePage {
            items,
            total: visible_total,
        })
    }

    pub async fn tree_authenticated_owner_visible_with_audience_context(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        mut query: CategoryTreeQuery,
    ) -> ForumResult<CategoryTreeResponse> {
        enforce_scope(&security, Resource::ForumCategories, Action::List)?;
        let locale = context_locale(&context, "category tree")?;
        let viewer = ForumCategoryAudienceViewer::authenticated(security.clone(), context)?;
        query.locale = Some(locale);

        let mut tree = self
            .category_service
            .tree(tenant_id, security, query)
            .await?;
        let mut category_ids = Vec::with_capacity(tree.total_nodes as usize);
        collect_category_ids(&tree.roots, &mut category_ids);
        if category_ids.len() > MAX_FORUM_CATEGORY_TREE_NODES as usize {
            return Err(ForumError::Validation(format!(
                "Forum category audience tree exceeds the bounded limit of {MAX_FORUM_CATEGORY_TREE_NODES} nodes"
            )));
        }

        let mut visible_ids = HashSet::with_capacity(category_ids.len());
        for category_id in category_ids {
            if self
                .visibility
                .is_category_visible(tenant_id, category_id, &viewer)
                .await?
            {
                visible_ids.insert(category_id);
            }
        }

        tree.roots = prune_category_nodes(tree.roots, &visible_ids);
        let (total_nodes, max_depth) = category_tree_stats(&tree.roots);
        tree.total_nodes = total_nodes;
        tree.max_depth = max_depth;
        Ok(tree)
    }
}

fn context_locale(context: &PortContext, operation: &str) -> ForumResult<String> {
    let locale = context.locale.trim();
    if locale.is_empty() {
        return Err(ForumError::Validation(format!(
            "Forum category audience {operation} context locale is unavailable"
        )));
    }
    Ok(locale.to_string())
}

fn validate_page(page: u64, per_page: u64) -> ForumResult<()> {
    if page == 0 {
        return Err(ForumError::Validation(
            "Forum category audience page must be at least 1".to_string(),
        ));
    }
    if !(1..=MAX_FORUM_READ_LIMIT).contains(&per_page) {
        return Err(ForumError::Validation(format!(
            "Forum category audience page size must be between 1 and {MAX_FORUM_READ_LIMIT}"
        )));
    }
    Ok(())
}

fn collect_category_ids(nodes: &[CategoryTreeNode], output: &mut Vec<Uuid>) {
    for node in nodes {
        output.push(node.id);
        collect_category_ids(&node.children, output);
    }
}

fn prune_category_nodes(
    nodes: Vec<CategoryTreeNode>,
    visible_ids: &HashSet<Uuid>,
) -> Vec<CategoryTreeNode> {
    nodes
        .into_iter()
        .filter_map(|mut node| {
            if !visible_ids.contains(&node.id) {
                return None;
            }
            node.children = prune_category_nodes(node.children, visible_ids);
            node.children_count = node.children.len() as u32;
            node.has_children = !node.children.is_empty();
            Some(node)
        })
        .collect()
}

fn category_tree_stats(nodes: &[CategoryTreeNode]) -> (u32, u16) {
    let mut total = 0_u32;
    let mut max_depth = 0_u16;
    for node in nodes {
        total = total.saturating_add(1);
        max_depth = max_depth.max(node.depth);
        let (child_total, child_depth) = category_tree_stats(&node.children);
        total = total.saturating_add(child_total);
        max_depth = max_depth.max(child_depth);
    }
    (total, max_depth)
}
