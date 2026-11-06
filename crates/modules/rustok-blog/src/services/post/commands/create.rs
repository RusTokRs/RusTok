use super::*;
use rustok_api::{PortError, PortErrorKind};
use rustok_outbox::idempotency::{self, Admission, OwnerOperationScope};

mod transaction;

impl PostService {
    /// One caller identity is retained across response loss. The receipt stores
    /// only the original acknowledgement, never a fresh privileged Post read.
    #[instrument(skip(self, security, input))]
    pub async fn create_post(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreatePostInput,
    ) -> BlogResult<Uuid> {
        if tenant_id.is_nil() {
            return Err(BlogError::validation("Tenant ID must not be nil"));
        }
        if input.command_id.is_nil() {
            return Err(BlogError::validation("Command ID must not be nil"));
        }
        let author = enforce_create_author(&security, Resource::BlogPosts, Action::Create)?;
        if input.publish {
            enforce_scope(&security, Resource::BlogPosts, Action::Publish)?;
        }
        let key = idempotency::scoped_caller_idempotency_key(
            "blog",
            tenant_id,
            author,
            "create_post",
            None,
            &input.command_id.to_string(),
        );
        let lease = match idempotency::admit(
            &self.db,
            OwnerOperationScope::Tenant(tenant_id),
            "blog",
            &key,
            "create_post",
            &input,
        )
        .await
        .map_err(receipt_error)?
        {
            Admission::Run(lease) => lease,
            Admission::Replay(value) => {
                let id: Uuid = serde_json::from_value(value)
                    .map_err(|_| BlogError::invariant("Invalid Blog creation receipt"))?;
                if id.is_nil() {
                    return Err(BlogError::invariant("Nil Blog creation receipt identity"));
                }
                return Ok(id);
            }
            Admission::ReplayError(error) => {
                if error.code != "blog.creation_rejected" {
                    return Err(BlogError::invariant(
                        "Invalid Blog creation failure receipt",
                    ));
                }
                let public = decode_creation_rejection(&error.message)?;
                return Err(BlogError::CreationRejected(Box::new(public)));
            }
        };
        self.run_creation(tenant_id, security, input, lease).await
    }

    async fn run_creation(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreatePostInput,
        lease: idempotency::Lease,
    ) -> BlogResult<Uuid> {
        let txn = match self.db.begin().await {
            Ok(txn) => txn,
            Err(error) => return Err(self.release_creation_failure(lease, error.into()).await),
        };
        match self
            .create_post_in_tx(&txn, tenant_id, security, input)
            .await
        {
            Ok(post_id) => {
                if let Err(error) = idempotency::complete(&txn, lease, &post_id).await {
                    txn.rollback().await?;
                    return Err(self
                        .release_creation_failure(lease, receipt_error(error))
                        .await);
                }
                if let Err(error) = txn.commit().await {
                    return Err(self.release_creation_failure(lease, error.into()).await);
                }
                Ok(post_id)
            }
            Err(error) => {
                txn.rollback().await?;
                // Infrastructure failures remain retryable, even after a
                // successful rollback; no permanent outage receipt is written.
                if !matches!(
                    error,
                    BlogError::Validation(_)
                        | BlogError::Forbidden(_)
                        | BlogError::AuthorRequired
                        | BlogError::DuplicateSlug { .. }
                        | BlogError::CategoryNotFound(_)
                        | BlogError::TaxonomyTermNotFound(_)
                        | BlogError::Conflict(_)
                ) {
                    return Err(self.release_creation_failure(lease, error).await);
                }
                let public = crate::BlogPublicError::from(error);
                let encoded = serde_json::to_string(&public)
                    .map_err(|_| BlogError::invariant("Cannot encode Blog creation rejection"))?;
                idempotency::fail(
                    &self.db,
                    lease,
                    &PortError::validation("blog.creation_rejected", encoded),
                )
                .await
                .map_err(receipt_error)?;
                Err(BlogError::CreationRejected(Box::new(public)))
            }
        }
    }

    async fn release_creation_failure(
        &self,
        lease: idempotency::Lease,
        error: BlogError,
    ) -> BlogError {
        // Called only after confirmed rollback, failed begin, or returned commit.
        // Failed rollback retains the lease for fenced stale-worker recovery.
        let retryable = PortError::unavailable("blog.creation_unknown", "Creation outcome unknown");
        match idempotency::release_retryable(&self.db, lease, &retryable).await {
            Ok(()) => error,
            Err(error) => receipt_error(error),
        }
    }
}

fn decode_creation_rejection(encoded: &str) -> BlogResult<crate::BlogPublicError> {
    let public: crate::BlogPublicError = serde_json::from_str(encoded)
        .map_err(|_| BlogError::invariant("Invalid Blog creation failure payload"))?;
    // Only the terminal owner errors finalized by run_creation are replayable
    // rejections. Corrupt success/auth/outage statuses cannot release a client's
    // retained command, even when the persisted JSON is structurally valid.
    if !matches!(public.status, 400 | 403 | 404 | 409)
        || public.code.trim().is_empty()
        || public.message.trim().is_empty()
    {
        return Err(BlogError::invariant(
            "Invalid Blog creation failure outcome",
        ));
    }
    Ok(public)
}

fn receipt_error(error: PortError) -> BlogError {
    use rustok_core::error::{ErrorKind, RichError};
    let (kind, message) = match error.kind {
        PortErrorKind::Conflict => (ErrorKind::Conflict, "Creation command has different input"),
        PortErrorKind::Validation => (ErrorKind::Validation, "Invalid creation command"),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            ErrorKind::ExternalService,
            "Creation is unconfirmed; retry the same command",
        ),
        _ => (
            ErrorKind::Internal,
            "The Blog operation could not be completed",
        ),
    };
    BlogError::Rich(Box::new(
        RichError::new(kind, "Blog creation receipt failed").with_user_message(message),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation_failure_receipt_preserves_terminal_owner_outcomes() {
        for status in [400, 403, 404, 409] {
            let public = crate::BlogPublicError {
                status,
                code: "BLOG_REJECTION".into(),
                message: "The command was rejected".into(),
            };
            let encoded = serde_json::to_string(&public).unwrap();
            assert_eq!(decode_creation_rejection(&encoded).unwrap(), public);
        }
    }

    #[test]
    fn corrupt_failure_receipt_cannot_confirm_a_rejection() {
        let public = serde_json::json!({
            "status": 400,
            "code": "BLOG_REJECTION",
            "message": "The command was rejected"
        });
        for status in [0, 200, 302, 401, 422, 429, 500, u16::MAX] {
            let mut encoded = public.clone();
            encoded["status"] = status.into();
            assert!(matches!(
                decode_creation_rejection(&encoded.to_string()),
                Err(BlogError::Invariant(_))
            ));
        }
        for field in ["code", "message"] {
            let mut encoded = public.clone();
            encoded[field] = "  ".into();
            assert!(matches!(
                decode_creation_rejection(&encoded.to_string()),
                Err(BlogError::Invariant(_))
            ));
        }
        for encoded in ["not JSON", "null", "{}"] {
            assert!(matches!(
                decode_creation_rejection(encoded),
                Err(BlogError::Invariant(_))
            ));
        }
    }
}
