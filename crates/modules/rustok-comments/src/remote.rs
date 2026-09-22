use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    CommentListItem, CommentRecord, CommentsThreadPort, CreateCommentInput, ListCommentsFilter,
    SetCommentStatusRequest, UpdateCommentInput,
};

/// Transport-neutral wire request for a remote `CommentsThreadPort` provider.
///
/// Concrete HTTP, gRPC, message-bus, or sidecar clients implement
/// [`CommentsThreadTransport`] and remain responsible for authentication,
/// endpoint discovery, cancellation, and converting transport failures into the
/// stable `PortError` envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", content = "payload", rename_all = "snake_case")]
pub enum CommentsThreadRequest {
    CreateComment {
        context: PortContext,
        request: CreateCommentInput,
    },
    GetComment {
        context: PortContext,
        comment_id: Uuid,
        fallback_locale: Option<String>,
    },
    ListCommentsForTarget {
        context: PortContext,
        target_type: String,
        target_id: Uuid,
        filter: ListCommentsFilter,
        fallback_locale: Option<String>,
    },
    ListPublicCommentsForTarget {
        context: PortContext,
        target_type: String,
        target_id: Uuid,
        filter: ListCommentsFilter,
        fallback_locale: Option<String>,
    },
    UpdateComment {
        context: PortContext,
        comment_id: Uuid,
        request: UpdateCommentInput,
    },
    SetCommentStatus {
        context: PortContext,
        comment_id: Uuid,
        request: SetCommentStatusRequest,
    },
    DeleteComment {
        context: PortContext,
        comment_id: Uuid,
    },
}

impl CommentsThreadRequest {
    /// Returns the complete port context carried by this operation.
    pub fn context(&self) -> &PortContext {
        match self {
            Self::CreateComment { context, .. }
            | Self::GetComment { context, .. }
            | Self::ListCommentsForTarget { context, .. }
            | Self::ListPublicCommentsForTarget { context, .. }
            | Self::UpdateComment { context, .. }
            | Self::SetCommentStatus { context, .. }
            | Self::DeleteComment { context, .. } => context,
        }
    }
}

/// Transport-neutral wire response for a remote `CommentsThreadPort` provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "result", content = "payload", rename_all = "snake_case")]
pub enum CommentsThreadResponse {
    Comment(Box<CommentRecord>),
    CommentsPage {
        items: Vec<CommentListItem>,
        total: u64,
    },
    Deleted,
}

/// Stable provider reply envelope used by concrete remote transports.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", content = "payload", rename_all = "snake_case")]
pub enum CommentsThreadTransportReply {
    Success(Box<CommentsThreadResponse>),
    Error(PortError),
}

/// Executes one typed Comments request over a concrete remote transport.
///
/// Implementations must preserve the complete `PortContext`, including deadline,
/// correlation, causation, trace, claims, roles, channel, and idempotency data.
#[async_trait]
pub trait CommentsThreadTransport: Send + Sync {
    async fn execute(
        &self,
        request: CommentsThreadRequest,
    ) -> Result<CommentsThreadResponse, PortError>;
}

/// `CommentsThreadPort` adapter backed by a typed remote transport.
#[derive(Clone)]
pub struct RemoteCommentsThreadPort {
    transport: Arc<dyn CommentsThreadTransport>,
}

impl RemoteCommentsThreadPort {
    pub fn new(transport: Arc<dyn CommentsThreadTransport>) -> Self {
        Self { transport }
    }
}

/// Builds a remote Comments provider that can be injected into Blog or another
/// consumer through its existing `Arc<dyn CommentsThreadPort>` composition seam.
pub fn remote_comments_thread_port(
    transport: Arc<dyn CommentsThreadTransport>,
) -> Arc<dyn CommentsThreadPort> {
    Arc::new(RemoteCommentsThreadPort::new(transport))
}

#[async_trait]
impl CommentsThreadPort for RemoteCommentsThreadPort {
    async fn create_comment(
        &self,
        context: PortContext,
        request: CreateCommentInput,
    ) -> Result<CommentRecord, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        expect_comment(
            "create_comment",
            self.transport
                .execute(CommentsThreadRequest::CreateComment { context, request })
                .await?,
        )
    }

    async fn get_comment(
        &self,
        context: PortContext,
        comment_id: Uuid,
        fallback_locale: Option<String>,
    ) -> Result<CommentRecord, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        expect_comment(
            "get_comment",
            self.transport
                .execute(CommentsThreadRequest::GetComment {
                    context,
                    comment_id,
                    fallback_locale,
                })
                .await?,
        )
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
        expect_comments_page(
            "list_comments_for_target",
            self.transport
                .execute(CommentsThreadRequest::ListCommentsForTarget {
                    context,
                    target_type,
                    target_id,
                    filter,
                    fallback_locale,
                })
                .await?,
        )
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
        expect_comments_page(
            "list_public_comments_for_target",
            self.transport
                .execute(CommentsThreadRequest::ListPublicCommentsForTarget {
                    context,
                    target_type,
                    target_id,
                    filter,
                    fallback_locale,
                })
                .await?,
        )
    }

    async fn update_comment(
        &self,
        context: PortContext,
        comment_id: Uuid,
        request: UpdateCommentInput,
    ) -> Result<CommentRecord, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        expect_comment(
            "update_comment",
            self.transport
                .execute(CommentsThreadRequest::UpdateComment {
                    context,
                    comment_id,
                    request,
                })
                .await?,
        )
    }

    async fn set_comment_status(
        &self,
        context: PortContext,
        comment_id: Uuid,
        request: SetCommentStatusRequest,
    ) -> Result<CommentRecord, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        expect_comment(
            "set_comment_status",
            self.transport
                .execute(CommentsThreadRequest::SetCommentStatus {
                    context,
                    comment_id,
                    request,
                })
                .await?,
        )
    }

    async fn delete_comment(
        &self,
        context: PortContext,
        comment_id: Uuid,
    ) -> Result<(), PortError> {
        context.require_policy(PortCallPolicy::write())?;
        match self
            .transport
            .execute(CommentsThreadRequest::DeleteComment {
                context,
                comment_id,
            })
            .await?
        {
            CommentsThreadResponse::Deleted => Ok(()),
            response => Err(response_mismatch("delete_comment", &response)),
        }
    }
}

fn expect_comment(
    operation: &'static str,
    response: CommentsThreadResponse,
) -> Result<CommentRecord, PortError> {
    match response {
        CommentsThreadResponse::Comment(comment) => Ok(*comment),
        response => Err(response_mismatch(operation, &response)),
    }
}

fn expect_comments_page(
    operation: &'static str,
    response: CommentsThreadResponse,
) -> Result<(Vec<CommentListItem>, u64), PortError> {
    match response {
        CommentsThreadResponse::CommentsPage { items, total } => Ok((items, total)),
        response => Err(response_mismatch(operation, &response)),
    }
}

fn response_mismatch(operation: &'static str, _response: &CommentsThreadResponse) -> PortError {
    PortError::invariant_violation(
        "comments.remote_response_mismatch",
        format!("comments remote transport returned an incompatible response for {operation}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_adapter_accepts_a_transport_trait_object() {
        let constructor: fn(Arc<dyn CommentsThreadTransport>) -> Arc<dyn CommentsThreadPort> =
            remote_comments_thread_port;
        let _ = constructor;
    }
}
