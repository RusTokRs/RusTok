use async_trait::async_trait;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleWorkItem {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub worker_slug: String,
    pub lease_token: String,
    pub payload: serde_json::Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModuleWorkOutcome {
    Completed,
    Retryable {
        message: String,
    },
    Rejected {
        message: String,
    },
    Cancelled,
    /// The handler could not attempt the work because a policy or state
    /// gate denied it before any execution. The item made zero progress
    /// and must not consume its failure-attempt budget: the source parks
    /// it in an eligible state so it can proceed once the gate opens.
    /// This is not a failure and must never be recorded as completed.
    Held {
        message: String,
    },
}

#[derive(Clone, Debug, thiserror::Error, Eq, PartialEq)]
pub enum ModuleWorkError {
    #[error("module work source failed: {0}")]
    Source(String),
    #[error("module work handler failed: {0}")]
    Handler(String),
    #[error("duplicate module work handler `{0}`")]
    DuplicateHandler(String),
}

/// Durable, owner-provided work queue boundary. The scheduler does not know a
/// module's tables or task types; it only claims a tenant-scoped leased item.
#[async_trait]
pub trait ModuleWorkSource: Send + Sync {
    async fn claim(&self, worker_slug: &str) -> Result<Option<ModuleWorkItem>, ModuleWorkError>;
    async fn complete(
        &self,
        item: &ModuleWorkItem,
        outcome: ModuleWorkOutcome,
    ) -> Result<(), ModuleWorkError>;
}

/// Module-owned handler for one durable work kind.
#[async_trait]
pub trait ModuleWorkHandler: Send + Sync {
    fn worker_slug(&self) -> &'static str;
    async fn execute(&self, item: ModuleWorkItem) -> Result<ModuleWorkOutcome, ModuleWorkError>;
}
