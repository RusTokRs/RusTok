use async_trait::async_trait;
use rustok_api::{PortActor, PortCallPolicy, PortContext, PortError, PortErrorKind};
use rustok_core::SecurityContext;
use rustok_outbox::{idempotency::{self, Admission, OwnerOperationScope}, TransactionalEventBus};
use sea_orm::{DatabaseConnection, TransactionTrait};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    CommentListItem, CommentRecord, CommentsError, CommentsService, CreateCommentInput,
    ListCommentsFilter, SetCommentStatusRequest, UpdateCommentInput,
};
use rustok_comments_api::{
    CommentListItem as ApiCommentListItem, CommentRecord as ApiCommentRecord,
    CommentsThreadPort, CreateCommentInput as ApiCreateCommentInput,
    ListCommentsFilter as ApiListCommentsFilter,
    SetCommentStatusRequest as ApiSetCommentStatusRequest,
    UpdateCommentInput as ApiUpdateCommentInput,
};

struct InProcessCommentsThreadProvider {
    db: DatabaseConnection,
    service: CommentsService,
}

/// Builds the owner-managed in-process comments thread provider for consumers.
pub fn in_process_comments_thread_port(
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
) -> Arc<dyn CommentsThreadPort> {
    Arc::new(InProcessCommentsThreadProvider {
        service: CommentsService::with_event_bus(db.clone(), event_bus),
        db,
    })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct SetCommentStatusRequest {
    pub status: crate::CommentStatus,
    pub fallback_locale: Option<String>,
}

#[derive(serde::Serialize)]
struct CommentsIdempotencyRequest<'a, T> {
    /// The authenticated transport principal is part of the durable receipt identity.
    /// This prevents a replay from crossing users or service actors inside one tenant.
    actor: &'a PortActor,
    request: &'a T,
}

fn bind_idempotency_actor<'a, T: serde::Serialize>(
    context: &'a PortContext,
    request: &'a T,
) -> CommentsIdempotencyRequest<'a, T> {
    CommentsIdempotencyRequest {
        actor: &context.actor,
        request,
    }
}

#[async_trait]
impl CommentsThreadPort for InProcessCommentsThreadProvider {
    async fn create_comment(
        &self,
        context: PortContext,
        request: ApiCreateCommentInput,
    ) -> Result<ApiCommentRecord, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        let tenant_id = parse_tenant_id(&context)?;
        let security = SecurityContext::try_from_port_context(&context)?;
        let idempotency_key = required_idempotency_key(&context)?;
        let domain_request: CreateCommentInput = request.clone().into();
        let receipt_request = bind_idempotency_actor(&context, &request);

        let lease = match idempotency::admit(
            &self.db,
            OwnerOperationScope::Tenant(tenant_id),
            "comments",
            idempotency_key,
            "create_comment",
            &receipt_request,
        )
        .await?
        {
            Admission::Run(lease) => lease,
            Admission::Replay(value) => {
                let record: CommentRecord = serde_json::from_value(value).map_err(|error| {
                    PortError::invariant_violation(
                        "comments.operation_receipt_corrupt",
                        error.to_string(),
                    )
                })?;
                return Ok(record.into());
            }
            Admission::ReplayError(error) => return Err(error),
        };

        let txn = match self.db.begin().await {
            Ok(txn) => txn,
            Err(error) => {
                let port_error = PortError::unavailable(
                    "comments.operation_begin_failed",
                    error.to_string(),
                );
                persist_idempotency_failure(&self.db, lease, &port_error).await;
                return Err(port_error);
            }
        };
        let result = self.service
            .create_comment_record_in_tx(&txn, tenant_id, security, domain_request)
            .await
            .map_err(comments_error_to_port_error);

        match result {
            Ok(record) => {
                if let Err(error) = idempotency::complete(&txn, lease, &record).await {
                    if let Err(rollback_error) = txn.rollback().await {
                        tracing::error!(
                            operation_id = %lease.operation_id,
                            %error,
                            %rollback_error,
                            "Failed to rollback Comments transaction after receipt completion failure: Failed to complete durable Comments create receipt"
                        );
                    }
                    persist_idempotency_failure(&self.db, lease, &error).await;
                    tracing::error!(
                        operation_id = %lease.operation_id,
                        %error,
                        "Failed to complete durable Comments create receipt"
                    );
                    return Err(error);
                }
                let commit_result = txn.commit().await.map_err(|error| {
                    PortError::unavailable(
                        "comments.operation_commit_failed",
                        error.to_string(),
                    )
                });
                if let Err(commit_error) = commit_result {
                    persist_idempotency_failure(&self.db, lease, &commit_error).await;
                    return Err(commit_error);
                }
                Ok(record.into())
            }
            Err(error) => {
                if let Err(rollback_error) = txn.rollback().await {
                    tracing::error!(
                        operation_id = %lease.operation_id,
                        %error,
                        %rollback_error,
                        "Failed to rollback Comments transaction after domain failure"
                    );
                }
                persist_idempotency_failure(&self.db, lease, &error).await;
                Err(error)
            }
        }
    }

    async fn get_comment(
        &self,
        context: PortContext,
        comment_id: Uuid,
        fallback_locale: Option<String>,
    ) -> Result<ApiCommentRecord, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context)?;
        self.service
            .get_comment(
                tenant_id,
                SecurityContext::try_from_port_context(&context)?,
                comment_id,
                &context.locale,
                fallback_locale.as_deref(),
            )
            .await
            .map_err(comments_error_to_port_error)
            .map(Into::into)
    }

    async fn list_comments_for_target(
        &self,
        context: PortContext,
        target_type: String,
        target_id: Uuid,
        filter: ApiListCommentsFilter,
        fallback_locale: Option<String>,
    ) -> Result<(Vec<ApiCommentListItem>, u64), PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context)?;
        let domain_filter: ListCommentsFilter = filter.into();
        self.service
            .list_comments_for_target(
                tenant_id,
                SecurityContext::try_from_port_context(&context)?,
                &target_type,
                target_id,
                domain_filter,
                fallback_locale.as_deref(),
            )
            .await
            .map_err(comments_error_to_port_error)
            .map(|(items, total)| (items.into_iter().map(Into::into).collect(), total))
    }

    async fn list_public_comments_for_target(
        &self,
        context: PortContext,
        target_type: String,
        target_id: Uuid,
        filter: ApiListCommentsFilter,
        fallback_locale: Option<String>,
    ) -> Result<(Vec<ApiCommentListItem>, u64), PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context)?;
        let domain_filter: ListCommentsFilter = filter.into();
        crate::public_read::list_public_comments_for_target(
            &self.db,
            tenant_id,
            &target_type,
            target_id,
            domain_filter,
            fallback_locale.as_deref(),
        )
        .await
        .map_err(comments_error_to_port_error)
        .map(|(items, total)| (items.into_iter().map(Into::into).collect(), total))
    }

    async fn update_comment(
        &self,
        context: PortContext,
        comment_id: Uuid,
        request: ApiUpdateCommentInput,
    ) -> Result<ApiCommentRecord, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        let tenant_id = parse_tenant_id(&context)?;
        let security = SecurityContext::try_from_port_context(&context)?;
        let idempotency_key = required_idempotency_key(&context)?;
        let domain_request: UpdateCommentInput = request.clone().into();
        let target_request = (comment_id, &request);
        let receipt_request = bind_idempotency_actor(&context, &target_request);

        let lease = match idempotency::admit(
            &self.db,
            OwnerOperationScope::Tenant(tenant_id),
            "comments",
            idempotency_key,
            "update_comment",
            &receipt_request,
        )
        .await?
        {
            Admission::Run(lease) => lease,
            Admission::Replay(value) => {
                let record: CommentRecord = serde_json::from_value(value).map_err(|error| {
                    PortError::invariant_violation(
                        "comments.operation_receipt_corrupt",
                        error.to_string(),
                    )
                })?;
                return Ok(record.into());
            }
            Admission::ReplayError(error) => return Err(error),
        };

        let txn = match self.db.begin().await {
            Ok(txn) => txn,
            Err(error) => {
                let port_error = PortError::unavailable(
                    "comments.operation_begin_failed",
                    error.to_string(),
                );
                persist_idempotency_failure(&self.db, lease, &port_error).await;
                return Err(port_error);
            }
        };
        let result = self
            .service
            .update_comment_in_tx(&txn, tenant_id, security, comment_id, domain_request)
            .await
            .map_err(comments_error_to_port_error);

        match result {
            Ok(record) => {
                if let Err(error) = idempotency::complete(&txn, lease, &record).await {
                    if let Err(rollback_error) = txn.rollback().await {
                        tracing::error!(
                            operation_id = %lease.operation_id,
                            %error,
                            %rollback_error,
                            "Failed to rollback Comments transaction after receipt completion failure: Failed to complete durable Comments update receipt"
                        );
                    }
                    persist_idempotency_failure(&self.db, lease, &error).await;
                    tracing::error!(
                        operation_id = %lease.operation_id,
                        %error,
                        "Failed to complete durable Comments update receipt"
                    );
                    return Err(error);
                }
                let commit_result = txn.commit().await.map_err(|error| {
                    PortError::unavailable(
                        "comments.operation_commit_failed",
                        error.to_string(),
                    )
                });
                if let Err(commit_error) = commit_result {
                    persist_idempotency_failure(&self.db, lease, &commit_error).await;
                    return Err(commit_error);
                }
                Ok(record.into())
            }
            Err(error) => {
                if let Err(rollback_error) = txn.rollback().await {
                    tracing::error!(
                        operation_id = %lease.operation_id,
                        %error,
                        %rollback_error,
                        "Failed to rollback Comments transaction after domain failure"
                    );
                }
                persist_idempotency_failure(&self.db, lease, &error).await;
                Err(error)
            }
        }
    }

    async fn delete_comment(
        &self,
        context: PortContext,
        comment_id: Uuid,
    ) -> Result<(), PortError> {
        context.require_policy(PortCallPolicy::write())?;
        let tenant_id = parse_tenant_id(&context)?;
        let security = SecurityContext::try_from_port_context(&context)?;
        let idempotency_key = required_idempotency_key(&context)?;

        let target_request = (comment_id,);
        let receipt_request = bind_idempotency_actor(&context, &target_request);
        let lease = match idempotency::admit(
            &self.db,
            OwnerOperationScope::Tenant(tenant_id),
            "comments",
            idempotency_key,
            "delete_comment",
            &receipt_request,
        )
        .await?
        {
            Admission::Run(lease) => lease,
            Admission::Replay(value) => {
                serde_json::from_value::<()>(value).map_err(|error| {
                    PortError::invariant_violation(
                        "comments.operation_receipt_corrupt",
                        error.to_string(),
                    )
                })?;
                return Ok(());
            }
            Admission::ReplayError(error) => return Err(error),
        };

        let txn = match self.db.begin().await {
            Ok(txn) => txn,
            Err(error) => {
                let port_error = PortError::unavailable(
                    "comments.operation_begin_failed",
                    error.to_string(),
                );
                persist_idempotency_failure(&self.db, lease, &port_error).await;
                return Err(port_error);
            }
        };
        let result = self
            .service
            .delete_comment_record_in_tx(&txn, tenant_id, security, comment_id)
            .await
            .map_err(comments_error_to_port_error);

        match result {
            Ok(()) => {
                if let Err(error) = idempotency::complete(&txn, lease, &()).await {
                    if let Err(rollback_error) = txn.rollback().await {
                        tracing::error!(
                            operation_id = %lease.operation_id,
                            %error,
                            %rollback_error,
                            "Failed to rollback Comments transaction after receipt completion failure: Failed to complete durable Comments delete receipt"
                        );
                    }
                    persist_idempotency_failure(&self.db, lease, &error).await;
                    tracing::error!(
                        operation_id = %lease.operation_id,
                        %error,
                        "Failed to complete durable Comments delete receipt"
                    );
                    return Err(error);
                }
                let commit_result = txn.commit().await.map_err(|error| {
                    PortError::unavailable(
                        "comments.operation_commit_failed",
                        error.to_string(),
                    )
                });
                if let Err(commit_error) = commit_result {
                    persist_idempotency_failure(&self.db, lease, &commit_error).await;
                    return Err(commit_error);
                }
                Ok(())
            }
            Err(error) => {
                if let Err(rollback_error) = txn.rollback().await {
                    tracing::error!(
                        operation_id = %lease.operation_id,
                        %error,
                        %rollback_error,
                        "Failed to rollback Comments transaction after domain failure"
                    );
                }
                persist_idempotency_failure(&self.db, lease, &error).await;
                Err(error)
            }
        }
    }

    async fn set_comment_status(
        &self,
        context: PortContext,
        comment_id: Uuid,
        request: ApiSetCommentStatusRequest,
    ) -> Result<ApiCommentRecord, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        let tenant_id = parse_tenant_id(&context)?;
        let security = SecurityContext::try_from_port_context(&context)?;
        let idempotency_key = required_idempotency_key(&context)?;
        let domain_request: SetCommentStatusRequest = request.clone().into();
        let target_request = (comment_id, &request, &context.locale);
        let receipt_request = bind_idempotency_actor(&context, &target_request);

        let lease = match idempotency::admit(
            &self.db,
            OwnerOperationScope::Tenant(tenant_id),
            "comments",
            idempotency_key,
            "set_comment_status",
            &receipt_request,
        )
        .await?
        {
            Admission::Run(lease) => lease,
            Admission::Replay(value) => {
                let record: CommentRecord = serde_json::from_value(value).map_err(|error| {
                    PortError::invariant_violation(
                        "comments.operation_receipt_corrupt",
                        error.to_string(),
                    )
                })?;
                return Ok(record.into());
            }
            Admission::ReplayError(error) => return Err(error),
        };

        let txn = match self.db.begin().await {
            Ok(txn) => txn,
            Err(error) => {
                let port_error = PortError::unavailable(
                    "comments.operation_begin_failed",
                    error.to_string(),
                );
                persist_idempotency_failure(&self.db, lease, &port_error).await;
                return Err(port_error);
            }
        };
        let result = self
            .service
            .set_comment_status_in_tx(
                &txn,
                tenant_id,
                security,
                comment_id,
                domain_request.status,
                &context.locale,
                domain_request.fallback_locale.as_deref(),
            )
            .await
            .map_err(comments_error_to_port_error);

        match result {
            Ok(record) => {
                if let Err(error) = idempotency::complete(&txn, lease, &record).await {
                    if let Err(rollback_error) = txn.rollback().await {
                        tracing::error!(
                            operation_id = %lease.operation_id,
                            %error,
                            %rollback_error,
                            "Failed to rollback Comments transaction after receipt completion failure: Failed to complete durable Comments status receipt"
                        );
                    }
                    persist_idempotency_failure(&self.db, lease, &error).await;
                    tracing::error!(
                        operation_id = %lease.operation_id,
                        %error,
                        "Failed to complete durable Comments status receipt"
                    );
                    return Err(error);
                }
                let commit_result = txn.commit().await.map_err(|error| {
                    PortError::unavailable(
                        "comments.operation_commit_failed",
                        error.to_string(),
                    )
                });
                if let Err(commit_error) = commit_result {
                    persist_idempotency_failure(&self.db, lease, &commit_error).await;
                    return Err(commit_error);
                }
                Ok(record.into())
            }
            Err(error) => {
                if let Err(rollback_error) = txn.rollback().await {
                    tracing::error!(
                        operation_id = %lease.operation_id,
                        %error,
                        %rollback_error,
                        "Failed to rollback Comments transaction after domain failure"
                    );
                }
                persist_idempotency_failure(&self.db, lease, &error).await;
                Err(error)
            }
        }
    }
}

fn required_idempotency_key(context: &PortContext) -> Result<&str, PortError> {
    context.idempotency_key.as_deref().ok_or_else(|| {
        PortError::validation(
            "port.idempotency_key_required",
            "write port calls require a non-empty idempotency key",
        )
    })
}

async fn persist_idempotency_failure(
    database: &DatabaseConnection,
    lease: idempotency::Lease,
    error: &PortError,
) {
    if let Err(receipt_error) = idempotency::fail(database, lease, error).await {
        tracing::error!(
            operation_id = %lease.operation_id,
            %error,
            %receipt_error,
            "Failed to persist durable Comments failure receipt"
        );
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
