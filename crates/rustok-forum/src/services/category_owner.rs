#![allow(dead_code)]

use std::ops::Deref;

use sea_orm::{DatabaseConnection, DatabaseTransaction};
use uuid::Uuid;

use rustok_core::SecurityContext;

use crate::dto::{
    CategoryListItem, CategoryResponse, CategorySubtreeLifecycleResponse,
    CategoryTopicPolicyResponse, CategoryTreeQuery, CategoryTreeResponse, MAX_FORUM_READ_LIMIT,
    MoveCategoryInput, MoveCategoryResponse, ReorderCategorySiblingsInput,
    ReorderCategorySiblingsResponse, UpdateCategoryInput, UpdateCategoryTopicPolicyInput,
    bounded_forum_read_limit,
};
use crate::error::{ForumError, ForumResult};

use super::{category, category_command, category_lifecycle, category_policy, category_tree};

pub struct CategoryService {
    inner: category::CategoryService,
    commands: category_command::CategoryCommandService,
    lifecycle: category_lifecycle::CategoryLifecycleService,
    policy: category_policy::CategoryTopicPolicyService,
    tree: category_tree::CategoryTreeService,
}

impl CategoryService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            inner: category::CategoryService::new(db.clone()),
            commands: category_command::CategoryCommandService::new(db.clone()),
            lifecycle: category_lifecycle::CategoryLifecycleService::new(db.clone()),
            policy: category_policy::CategoryTopicPolicyService::new(db.clone()),
            tree: category_tree::CategoryTreeService::new(db),
        }
    }

    pub async fn tree(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        query: CategoryTreeQuery,
    ) -> ForumResult<CategoryTreeResponse> {
        self.tree.read(tenant_id, security, query).await
    }

    pub async fn archive_subtree(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<CategorySubtreeLifecycleResponse> {
        self.lifecycle
            .archive_subtree(tenant_id, category_id, security)
            .await
    }

    pub async fn restore_subtree(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<CategorySubtreeLifecycleResponse> {
        self.lifecycle
            .restore_subtree(tenant_id, category_id, security)
            .await
    }

    pub async fn topic_policy(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
    ) -> ForumResult<CategoryTopicPolicyResponse> {
        self.policy.get(tenant_id, category_id, security).await
    }

    pub async fn set_topic_policy(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        input: UpdateCategoryTopicPolicyInput,
    ) -> ForumResult<CategoryTopicPolicyResponse> {
        self.policy
            .set(tenant_id, category_id, security, input)
            .await
    }

    pub async fn move_category(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        input: MoveCategoryInput,
    ) -> ForumResult<MoveCategoryResponse> {
        self.commands
            .move_category(tenant_id, category_id, security, input)
            .await
    }

    pub async fn reorder_siblings(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: ReorderCategorySiblingsInput,
    ) -> ForumResult<ReorderCategorySiblingsResponse> {
        self.commands
            .reorder_siblings(tenant_id, security, input)
            .await
    }

    /// Update category metadata without bypassing the atomic placement commands.
    pub async fn update(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        input: UpdateCategoryInput,
    ) -> ForumResult<CategoryResponse> {
        if input.position.is_some() {
            return Err(ForumError::Validation(
                "Category position must be changed through move/reorder commands".to_string(),
            ));
        }
        self.inner
            .update(tenant_id, category_id, security, input)
            .await
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
