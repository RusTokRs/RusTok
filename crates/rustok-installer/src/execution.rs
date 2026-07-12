//! Typed input and output contracts for an install-apply executor.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::InstallPlan;

/// Host-selected execution options for one install apply operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallApplyOptions {
    pub lock_owner: String,
    pub lock_ttl_secs: i64,
    pub pg_admin_url: Option<String>,
}

impl Default for InstallApplyOptions {
    fn default() -> Self {
        Self {
            lock_owner: "installer".to_string(),
            lock_ttl_secs: 900,
            pg_admin_url: None,
        }
    }
}

/// Durable result of a completed install apply operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallApplyOutput {
    pub status: String,
    pub session_id: Uuid,
    pub tenant_id: Option<Uuid>,
    pub lock_owner: Option<String>,
    pub lock_expires_at: Option<DateTime<Utc>>,
    pub preflight_receipt_id: Uuid,
    pub preflight_receipt_checksum: String,
    pub config_receipt_id: Uuid,
    pub config_receipt_checksum: String,
    pub database_receipt_id: Uuid,
    pub database_receipt_checksum: String,
    pub migrate_receipt_id: Uuid,
    pub migrate_receipt_checksum: String,
    pub seed_receipt_id: Uuid,
    pub seed_receipt_checksum: String,
    pub admin_receipt_id: Uuid,
    pub admin_receipt_checksum: String,
    pub verify_receipt_id: Uuid,
    pub verify_receipt_checksum: String,
    pub finalize_receipt_id: Uuid,
    pub finalize_receipt_checksum: String,
    pub next: Option<String>,
}

/// Boundary implemented by a host-specific installer runtime.
#[async_trait::async_trait]
pub trait InstallExecutor: Send + Sync {
    async fn apply(
        &self,
        plan: InstallPlan,
        options: InstallApplyOptions,
    ) -> Result<InstallApplyOutput, InstallExecutionError>;
}

#[derive(Debug, Error)]
#[error("install execution failed: {message}")]
pub struct InstallExecutionError {
    message: String,
}

impl InstallExecutionError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}
