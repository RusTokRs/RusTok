use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use uuid::Uuid;

use crate::{
    CommentListItem, CommentRecord, CreateCommentInput, ListCommentsFilter,
    SetCommentStatusRequest, UpdateCommentInput,
};

/// Transport-neutral owner boundary for generic comment threads.
///
/// The Comments module owns the implementation and persistence. Consumers receive
/// only this stable contract; no consumer may construct a local Comments provider
/// or read Comments-owned tables directly.
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

    /// Public read projection owned by Comments.
    ///
    /// This operation is mandatory on every provider. Provider availability is
    /// selected at the runtime capability boundary; an implementation may not
    /// silently downgrade the contract by inheriting an unavailable stub.
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

    async fn delete_comment(
        &self,
        context: PortContext,
        comment_id: Uuid,
    ) -> Result<(), PortError>;
}
