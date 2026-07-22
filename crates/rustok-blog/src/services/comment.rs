use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_api::{PLATFORM_FALLBACK_LOCALE, PortActor, PortContext, PortError, PortErrorKind};
use rustok_comments::{
    CommentListItem as DomainCommentListItem, CommentRecord as DomainCommentRecord,
    CommentStatus as DomainCommentStatus, CommentsThreadPort,
    CreateCommentInput as DomainCreateCommentInput, ListCommentsFilter as DomainListCommentsFilter,
    SetCommentStatusRequest, UpdateCommentInput as DomainUpdateCommentInput,
    in_process_comments_thread_port,
};
use rustok_core::{SecurityActorKind, SecurityContext, prepare_content_payload};
use rustok_outbox::TransactionalEventBus;
use std::sync::Arc;

use crate::dto::{
    CommentListItem, CommentResponse, CreateCommentInput, ListCommentsFilter, ModerateCommentInput,
    UpdateCommentInput,
};
use crate::entities::blog_post;
use crate::error::{BlogError, BlogResult};
use crate::services::rbac::enforce_scope;

const TARGET_TYPE_BLOG_POST: &str = "blog_post";
const PUBLIC_COMMENTS_PORT_ACTOR: &str = "rustok-blog.public-comments";

pub struct CommentService {
    db: DatabaseConnection,
    comments_thread_port: Arc<dyn CommentsThreadPort>,
}

impl CommentService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            comments_thread_port: in_process_comments_thread_port(db.clone(), event_bus),
            db,
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

        // A create call has no comment id yet. Use a per-command nonce so two
        // independent comments on the same post never share an idempotency key.
        let command_id = Uuid::new_v4();
        let record = self
            .comments_thread_port
            .create_comment(
                comments_write_port_context(
                    tenant_id,
                    &security,
                    locale.as_str(),
                    "create",
                    post_id,
                    command_id,
                )?,
                DomainCreateCommentInput {
                    target_type: TARGET_TYPE_BLOG_POST.to_string(),
                    target_id: post_id,
                    locale,
                    body: prepared.body,
                    body_format: prepared.format,
                    parent_comment_id: input.parent_comment_id,
                    status: DomainCommentStatus::Pending,
                },
            )
            .await
            .map_err(comments_port_error_to_blog_error)?;
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
            .comments_thread_port
            .get_comment(
                comments_read_port_context(
                    tenant_id,
                    &SecurityContext::system(),
                    locale,
                    comment_id,
                )?,
                comment_id,
                fallback_locale.map(str::to_owned),
            )
            .await
            .map_err(comments_port_error_to_blog_error)?;
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
            .comments_thread_port
            .update_comment(
                comments_write_port_context(
                    tenant_id,
                    &security,
                    locale.as_str(),
                    "update",
                    comment_id,
                    comment_id,
                )?,
                comment_id,
                domain_input,
            )
            .await
            .map_err(comments_port_error_to_blog_error)?;
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
            .comments_thread_port
            .get_comment(
                comments_read_port_context(
                    tenant_id,
                    &SecurityContext::system(),
                    locale,
                    comment_id,
                )?,
                comment_id,
                fallback_locale.map(str::to_owned),
            )
            .await
            .map_err(comments_port_error_to_blog_error)?;
        Self::ensure_blog_target(&existing)?;

        let record = self
            .comments_thread_port
            .set_comment_status(
                comments_write_port_context(
                    tenant_id,
                    &SecurityContext::system(),
                    locale,
                    "moderate",
                    comment_id,
                    comment_id,
                )?,
                comment_id,
                SetCommentStatusRequest {
                    status: input.status.into(),
                    fallback_locale: fallback_locale.map(str::to_owned),
                },
            )
            .await
            .map_err(comments_port_error_to_blog_error)?;
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
            .comments_thread_port
            .get_comment(
                comments_read_port_context(
                    tenant_id,
                    &SecurityContext::system(),
                    PLATFORM_FALLBACK_LOCALE,
                    comment_id,
                )?,
                comment_id,
                None,
            )
            .await
            .map_err(comments_port_error_to_blog_error)?;
        Self::ensure_blog_target(&existing)?;

        self.comments_thread_port
            .delete_comment(
                comments_write_port_context(
                    tenant_id,
                    &security,
                    PLATFORM_FALLBACK_LOCALE,
                    "delete",
                    comment_id,
                    comment_id,
                )?,
                comment_id,
            )
            .await
            .map_err(comments_port_error_to_blog_error)?;

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
        let domain_filter = DomainListCommentsFilter {
            locale: locale.clone(),
            page: filter.page,
            per_page: filter.per_page,
        };

        let result = if security.is_public_read() {
            self.comments_thread_port
                .list_public_comments_for_target(
                    comments_public_read_port_context(tenant_id, locale.as_str(), post_id),
                    TARGET_TYPE_BLOG_POST.to_string(),
                    post_id,
                    domain_filter,
                    fallback_locale.map(str::to_owned),
                )
                .await
        } else {
            self.comments_thread_port
                .list_comments_for_target(
                    comments_read_port_context(tenant_id, &security, locale.as_str(), post_id)?,
                    TARGET_TYPE_BLOG_POST.to_string(),
                    post_id,
                    domain_filter,
                    fallback_locale.map(str::to_owned),
                )
                .await
        };

        let (items, total) = result.map_err(comments_port_error_to_blog_error)?;
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

fn comments_write_port_context(
    tenant_id: Uuid,
    security: &SecurityContext,
    locale: &str,
    operation: &str,
    resource_id: Uuid,
    command_id: Uuid,
) -> BlogResult<PortContext> {
    comments_port_context(
        tenant_id,
        security,
        locale,
        operation,
        resource_id,
        Some(command_id),
    )
}

fn comments_read_port_context(
    tenant_id: Uuid,
    security: &SecurityContext,
    locale: &str,
    resource_id: Uuid,
) -> BlogResult<PortContext> {
    comments_port_context(tenant_id, security, locale, "read", resource_id, None)
}

fn comments_public_read_port_context(tenant_id: Uuid, locale: &str, post_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service(PUBLIC_COMMENTS_PORT_ACTOR),
        locale,
        format!("blog-comment:public-list:{post_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2))
}

fn comments_port_context(
    tenant_id: Uuid,
    security: &SecurityContext,
    locale: &str,
    operation: &str,
    resource_id: Uuid,
    command_id: Option<Uuid>,
) -> BlogResult<PortContext> {
    let actor = match security.actor_kind {
        SecurityActorKind::System => PortActor::system(),
        SecurityActorKind::User => PortActor::user(
            security
                .user_id
                .ok_or(BlogError::AuthorRequired)?
                .to_string(),
        ),
        SecurityActorKind::Service if command_id.is_none() => PortActor::service(
            security
                .user_id
                .ok_or_else(|| BlogError::forbidden("service comment reads require an actor id"))?
                .to_string(),
        ),
        SecurityActorKind::Service | SecurityActorKind::Public => {
            return Err(BlogError::forbidden(
                "comment writes require an authenticated user or system actor",
            ));
        }
    };

    let correlation_id = format!("blog-comment:{operation}:{resource_id}");
    let mut context =
        PortContext::new(tenant_id.to_string(), actor, locale, correlation_id.clone())
            .with_deadline(std::time::Duration::from_secs(2));
    if let Some(command_id) = command_id {
        context = context.with_idempotency_key(format!("{correlation_id}:command:{command_id}"));
    }

    if security.actor_kind != SecurityActorKind::System {
        context = context.with_role(security.role.to_string());
        for permission in security.permissions() {
            context = context.with_claim(permission.to_string());
        }
    }

    Ok(context)
}

fn comments_port_error_to_blog_error(error: PortError) -> BlogError {
    let kind = match error.kind {
        PortErrorKind::Validation => rustok_core::error::ErrorKind::Validation,
        PortErrorKind::NotFound => rustok_core::error::ErrorKind::NotFound,
        PortErrorKind::Conflict => rustok_core::error::ErrorKind::Conflict,
        PortErrorKind::Forbidden => rustok_core::error::ErrorKind::Forbidden,
        PortErrorKind::Unavailable => rustok_core::error::ErrorKind::ExternalService,
        PortErrorKind::Timeout => rustok_core::error::ErrorKind::Timeout,
        PortErrorKind::InvariantViolation => rustok_core::error::ErrorKind::Internal,
    };
    BlogError::Rich(Box::new(
        rustok_core::error::RichError::new(kind, error.message).with_error_code(error.code),
    ))
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
