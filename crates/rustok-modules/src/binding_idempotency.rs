//! Durable idempotency coordination for platform-routed artifact bindings.

use crate::data::configure_tenant_scope;
use chrono::{DateTime, Duration, Utc};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait, Value as SqlValue,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::ControlPlaneInfrastructure;

const MAX_IDEMPOTENCY_KEY_LENGTH: usize = 128;
const LEASE_SECONDS: i64 = 60;

/// Immutable identity for a single externally routed artifact binding operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactBindingIdempotencyRequest {
    pub tenant_id: Uuid,
    pub actor_id: Uuid,
    pub installation_id: Uuid,
    pub binding_id: String,
    pub idempotency_key: String,
    pub request_digest: String,
}

/// The result of claiming a durable artifact binding operation.
#[derive(Clone, Debug, PartialEq)]
pub enum ArtifactBindingIdempotencyClaim {
    Execute { operation_id: Uuid },
    Replay { response: Value },
    InProgress,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ArtifactBindingIdempotencyError {
    #[error("artifact binding idempotency request is invalid")]
    InvalidRequest,
    #[error("artifact binding idempotency key was reused for a different request")]
    Conflict,
    #[error("artifact binding idempotency response is invalid")]
    InvalidStoredResponse,
    #[error("artifact binding idempotency storage failed: {0}")]
    Storage(String),
}

/// Owner service for replaying completed binding outputs and leasing one live execution.
#[derive(Clone)]
pub struct SeaOrmArtifactBindingIdempotencyStore {
    db: DatabaseConnection,
    infrastructure: ControlPlaneInfrastructure,
}

/// Canonical digest of a host-owned binding request envelope.
pub fn artifact_binding_request_digest(
    request: &Value,
) -> Result<String, ArtifactBindingIdempotencyError> {
    let bytes = serde_json::to_vec(request).map_err(storage_error)?;
    let digest = Sha256::digest(bytes);
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(format!("sha256:{hex}"))
}

impl SeaOrmArtifactBindingIdempotencyStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self::with_infrastructure(db, ControlPlaneInfrastructure::default())
    }

    pub fn with_infrastructure(
        db: DatabaseConnection,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self { db, infrastructure }
    }

    pub async fn claim(
        &self,
        request: &ArtifactBindingIdempotencyRequest,
    ) -> Result<ArtifactBindingIdempotencyClaim, ArtifactBindingIdempotencyError> {
        validate_request(request)?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        let backend = transaction.get_database_backend();
        ensure_supported_backend(backend)?;
        configure_tenant_scope(&transaction, request.tenant_id)
            .await
            .map_err(storage_error)?;
        let existing = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                select_operation_sql(backend),
                request_values(request, backend),
            ))
            .await
            .map_err(storage_error)?;

        if let Some(existing) = existing {
            let stored_digest: String = existing
                .try_get("", "request_digest")
                .map_err(storage_error)?;
            if stored_digest != request.request_digest {
                return Err(ArtifactBindingIdempotencyError::Conflict);
            }
            let status: String = existing.try_get("", "status").map_err(storage_error)?;
            if status == "completed" {
                let response: String = existing.try_get("", "response").map_err(storage_error)?;
                let response = serde_json::from_str(&response)
                    .map_err(|_| ArtifactBindingIdempotencyError::InvalidStoredResponse)?;
                transaction.commit().await.map_err(storage_error)?;
                return Ok(ArtifactBindingIdempotencyClaim::Replay { response });
            }

            let operation_id = self.infrastructure.new_id();
            let recovered = transaction
                .execute(Statement::from_sql_and_values(
                    backend,
                    recover_operation_sql(backend),
                    vec![
                        uuid_value(operation_id, backend),
                        lease_value(self.infrastructure.now(), backend),
                        uuid_value(request.tenant_id, backend),
                        uuid_value(request.actor_id, backend),
                        uuid_value(request.installation_id, backend),
                        request.binding_id.clone().into(),
                        request.idempotency_key.clone().into(),
                    ],
                ))
                .await
                .map_err(storage_error)?;
            transaction.commit().await.map_err(storage_error)?;
            return Ok(if recovered.rows_affected() == 1 {
                ArtifactBindingIdempotencyClaim::Execute { operation_id }
            } else {
                ArtifactBindingIdempotencyClaim::InProgress
            });
        }

        let operation_id = self.infrastructure.new_id();
        let inserted = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                insert_operation_sql(backend),
                vec![
                    uuid_value(operation_id, backend),
                    uuid_value(request.tenant_id, backend),
                    uuid_value(request.actor_id, backend),
                    uuid_value(request.installation_id, backend),
                    request.binding_id.clone().into(),
                    request.idempotency_key.clone().into(),
                    request.request_digest.clone().into(),
                    lease_value(self.infrastructure.now(), backend),
                ],
            ))
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(if inserted.rows_affected() == 1 {
            ArtifactBindingIdempotencyClaim::Execute { operation_id }
        } else {
            // A competing request claimed the same unique key after the read.
            // It owns the execution; a retry will replay its completed result.
            ArtifactBindingIdempotencyClaim::InProgress
        })
    }

    pub async fn complete(
        &self,
        request: &ArtifactBindingIdempotencyRequest,
        operation_id: Uuid,
        response: &Value,
    ) -> Result<(), ArtifactBindingIdempotencyError> {
        validate_request(request)?;
        if operation_id.is_nil() {
            return Err(ArtifactBindingIdempotencyError::InvalidRequest);
        }
        let transaction = self.db.begin().await.map_err(storage_error)?;
        let backend = transaction.get_database_backend();
        ensure_supported_backend(backend)?;
        configure_tenant_scope(&transaction, request.tenant_id)
            .await
            .map_err(storage_error)?;
        let completed = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                complete_operation_sql(backend),
                vec![
                    SqlValue::Json(Some(Box::new(response.clone()))),
                    uuid_value(operation_id, backend),
                    uuid_value(request.tenant_id, backend),
                    uuid_value(request.actor_id, backend),
                    uuid_value(request.installation_id, backend),
                    request.binding_id.clone().into(),
                    request.idempotency_key.clone().into(),
                    request.request_digest.clone().into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if completed.rows_affected() != 1 {
            return Err(ArtifactBindingIdempotencyError::Storage(
                "artifact binding operation is no longer leased by this request".to_string(),
            ));
        }
        transaction.commit().await.map_err(storage_error)?;
        Ok(())
    }

    pub async fn abandon(
        &self,
        request: &ArtifactBindingIdempotencyRequest,
        operation_id: Uuid,
    ) -> Result<(), ArtifactBindingIdempotencyError> {
        validate_request(request)?;
        if operation_id.is_nil() {
            return Err(ArtifactBindingIdempotencyError::InvalidRequest);
        }
        let transaction = self.db.begin().await.map_err(storage_error)?;
        let backend = transaction.get_database_backend();
        ensure_supported_backend(backend)?;
        configure_tenant_scope(&transaction, request.tenant_id)
            .await
            .map_err(storage_error)?;
        transaction
            .execute(Statement::from_sql_and_values(
                backend,
                abandon_operation_sql(backend),
                vec![
                    uuid_value(operation_id, backend),
                    uuid_value(request.tenant_id, backend),
                    uuid_value(request.actor_id, backend),
                    uuid_value(request.installation_id, backend),
                    request.binding_id.clone().into(),
                    request.idempotency_key.clone().into(),
                    request.request_digest.clone().into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(())
    }
}

fn validate_request(
    request: &ArtifactBindingIdempotencyRequest,
) -> Result<(), ArtifactBindingIdempotencyError> {
    if request.tenant_id.is_nil()
        || request.actor_id.is_nil()
        || request.installation_id.is_nil()
        || request.binding_id.trim().is_empty()
        || request.binding_id.len() > 256
        || request.idempotency_key.trim() != request.idempotency_key
        || request.idempotency_key.is_empty()
        || request.idempotency_key.len() > MAX_IDEMPOTENCY_KEY_LENGTH
        || request.idempotency_key.chars().any(char::is_control)
        || !is_digest(&request.request_digest)
    {
        return Err(ArtifactBindingIdempotencyError::InvalidRequest);
    }
    Ok(())
}

fn is_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

fn ensure_supported_backend(backend: DbBackend) -> Result<(), ArtifactBindingIdempotencyError> {
    match backend {
        DbBackend::Postgres | DbBackend::Sqlite => Ok(()),
        backend => Err(ArtifactBindingIdempotencyError::Storage(format!(
            "artifact binding idempotency does not support {backend:?}"
        ))),
    }
}

fn request_values(
    request: &ArtifactBindingIdempotencyRequest,
    backend: DbBackend,
) -> Vec<sea_orm::Value> {
    vec![
        uuid_value(request.tenant_id, backend),
        uuid_value(request.actor_id, backend),
        uuid_value(request.installation_id, backend),
        request.binding_id.clone().into(),
        request.idempotency_key.clone().into(),
    ]
}

fn lease_value(now: DateTime<Utc>, backend: DbBackend) -> sea_orm::Value {
    let lease = now + Duration::seconds(LEASE_SECONDS);
    match backend {
        DbBackend::Postgres => lease.into(),
        DbBackend::Sqlite => lease.to_rfc3339().into(),
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn uuid_value(value: Uuid, backend: DbBackend) -> sea_orm::Value {
    match backend {
        DbBackend::Postgres => value.into(),
        DbBackend::Sqlite => value.to_string().into(),
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn select_operation_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!("SELECT request_digest, status, CAST(response AS TEXT) AS response FROM module_artifact_binding_operations WHERE tenant_id = {prefix}1 AND actor_id = {prefix}2 AND installation_id = {prefix}3 AND binding_id = {prefix}4 AND idempotency_key = {prefix}5 LIMIT 1")
}

fn recover_operation_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let expired = match backend {
        DbBackend::Postgres => "lease_expires_at <= CURRENT_TIMESTAMP",
        DbBackend::Sqlite => "datetime(lease_expires_at) <= CURRENT_TIMESTAMP",
        _ => unreachable!("unsupported database backend was validated"),
    };
    format!("UPDATE module_artifact_binding_operations SET operation_id = {prefix}1, lease_expires_at = {prefix}2 WHERE tenant_id = {prefix}3 AND actor_id = {prefix}4 AND installation_id = {prefix}5 AND binding_id = {prefix}6 AND idempotency_key = {prefix}7 AND status = 'pending' AND {expired}")
}

fn insert_operation_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!("INSERT INTO module_artifact_binding_operations (operation_id, tenant_id, actor_id, installation_id, binding_id, idempotency_key, request_digest, status, lease_expires_at) VALUES ({prefix}1, {prefix}2, {prefix}3, {prefix}4, {prefix}5, {prefix}6, {prefix}7, 'pending', {prefix}8) ON CONFLICT (tenant_id, actor_id, installation_id, binding_id, idempotency_key) DO NOTHING")
}

fn complete_operation_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!("UPDATE module_artifact_binding_operations SET status = 'completed', response = {prefix}1, completed_at = CURRENT_TIMESTAMP WHERE operation_id = {prefix}2 AND tenant_id = {prefix}3 AND actor_id = {prefix}4 AND installation_id = {prefix}5 AND binding_id = {prefix}6 AND idempotency_key = {prefix}7 AND request_digest = {prefix}8 AND status = 'pending'")
}

fn abandon_operation_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!("DELETE FROM module_artifact_binding_operations WHERE operation_id = {prefix}1 AND tenant_id = {prefix}2 AND actor_id = {prefix}3 AND installation_id = {prefix}4 AND binding_id = {prefix}5 AND idempotency_key = {prefix}6 AND request_digest = {prefix}7 AND status = 'pending'")
}

fn placeholder_prefix(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => "$",
        DbBackend::Sqlite => "?",
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn storage_error(error: impl std::fmt::Display) -> ArtifactBindingIdempotencyError {
    ArtifactBindingIdempotencyError::Storage(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Database;

    async fn store() -> SeaOrmArtifactBindingIdempotencyStore {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database");
        db.execute_unprepared(
            "CREATE TABLE module_artifact_binding_operations (\
             operation_id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, actor_id TEXT NOT NULL, \
             installation_id TEXT NOT NULL, binding_id TEXT NOT NULL, idempotency_key TEXT NOT NULL, \
             request_digest TEXT NOT NULL, status TEXT NOT NULL, response TEXT NULL, \
             lease_expires_at TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, \
             completed_at TEXT NULL, \
             UNIQUE (tenant_id, actor_id, installation_id, binding_id, idempotency_key))",
        )
        .await
        .expect("binding operation table");
        SeaOrmArtifactBindingIdempotencyStore::new(db)
    }

    fn request(tenant_id: Uuid, digest_payload: &str) -> ArtifactBindingIdempotencyRequest {
        ArtifactBindingIdempotencyRequest {
            tenant_id,
            actor_id: Uuid::new_v4(),
            installation_id: Uuid::new_v4(),
            binding_id: "admin_action".to_string(),
            idempotency_key: "stable-request-key".to_string(),
            request_digest: artifact_binding_request_digest(&serde_json::json!({
                "payload": digest_payload,
            }))
            .expect("request digest"),
        }
    }

    #[tokio::test]
    async fn completed_response_replays_for_the_exact_tenant_request() {
        let store = store().await;
        let request = request(Uuid::new_v4(), "first");
        let operation_id = match store.claim(&request).await.expect("claim") {
            ArtifactBindingIdempotencyClaim::Execute { operation_id } => operation_id,
            other => panic!("expected execution claim, got {other:?}"),
        };
        let response = serde_json::json!({ "status": "accepted" });
        store
            .complete(&request, operation_id, &response)
            .await
            .expect("complete");

        assert_eq!(
            store.claim(&request).await.expect("replay"),
            ArtifactBindingIdempotencyClaim::Replay { response }
        );
    }

    #[tokio::test]
    async fn identical_idempotency_keys_remain_tenant_scoped() {
        let store = store().await;
        let first = request(Uuid::new_v4(), "same");
        let mut second = first.clone();
        second.tenant_id = Uuid::new_v4();

        assert!(matches!(
            store.claim(&first).await.expect("first tenant claim"),
            ArtifactBindingIdempotencyClaim::Execute { .. }
        ));
        assert!(matches!(
            store.claim(&second).await.expect("second tenant claim"),
            ArtifactBindingIdempotencyClaim::Execute { .. }
        ));
    }
}
