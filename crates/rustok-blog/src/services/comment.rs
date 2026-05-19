use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_comments::{
    CommentListItem as DomainCommentListItem, CommentRecord as DomainCommentRecord,
    CommentStatus as DomainCommentStatus, CommentsService,
    CreateCommentInput as DomainCreateCommentInput, ListCommentsFilter as DomainListCommentsFilter,
    UpdateCommentInput as DomainUpdateCommentInput,
};
use rustok_content::PLATFORM_FALLBACK_LOCALE;
use rustok_core::{prepare_content_payload, Action, Resource, SecurityContext};
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::dto::{
    CommentListItem, CommentResponse, CreateCommentInput, ListCommentsFilter, ModerateCommentInput,
    UpdateCommentInput,
};
use crate::entities::blog_post;
use crate::error::{BlogError, BlogResult};
use crate::services::rbac::enforce_scope;

const TARGET_TYPE_BLOG_POST: &str = "blog_post";

pub struct CommentService {
    db: DatabaseConnection,
    comments: CommentsService,
    event_bus: TransactionalEventBus,
}

impl CommentService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            comments: CommentsService::new(db.clone()),
            db,
            event_bus,
        }
    }

    #[instrument(skip(self, security, input))]
    pub async fn create_comment(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        post_id: Uuid,
        input: CreateCommentInput,
    ) -> BlogResult<CommentResponse> {
        self.ensure_post_exists(tenant_id, post_id).await?;

        if security.user_id.is_none() {
            return Err(BlogError::AuthorRequired);
        }

        let locale = input.locale.clone();
        let prepared = prepare_content_payload(
            Some(&input.content_format),
            Some(&input.content),
            input.content_json.as_ref(),
            &locale,
            "Comment content",
        )
        .map_err(BlogError::validation)?;

        let txn = self.db.begin().await.map_err(BlogError::from)?;
        let comment_id = self
            .comments
            .create_comment_in_tx(
                &txn,
                tenant_id,
                security.clone(),
                DomainCreateCommentInput {
                    target_type: TARGET_TYPE_BLOG_POST.to_string(),
                    target_id: post_id,
                    locale: locale.clone(),
                    body: prepared.body,
                    body_format: prepared.format,
                    parent_comment_id: input.parent_comment_id,
                    status: DomainCommentStatus::Pending,
                },
            )
            .await
            .map_err(BlogError::from)?;

        self.adjust_post_reply_count_in_tx(&txn, tenant_id, post_id, 1)
            .await?;
        self.publish_post_updated_event_in_tx(&txn, tenant_id, security.user_id, post_id)
            .await?;
        txn.commit().await.map_err(BlogError::from)?;

        let record = self
            .comments
            .get_comment(tenant_id, security, comment_id, &locale, None)
            .await
            .map_err(BlogError::from)?;
        Self::map_comment_record(record)
    }

    #[instrument(skip(self))]
    pub async fn get_comment(
        &self,
        tenant_id: Uuid,
        comment_id: Uuid,
        locale: &str,
    ) -> BlogResult<CommentResponse> {
        self.get_comment_with_locale_fallback(tenant_id, comment_id, locale, None)
            .await
    }

    #[instrument(skip(self))]
    pub async fn get_comment_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        comment_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> BlogResult<CommentResponse> {
        let record = self
            .comments
            .get_comment(
                tenant_id,
                SecurityContext::system(),
                comment_id,
                locale,
                fallback_locale,
            )
            .await
            .map_err(BlogError::from)?;
        Self::map_comment_record(record)
    }

    #[instrument(skip(self, security, input))]
    pub async fn update_comment(
        &self,
        tenant_id: Uuid,
        comment_id: Uuid,
        security: SecurityContext,
        input: UpdateCommentInput,
    ) -> BlogResult<CommentResponse> {
        let locale = input.locale.clone();
        let domain_input = if input.content.is_some()
            || input.content_json.is_some()
            || input.content_format.is_some()
        {
            let prepared = prepare_content_payload(
                input.content_format.as_deref(),
                input.content.as_deref(),
                input.content_json.as_ref(),
                &locale,
                "Comment content",
            )
            .map_err(BlogError::validation)?;

            DomainUpdateCommentInput {
                locale: locale.clone(),
                body: Some(prepared.body),
                body_format: Some(prepared.format),
            }
        } else {
            DomainUpdateCommentInput {
                locale: locale.clone(),
                body: None,
                body_format: None,
            }
        };

        let record = self
            .comments
            .update_comment(tenant_id, security, comment_id, domain_input)
            .await
            .map_err(BlogError::from)?;
        Self::map_comment_record(record)
    }

    #[instrument(skip(self, security, input))]
    pub async fn moderate_comment(
        &self,
        tenant_id: Uuid,
        comment_id: Uuid,
        security: SecurityContext,
        input: ModerateCommentInput,
        fallback_locale: Option<&str>,
    ) -> BlogResult<CommentResponse> {
        enforce_scope(&security, Resource::BlogPosts, Action::Manage)?;

        let locale = input
            .locale
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(PLATFORM_FALLBACK_LOCALE);

        let existing = self
            .comments
            .get_comment(
                tenant_id,
                SecurityContext::system(),
                comment_id,
                locale,
                fallback_locale,
            )
            .await
            .map_err(BlogError::from)?;
        Self::ensure_blog_target(&existing)?;

        let record = self
            .comments
            .set_comment_status(
                tenant_id,
                SecurityContext::system(),
                comment_id,
                input.status.into(),
                locale,
                fallback_locale,
            )
            .await
            .map_err(BlogError::from)?;
        Self::map_comment_record(record)
    }

    #[instrument(skip(self, security))]
    pub async fn delete_comment(
        &self,
        tenant_id: Uuid,
        comment_id: Uuid,
        security: SecurityContext,
    ) -> BlogResult<()> {
        let existing = self
            .comments
            .get_comment(
                tenant_id,
                SecurityContext::system(),
                comment_id,
                PLATFORM_FALLBACK_LOCALE,
                None,
            )
            .await
            .map_err(BlogError::from)?;
        let post_id = Self::ensure_blog_target(&existing)?;

        let txn = self.db.begin().await.map_err(BlogError::from)?;
        self.comments
            .delete_comment_in_tx(&txn, tenant_id, security.clone(), comment_id)
            .await
            .map_err(BlogError::from)?;
        self.adjust_post_reply_count_in_tx(&txn, tenant_id, post_id, -1)
            .await?;
        self.publish_post_updated_event_in_tx(&txn, tenant_id, security.user_id, post_id)
            .await?;
        txn.commit().await.map_err(BlogError::from)?;

        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn list_for_post(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        post_id: Uuid,
        filter: ListCommentsFilter,
    ) -> BlogResult<(Vec<CommentListItem>, u64)> {
        self.list_for_post_with_locale_fallback(tenant_id, security, post_id, filter, None)
            .await
    }

    #[instrument(skip(self, security))]
    pub async fn list_for_post_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        post_id: Uuid,
        filter: ListCommentsFilter,
        fallback_locale: Option<&str>,
    ) -> BlogResult<(Vec<CommentListItem>, u64)> {
        self.ensure_post_exists(tenant_id, post_id).await?;

        let locale = filter
            .locale
            .clone()
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        let (items, total) = self
            .comments
            .list_comments_for_target(
                tenant_id,
                security,
                TARGET_TYPE_BLOG_POST,
                post_id,
                DomainListCommentsFilter {
                    locale: locale.clone(),
                    page: filter.page,
                    per_page: filter.per_page,
                },
                fallback_locale,
            )
            .await
            .map_err(BlogError::from)?;

        Ok((
            items
                .into_iter()
                .map(Self::map_comment_list_item)
                .collect::<Vec<_>>(),
            total,
        ))
    }

    async fn ensure_post_exists(&self, tenant_id: Uuid, post_id: Uuid) -> BlogResult<()> {
        let exists = blog_post::Entity::find_by_id(post_id)
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(BlogError::from)?
            .is_some();
        if !exists {
            return Err(BlogError::post_not_found(post_id));
        }
        Ok(())
    }

    fn ensure_blog_target(record: &DomainCommentRecord) -> BlogResult<Uuid> {
        if record.target_type != TARGET_TYPE_BLOG_POST {
            return Err(BlogError::comment_not_found(record.id));
        }
        Ok(record.target_id)
    }

    async fn adjust_post_reply_count_in_tx(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        tenant_id: Uuid,
        post_id: Uuid,
        delta: i32,
    ) -> BlogResult<()> {
        let post = blog_post::Entity::find_by_id(post_id)
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .one(txn)
            .await
            .map_err(BlogError::from)?
            .ok_or_else(|| BlogError::post_not_found(post_id))?;

        let mut active: blog_post::ActiveModel = post.clone().into();
        active.comment_count = Set((post.comment_count + delta).max(0));
        active.updated_at = Set(Utc::now().into());
        active.version = Set(post.version + 1);
        active.update(txn).await.map_err(BlogError::from)?;

        Ok(())
    }

    async fn publish_post_updated_event_in_tx(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        tenant_id: Uuid,
        actor_user_id: Option<Uuid>,
        post_id: Uuid,
    ) -> BlogResult<()> {
        self.event_bus
            .publish_in_tx(
                txn,
                tenant_id,
                actor_user_id,
                DomainEvent::BlogPostUpdated {
                    post_id,
                    locale: PLATFORM_FALLBACK_LOCALE.to_string(),
                },
            )
            .await
            .map_err(BlogError::from)?;
        Ok(())
    }

    fn map_comment_record(record: DomainCommentRecord) -> BlogResult<CommentResponse> {
        let post_id = Self::ensure_blog_target(&record)?;
        let requested_locale = record.requested_locale.clone();
        let effective_locale = record.effective_locale.clone();
        let body = record.body;
        let body_format = record.body_format;
        let content_json = if body_format == "rt_json_v1" {
            serde_json::from_str(&body).ok()
        } else {
            None
        };

        Ok(CommentResponse {
            id: record.id,
            requested_locale: requested_locale.clone(),
            locale: requested_locale,
            effective_locale,
            post_id,
            author_id: Some(record.author_id),
            content: body,
            content_format: body_format,
            content_json,
            status: comment_status_label(record.status).to_string(),
            parent_comment_id: record.parent_comment_id,
            created_at: record.created_at,
            updated_at: record.updated_at,
        })
    }

    fn map_comment_list_item(item: DomainCommentListItem) -> CommentListItem {
        CommentListItem {
            id: item.id,
            locale: item.requested_locale,
            effective_locale: item.effective_locale,
            post_id: item.target_id,
            author_id: Some(item.author_id),
            content_preview: item.body_preview,
            status: comment_status_label(item.status).to_string(),
            parent_comment_id: item.parent_comment_id,
            created_at: item.created_at,
        }
    }
}

fn comment_status_label(status: DomainCommentStatus) -> &'static str {
    match status {
        DomainCommentStatus::Pending => "pending",
        DomainCommentStatus::Approved => "approved",
        DomainCommentStatus::Spam => "spam",
        DomainCommentStatus::Trash => "trash",
    }
}

#[cfg(test)]
mod rich_content_tests {
    use super::*;
    use rustok_comments::CommentRecord;

    #[test]
    fn map_comment_record_extracts_rt_json_content_json() {
        let rich = serde_json::json!({"version":"rt_json_v1","locale":"en","doc":{"type":"doc","content":[]}});
        let record = CommentRecord {
            id: Uuid::new_v4(),
            thread_id: Uuid::new_v4(),
            target_type: TARGET_TYPE_BLOG_POST.to_string(),
            target_id: Uuid::new_v4(),
            requested_locale: "en".into(),
            effective_locale: "en".into(),
            author_id: Uuid::new_v4(),
            parent_comment_id: None,
            body: rich.to_string(),
            body_format: "rt_json_v1".into(),
            status: DomainCommentStatus::Pending,
            position: 1,
            created_at: "2024-01-01T00:00:00Z".into(),
            updated_at: "2024-01-01T00:00:00Z".into(),
        };

        let response = CommentService::map_comment_record(record).expect("mapping should succeed");

        assert_eq!(response.content_format, "rt_json_v1");
        assert_eq!(response.content_json, Some(rich));
    }
}
