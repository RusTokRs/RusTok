use std::ops::Deref;

use sea_orm::{DatabaseConnection, DatabaseTransaction};
use uuid::Uuid;

use rustok_core::SecurityContext;

use crate::dto::{
    bounded_forum_read_limit, CategoryListItem, CategoryTreeQuery, CategoryTreeResponse,
    MAX_FORUM_READ_LIMIT,
};
use crate::error::ForumResult;

use super::{category, category_tree};

/// Public category service with bounded compatibility reads.
///
/// Category commands and point reads continue to delegate to the raw
/// persistence service. Legacy collection methods are capped so callers cannot
/// bypass the canonical cursor or nested-tree read models with a large offset
/// page.
pub struct CategoryService {
    inner: category::CategoryService,
    tree: category_tree::CategoryTreeService,
}

impl CategoryService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            inner: category::CategoryService::new(db.clone()),
            tree: category_tree::CategoryTreeService::new(db),
        }
    }

    /// Return the complete tenant category hierarchy in one bounded owner call.
    pub async fn tree(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        query: CategoryTreeQuery,
    ) -> ForumResult<CategoryTreeResponse> {
        self.tree.read(tenant_id, security, query).await
    }

    pub async fn list(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
    ) -> ForumResult<Vec<CategoryListItem>> {
        let (items, _) = self
            .inner
            .list_paginated_with_locale_fallback(
                tenant_id,
                security,
                locale,
                1,
                MAX_FORUM_READ_LIMIT,
                None,
            )
            .await?;
        Ok(items)
    }

    pub async fn list_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<Vec<CategoryListItem>> {
        let (items, _) = self
            .inner
            .list_paginated_with_locale_fallback(
                tenant_id,
                security,
                locale,
                1,
                MAX_FORUM_READ_LIMIT,
                fallback_locale,
            )
            .await?;
        Ok(items)
    }

    pub async fn list_paginated_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
        page: u64,
        per_page: u64,
        fallback_locale: Option<&str>,
    ) -> ForumResult<(Vec<CategoryListItem>, u64)> {
        self.inner
            .list_paginated_with_locale_fallback(
                tenant_id,
                security,
                locale,
                page,
                bounded_forum_read_limit(Some(per_page)),
                fallback_locale,
            )
            .await
    }

    pub(crate) async fn find_category_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> ForumResult<crate::entities::forum_category::Model> {
        category::CategoryService::find_category_in_tx(txn, tenant_id, category_id).await
    }

    pub(crate) async fn adjust_counters_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        topic_delta: i32,
        reply_delta: i32,
    ) -> ForumResult<()> {
        category::CategoryService::adjust_counters_in_tx(
            txn,
            tenant_id,
            category_id,
            topic_delta,
            reply_delta,
        )
        .await
    }
}

impl Deref for CategoryService {
    type Target = category::CategoryService;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
