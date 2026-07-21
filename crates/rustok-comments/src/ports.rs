use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use rustok_core::SecurityContext;
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    CommentListItem, CommentRecord, CommentsError, CommentsService, CreateCommentInput,
    ListCommentsFilter, UpdateCommentInput,
};

/// Transport-neutral owner boundary for generic comment threads.
#[async_trait]
pub trait CommentsThreadPort: Send + Sync {
    async fn create_comment(
        &self,
        context: PortContext,
        request: CreateCommentInput,
    ) -> Result<CommentRecord, PortError>;

    async fn get_comment(
        &self,
        context: PortContext,
        comment_id: Uuid,
        fallback_locale: Option<String>,
    ) -> Result<CommentRecord, PortError>;

    async fn list_comments_for_target(
        &self,
        context: PortContext,
        target_type: String,
        target_id: Uuid,
        filter: ListCommentsFilter,
        fallback_locale: Option<String>,
    ) -> Result<(Vec<CommentListItem>, u64), PortError>;

    /// Public read projection owned by Comments. Implementations must return only
    /// comments that are safe for unauthenticated storefront consumption.
    async fn list_public_comments_for_target(
        &self,
        context: PortContext,
        target_type: String,
        target_id: Uuid,
        filter: ListCommentsFilter,
        fallback_locale: Option<String>,
    ) -> Result<(Vec<CommentListItem>, u64), PortError>;

    async fn update_comment(
        &self,
        context: PortContext,
        comment_id: Uuid,
        request: UpdateCommentInput,
    ) -> Result<CommentRecord, PortError>;

    async fn set_comment_status(
        &self,
        context: PortContext,
        comment_id: Uuid,
        request: SetCommentStatusRequest,
    ) -> Result<CommentRecord, PortError>;

    async fn delete_comment(&self, context: PortContext, comment_id: Uuid)
        -> Result<(), PortError>;
}

/// Builds the owner-managed in-process comments thread provider for consumers.
pub fn in_process_comments_thread_port(
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
) -> Arc<dyn CommentsThreadPort> {
    Arc::new(CommentsService::with_event_bus(db, event_bus))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct SetCommentStatusRequest {
    pub status: crate::CommentStatus,
    pub fallback_locale: Option<String>,
}

#[async_trait]
impl CommentsThreadPort for CommentsService {
    async fn create_comment(
        &self,
        context: PortContext,
        request: CreateCommentInput,
    ) -> Result<CommentRecord, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        let tenant_id = parse_tenant_id(&context)?;
        self.create_comment(
            tenant_id,
            SecurityContext::try_from_port_context(&context)?,
            request,
        )
        .await
        .map_err(comments_error_to_port_error)
    }

    async fn get_comment(
        &self,
        context: PortContext,
        comment_id: Uuid,
        fallback_locale: Option<String>,
    ) -> Result<CommentRecord, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context)?;
        self.get_comment(
            tenant_id,
            SecurityContext::try_from_port_context(&context)?,
            comment_id,
            &context.locale,
            fallback_locale.as_deref(),
        )
        .await
        .map_err(comments_error_to_port_error)
    }

    async fn list_comments_for_target(
        &self,
        context: PortContext,
        target_type: String,
        target_id: Uuid,
        filter: ListCommentsFilter,
        fallback_locale: Option<String>,
    ) -> Result<(Vec<CommentListItem>, u64), PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context)?;
        self.list_comments_for_target(
            tenant_id,
            SecurityContext::try_from_port_context(&context)?,
            &target_type,
            target_id,
            filter,
            fallback_locale.as_deref(),
        )
        .await
        .map_err(comments_error_to_port_error)
    }

    async fn list_public_comments_for_target(
        &self,
        context: PortContext,
        target_type: String,
        target_id: Uuid,
        filter: ListCommentsFilter,
        fallback_locale: Option<String>,
    ) -> Result<(Vec<CommentListItem>, u64), PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context)?;
        crate::public_read::list_public_comments_for_target(
            &self.database,
            tenant_id,
            &target_type,
            target_id,
            filter,
            fallback_locale.as_deref(),
        )
        .await
        .map_err(comments_error_to_port_error)
    }

    async fn update_comment(
        &self,
        context: PortContext,
        comment_id: Uuid,
        request: UpdateCommentInput,
    ) -> Result<CommentRecord, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        let tenant_id = parse_tenant_id(&context)?;
        self.update_comment(
            tenant_id,
            SecurityContext::try_from_port_context(&context)?,
            comment_id,
            request,
        )
        .await
        .map_err(comments_error_to_port_error)
    }

    async fn delete_comment(
        &self,
        context: PortContext,
        comment_id: Uuid,
    ) -> Result<(), PortError> {
        context.require_policy(PortCallPolicy::write())?;
        let tenant_id = parse_tenant_id(&context)?;
        self.delete_comment(
            tenant_id,
            SecurityContext::try_from_port_context(&context)?,
            comment_id,
        )
        .await
        .map_err(comments_error_to_port_error)
    }

    async fn set_comment_status(
        &self,
        context: PortContext,
        comment_id: Uuid,
        request: SetCommentStatusRequest,
    ) -> Result<CommentRecord, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        let tenant_id = parse_tenant_id(&context)?;
        self.set_comment_status(
            tenant_id,
            SecurityContext::try_from_port_context(&context)?,
            comment_id,
            request.status,
            &context.locale,
            request.fallback_locale.as_deref(),
        )
        .await
        .map_err(comments_error_to_port_error)
    }
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "comments.invalid_tenant_id",
            "comments port context must carry a UUID tenant_id",
        )
    })
}

fn comments_error_to_port_error(error: CommentsError) -> PortError {
    match error {
        CommentsError::Database(source) => {
            PortError::unavailable("comments.database", source.to_string())
        }
        CommentsError::EventPublication(message) => {
            PortError::unavailable("comments.event_publication", message)
        }
        CommentsError::CommentNotFound(id) => PortError::new(
            PortErrorKind::NotFound,
            "comments.comment_not_found",
            format!("comment not found: {id}"),
            false,
        ),
        CommentsError::CommentThreadNotFound {
            target_type,
            target_id,
        } => PortError::new(
            PortErrorKind::NotFound,
            "comments.thread_not_found",
            format!("comment thread not found for target {target_type}:{target_id}"),
            false,
        ),
        CommentsError::CommentThreadClosed {
            target_type,
            target_id,
        } => PortError::new(
            PortErrorKind::Conflict,
            "comments.thread_closed",
            format!("comment thread is closed for target {target_type}:{target_id}"),
            false,
        ),
        CommentsError::Forbidden(message) => PortError::new(
            PortErrorKind::Forbidden,
            "comments.forbidden",
            message,
            false,
        ),
        CommentsError::Validation(message) => PortError::validation("comments.validation", message),
    }
}
