use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use uuid::Uuid;

use crate::dto::{
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

    async fn delete_comment(&self, context: PortContext, comment_id: Uuid)
    -> Result<(), PortError>;
}

#[cfg(feature = "server")]
mod in_process {
    use super::*;
    use async_trait::async_trait;
    use rustok_api::{PortActor, PortCallPolicy, PortErrorKind};
    use rustok_core::SecurityContext;
    use rustok_outbox::{
        TransactionalEventBus,
        idempotency::{self, Admission, OwnerOperationScope},
    };
    use sea_orm::{DatabaseConnection, TransactionTrait};
    use std::sync::Arc;

    use crate::{CommentsError, CommentsService};

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
            request: CreateCommentInput,
        ) -> Result<CommentRecord, PortError> {
            context.require_policy(PortCallPolicy::write())?;
            if request.target_type.trim().is_empty() {
                return Err(PortError::validation(
                    "comments.invalid_target_type",
                    "target_type cannot be empty",
                ));
            }
            if request.target_id.is_nil() {
                return Err(PortError::validation(
                    "comments.invalid_target_id",
                    "target_id cannot be nil",
                ));
            }
            if request.parent_comment_id.is_some_and(|id| id.is_nil()) {
                return Err(PortError::validation(
                    "comments.invalid_parent_comment_id",
                    "parent_comment_id cannot be nil",
                ));
            }
            let tenant_id = parse_tenant_id(&context)?;
            let security = SecurityContext::try_from_port_context(&context)?;
            let idempotency_key = required_idempotency_key(&context)?;
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
                    return Ok(record);
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
                .create_comment_record_in_tx(&txn, tenant_id, security, request)
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
                    Ok(record)
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
        ) -> Result<CommentRecord, PortError> {
            context.require_policy(PortCallPolicy::read())?;
            if comment_id.is_nil() {
                return Err(PortError::validation(
                    "comments.invalid_comment_id",
                    "comment_id cannot be nil",
                ));
            }
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
            if target_type.trim().is_empty() {
                return Err(PortError::validation(
                    "comments.invalid_target_type",
                    "target_type cannot be empty",
                ));
            }
            if target_id.is_nil() {
                return Err(PortError::validation(
                    "comments.invalid_target_id",
                    "target_id cannot be nil",
                ));
            }
            let tenant_id = parse_tenant_id(&context)?;
            self.service
                .list_comments_for_target(
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
            if target_type.trim().is_empty() {
                return Err(PortError::validation(
                    "comments.invalid_target_type",
                    "target_type cannot be empty",
                ));
            }
            if target_id.is_nil() {
                return Err(PortError::validation(
                    "comments.invalid_target_id",
                    "target_id cannot be nil",
                ));
            }
            let tenant_id = parse_tenant_id(&context)?;
            crate::public_read::list_public_comments_for_target(
                &self.db,
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
            if comment_id.is_nil() {
                return Err(PortError::validation(
                    "comments.invalid_comment_id",
                    "comment_id cannot be nil",
                ));
            }
            let tenant_id = parse_tenant_id(&context)?;
            let security = SecurityContext::try_from_port_context(&context)?;
            let idempotency_key = required_idempotency_key(&context)?;
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
                    return Ok(record);
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
                .update_comment_in_tx(&txn, tenant_id, security, comment_id, request)
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
                    Ok(record)
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
            if comment_id.is_nil() {
                return Err(PortError::validation(
                    "comments.invalid_comment_id",
                    "comment_id cannot be nil",
                ));
            }
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
            request: SetCommentStatusRequest,
        ) -> Result<CommentRecord, PortError> {
            context.require_policy(PortCallPolicy::write())?;
            if comment_id.is_nil() {
                return Err(PortError::validation(
                    "comments.invalid_comment_id",
                    "comment_id cannot be nil",
                ));
            }
            let tenant_id = parse_tenant_id(&context)?;
            let security = SecurityContext::try_from_port_context(&context)?;
            let idempotency_key = required_idempotency_key(&context)?;
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
                    return Ok(record);
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
                    request.status,
                    &context.locale,
                    request.fallback_locale.as_deref(),
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
                    Ok(record)
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
        // Only deterministic failures may be replayed as terminal receipts.
        // A transient DB/provider outage, timeout or uncertain commit must
        // release its *processing* lease so that the same command_id can retry.
        // The Outbox release checks both status and lease token, so it cannot
        // erase a concurrently completed write or another worker's claim.
        let result = if error.retryable {
            idempotency::release_retryable(database, lease, error).await
        } else {
            idempotency::fail(database, lease, error).await
        };
        if let Err(receipt_error) = result {
            tracing::error!(
                operation_id = %lease.operation_id,
                %error,
                %receipt_error,
                "Failed to finalize Comments failure receipt safely"
            );
        }
    }

    fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
        let tenant_id = Uuid::parse_str(&context.tenant_id).map_err(|_| {
            PortError::validation(
                "comments.invalid_tenant_id",
                "comments port context must carry a UUID tenant_id",
            )
        })?;
        if tenant_id.is_nil() {
            return Err(PortError::validation(
                "comments.invalid_tenant_id",
                "comments port context tenant_id cannot be nil",
            ));
        }
        Ok(tenant_id)
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
            CommentsError::Validation(message) => {
                PortError::validation("comments.validation", message)
            }
        }
    }

    #[cfg(test)]
    mod idempotency_tests {
        use super::*;

        #[test]
        fn idempotency_request_binds_principal_identity() {
            let request = serde_json::json!({
                "target_id": Uuid::nil(),
                "body": "same request",
            });
            let user_a = Uuid::new_v4();
            let user_b = Uuid::new_v4();
            let tenant_id = Uuid::new_v4().to_string();

            let context_a = PortContext::new(
                tenant_id.clone(),
                PortActor::user(user_a.to_string()),
                "en",
                "corr-a",
            );
            let context_a_retry = PortContext::new(
                tenant_id.clone(),
                PortActor::user(user_a.to_string()),
                "en",
                "corr-b",
            );
            let context_b = PortContext::new(
                tenant_id,
                PortActor::user(user_b.to_string()),
                "en",
                "corr-c",
            );

            let first = serde_json::to_value(bind_idempotency_actor(&context_a, &request))
                .expect("first receipt identity should serialize");
            let retry = serde_json::to_value(bind_idempotency_actor(&context_a_retry, &request))
                .expect("retry receipt identity should serialize");
            let other = serde_json::to_value(bind_idempotency_actor(&context_b, &request))
                .expect("other principal receipt identity should serialize");

            assert_eq!(first, retry);
            assert_ne!(first, other);
        }
    }
}

#[cfg(feature = "server")]
pub use in_process::in_process_comments_thread_port;
