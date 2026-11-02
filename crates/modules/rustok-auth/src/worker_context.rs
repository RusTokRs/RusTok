//! Current authenticated service contexts for selected background consumers.
//! A job/event actor, saved grant set or arbitrary service label is never authority.

use std::{fmt, sync::Arc, time::Duration};

use async_trait::async_trait;
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError};
use uuid::Uuid;

/// One exact-tenant, bounded read invocation; contains no credential or saved permissions.
#[derive(Clone, Debug)]
pub struct WorkerServiceContextRequest {
    tenant_id: Uuid,
    consumer: String,
    correlation_id: Uuid,
    budget: Duration,
}

impl WorkerServiceContextRequest {
    pub fn new(
        tenant_id: Uuid,
        consumer: impl Into<String>,
        correlation_id: Uuid,
        budget: Duration,
    ) -> Result<Self, PortError> {
        let consumer = consumer.into();
        if tenant_id.is_nil()
            || correlation_id.is_nil()
            || consumer.is_empty()
            || consumer.len() > 128
            || !consumer.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
            })
            || budget < Duration::from_millis(1)
            || budget > Duration::from_secs(30)
        {
            return Err(invalid_context());
        }
        Ok(Self {
            tenant_id,
            consumer,
            correlation_id,
            budget,
        })
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn consumer(&self) -> &str {
        &self.consumer
    }

    pub fn correlation_id(&self) -> Uuid {
        self.correlation_id
    }

    pub fn budget(&self) -> Duration {
        self.budget
    }

    fn validate_context(&self, context: &PortContext) -> Result<(), PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let actor_id = Uuid::parse_str(&context.actor.id).map_err(|_| invalid_context())?;
        if context.tenant_id != self.tenant_id.to_string()
            || context.actor.kind != PortActorKind::Service
            || actor_id.is_nil()
            || context.correlation_id != self.correlation_id.to_string()
            || context.idempotency_key.is_some()
            || context
                .deadline_ms
                .is_none_or(|ms| ms == 0 || u128::from(ms) > self.budget.as_millis())
        {
            return Err(invalid_context());
        }
        Ok(())
    }
}

/// Implemented by a trusted authentication adapter which resolves credentials and current grants.
/// Implementations must not derive authority from event/job metadata or cache effective claims.
#[async_trait]
pub trait WorkerServiceContextPort: Send + Sync {
    async fn current_context(
        &self,
        request: WorkerServiceContextRequest,
    ) -> Result<PortContext, PortError>;
}

#[derive(Clone)]
pub struct WorkerServiceContextRuntime(Arc<dyn WorkerServiceContextPort>);

impl WorkerServiceContextRuntime {
    pub fn new(provider: Arc<dyn WorkerServiceContextPort>) -> Self {
        Self(provider)
    }

    pub async fn current_context(
        &self,
        request: WorkerServiceContextRequest,
    ) -> Result<PortContext, PortError> {
        let context = self.0.current_context(request.clone()).await?;
        request.validate_context(&context)?;
        Ok(context)
    }
}

impl fmt::Debug for WorkerServiceContextRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkerServiceContextRuntime")
            .finish_non_exhaustive()
    }
}

fn invalid_context() -> PortError {
    PortError::invariant_violation(
        "auth.worker_context_invalid",
        "Worker reads require an exact current authenticated service context and bounded budget",
    )
}

#[cfg(test)]
mod tests {
    use rustok_api::{Permission, PortActor};

    use super::*;

    fn request() -> WorkerServiceContextRequest {
        WorkerServiceContextRequest::new(
            Uuid::from_u128(1),
            "inventory-stock-primary",
            Uuid::from_u128(2),
            Duration::from_secs(10),
        )
        .unwrap()
    }

    fn context() -> PortContext {
        PortContext::new(
            Uuid::from_u128(1).to_string(),
            PortActor::service(Uuid::from_u128(3).to_string()),
            "und",
            Uuid::from_u128(2).to_string(),
        )
        .with_claim(Permission::INVENTORY_READ.to_string())
        .with_deadline(Duration::from_secs(9))
    }

    #[test]
    fn request_rejects_nil_identity_unbounded_budget_and_unselected_consumer_names() {
        for (tenant, consumer, correlation, budget) in [
            (
                Uuid::nil(),
                "inventory-stock-primary",
                Uuid::from_u128(2),
                1,
            ),
            (
                Uuid::from_u128(1),
                "inventory-stock-primary",
                Uuid::nil(),
                1,
            ),
            (Uuid::from_u128(1), "", Uuid::from_u128(2), 1),
            (Uuid::from_u128(1), "*", Uuid::from_u128(2), 1),
            (
                Uuid::from_u128(1),
                "inventory-stock-primary",
                Uuid::from_u128(2),
                0,
            ),
            (
                Uuid::from_u128(1),
                "inventory-stock-primary",
                Uuid::from_u128(2),
                30_001,
            ),
        ] {
            assert!(
                WorkerServiceContextRequest::new(
                    tenant,
                    consumer,
                    correlation,
                    Duration::from_millis(budget),
                )
                .is_err()
            );
        }
    }

    #[test]
    fn current_context_rejects_scope_actor_and_deadline_substitution() {
        assert!(request().validate_context(&context()).is_ok());
        for changed in [
            PortContext {
                tenant_id: Uuid::from_u128(4).to_string(),
                ..context()
            },
            PortContext {
                actor: PortActor::user(Uuid::from_u128(3).to_string()),
                ..context()
            },
            PortContext {
                actor: PortActor::service("index-worker"),
                ..context()
            },
            PortContext {
                actor: PortActor::service(Uuid::nil().to_string()),
                ..context()
            },
            PortContext {
                correlation_id: Uuid::from_u128(5).to_string(),
                ..context()
            },
            PortContext {
                deadline_ms: Some(10_001),
                ..context()
            },
            PortContext {
                idempotency_key: Some("saved-mutation-key".into()),
                ..context()
            },
        ] {
            assert!(request().validate_context(&changed).is_err());
        }
    }

    #[tokio::test]
    async fn runtime_asks_provider_again_instead_of_retaining_previous_authority() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct RevokingProvider(AtomicUsize);
        #[async_trait]
        impl WorkerServiceContextPort for RevokingProvider {
            async fn current_context(
                &self,
                _: WorkerServiceContextRequest,
            ) -> Result<PortContext, PortError> {
                if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
                    Ok(context())
                } else {
                    Err(PortError::forbidden(
                        "auth.worker_service_forbidden",
                        "Current service authority was revoked",
                    ))
                }
            }
        }
        let runtime =
            WorkerServiceContextRuntime::new(Arc::new(RevokingProvider(AtomicUsize::new(0))));
        assert!(runtime.current_context(request()).await.is_ok());
        assert_eq!(
            runtime.current_context(request()).await.unwrap_err().code,
            "auth.worker_service_forbidden"
        );
    }
}
