use std::time::Duration;

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait, Value as SqlValue,
};
use thiserror::Error;
use uuid::Uuid;

const MAX_IDENTITY_BYTES: usize = 191;
const MAX_LEASE_SECONDS: u64 = 86_400;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocialGraphIndexDlqReceiptState {
    Reserved,
    Publishing,
    Published,
    Acknowledged,
}

impl SocialGraphIndexDlqReceiptState {
    fn parse(value: &str) -> Result<Self, SocialGraphIndexDlqReceiptError> {
        match value {
            "reserved" => Ok(Self::Reserved),
            "publishing" => Ok(Self::Publishing),
            "published" => Ok(Self::Published),
            "acknowledged" => Ok(Self::Acknowledged),
            other => Err(SocialGraphIndexDlqReceiptError::InvalidStoredState(
                other.to_owned(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocialGraphIndexDlqIdentity {
    pub tenant_id: Uuid,
    pub event_id: Uuid,
    pub consumer_group: String,
    pub source_stream: String,
    pub source_topic: String,
    pub source_partition: u32,
    pub source_offset: u64,
    pub payload: Vec<u8>,
}

impl SocialGraphIndexDlqIdentity {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tenant_id: Uuid,
        event_id: Uuid,
        consumer_group: impl Into<String>,
        source_stream: impl Into<String>,
        source_topic: impl Into<String>,
        source_partition: u32,
        source_offset: u64,
        payload: Vec<u8>,
    ) -> Result<Self, SocialGraphIndexDlqReceiptError> {
        let consumer_group = consumer_group.into();
        let source_stream = source_stream.into();
        let source_topic = source_topic.into();
        validate_identity_part("consumer_group", &consumer_group)?;
        validate_identity_part("source_stream", &source_stream)?;
        validate_identity_part("source_topic", &source_topic)?;
        if tenant_id.is_nil() {
            return Err(SocialGraphIndexDlqReceiptError::InvalidIdentity {
                field: "tenant_id",
                reason: "must not be nil",
            });
        }
        if event_id.is_nil() {
            return Err(SocialGraphIndexDlqReceiptError::InvalidIdentity {
                field: "event_id",
                reason: "must not be nil",
            });
        }
        if source_partition == 0 {
            return Err(SocialGraphIndexDlqReceiptError::InvalidIdentity {
                field: "source_partition",
                reason: "must be positive",
            });
        }
        if payload.is_empty() {
            return Err(SocialGraphIndexDlqReceiptError::InvalidIdentity {
                field: "payload",
                reason: "must not be empty",
            });
        }
        source_offset_i64(source_offset)?;

        Ok(Self {
            tenant_id,
            event_id,
            consumer_group,
            source_stream,
            source_topic,
            source_partition,
            source_offset,
            payload,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocialGraphIndexDlqReceipt {
    pub state: SocialGraphIndexDlqReceiptState,
    pub stable_error_code: String,
    pub projection_attempt_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocialGraphIndexDlqPublishClaim {
    Claimed,
    Busy,
    AlreadyPublished,
    AlreadyAcknowledged,
}

#[derive(Debug, Error)]
pub enum SocialGraphIndexDlqReceiptError {
    #[error("invalid Social Graph Index DLQ identity field {field}: {reason}")]
    InvalidIdentity {
        field: &'static str,
        reason: &'static str,
    },
    #[error("Social Graph Index DLQ identity was reused for different source bytes or metadata")]
    IdentityConflict,
    #[error("stored Social Graph Index DLQ receipt state is invalid: {0}")]
    InvalidStoredState(String),
    #[error("Social Graph Index DLQ publish claim was lost")]
    ClaimLost,
    #[error("Social Graph Index DLQ receipt storage failed")]
    Storage(String),
}

impl SocialGraphIndexDlqReceiptError {
    pub const fn stable_code(&self) -> &'static str {
        match self {
            Self::InvalidIdentity { .. } => "social_graph.index.dlq_receipt_invalid",
            Self::IdentityConflict => "social_graph.index.dlq_receipt_identity_conflict",
            Self::InvalidStoredState(_) => "social_graph.index.dlq_receipt_state_invalid",
            Self::ClaimLost => "social_graph.index.dlq_receipt_claim_lost",
            Self::Storage(_) => "social_graph.index.dlq_receipt_storage_failed",
        }
    }

    pub const fn is_retryable(&self) -> bool {
        matches!(self, Self::ClaimLost | Self::Storage(_))
    }
}

#[derive(Clone)]
pub struct SocialGraphIndexDlqReceiptStore {
    db: DatabaseConnection,
}

impl SocialGraphIndexDlqReceiptStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn find(
        &self,
        identity: &SocialGraphIndexDlqIdentity,
    ) -> Result<Option<SocialGraphIndexDlqReceipt>, SocialGraphIndexDlqReceiptError> {
        let backend = self.db.get_database_backend();
        ensure_supported_backend(backend)?;
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                select_receipt_sql(backend, false),
                receipt_key_values(identity, backend),
            ))
            .await
            .map_err(storage_error)?;
        row.map(|row| decode_and_validate_receipt(&row, identity, backend))
            .transpose()
    }

    pub async fn reserve_and_claim(
        &self,
        identity: &SocialGraphIndexDlqIdentity,
        stable_error_code: &str,
        projection_attempt_count: u32,
        publisher_id: Uuid,
        lease_duration: Duration,
    ) -> Result<SocialGraphIndexDlqPublishClaim, SocialGraphIndexDlqReceiptError> {
        validate_identity_part("stable_error_code", stable_error_code)?;
        if projection_attempt_count == 0 {
            return Err(SocialGraphIndexDlqReceiptError::InvalidIdentity {
                field: "projection_attempt_count",
                reason: "must be positive",
            });
        }
        if publisher_id.is_nil() {
            return Err(SocialGraphIndexDlqReceiptError::InvalidIdentity {
                field: "publisher_id",
                reason: "must not be nil",
            });
        }
        let lease_seconds = validate_lease_duration(lease_duration)?;

        let transaction = self.db.begin().await.map_err(storage_error)?;
        let result = self
            .reserve_and_claim_in_transaction(
                &transaction,
                identity,
                stable_error_code,
                projection_attempt_count,
                publisher_id,
                lease_seconds,
            )
            .await;
        match result {
            Ok(claim) => {
                transaction.commit().await.map_err(storage_error)?;
                Ok(claim)
            }
            Err(error) => {
                transaction.rollback().await.map_err(storage_error)?;
                Err(error)
            }
        }
    }

    async fn reserve_and_claim_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        identity: &SocialGraphIndexDlqIdentity,
        stable_error_code: &str,
        projection_attempt_count: u32,
        publisher_id: Uuid,
        lease_seconds: u64,
    ) -> Result<SocialGraphIndexDlqPublishClaim, SocialGraphIndexDlqReceiptError> {
        let backend = transaction.get_database_backend();
        ensure_supported_backend(backend)?;
        let mut insert_values = receipt_key_values(identity, backend);
        insert_values.extend([
            identity.source_stream.clone().into(),
            identity.source_topic.clone().into(),
            i64::from(identity.source_partition).into(),
            source_offset_value(identity.source_offset)?,
            identity.payload.clone().into(),
            stable_error_code.to_owned().into(),
            i64::from(projection_attempt_count).into(),
        ]);
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                insert_receipt_sql(backend),
                insert_values,
            ))
            .await
            .map_err(storage_error)?;

        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                select_receipt_sql(backend, true),
                receipt_key_values(identity, backend),
            ))
            .await
            .map_err(storage_error)?
            .ok_or_else(|| {
                SocialGraphIndexDlqReceiptError::Storage(
                    "reserved DLQ receipt disappeared before claim".to_owned(),
                )
            })?;
        let receipt = decode_and_validate_receipt(&row, identity, backend)?;
        if receipt.stable_error_code != stable_error_code {
            return Err(SocialGraphIndexDlqReceiptError::IdentityConflict);
        }
        match receipt.state {
            SocialGraphIndexDlqReceiptState::Published => {
                return Ok(SocialGraphIndexDlqPublishClaim::AlreadyPublished);
            }
            SocialGraphIndexDlqReceiptState::Acknowledged => {
                return Ok(SocialGraphIndexDlqPublishClaim::AlreadyAcknowledged);
            }
            SocialGraphIndexDlqReceiptState::Reserved
            | SocialGraphIndexDlqReceiptState::Publishing => {}
        }

        let mut claim_values = vec![
            uuid_value(publisher_id, backend),
            i64::try_from(lease_seconds)
                .map_err(|_| SocialGraphIndexDlqReceiptError::InvalidIdentity {
                    field: "lease_duration",
                    reason: "is out of range",
                })?
                .into(),
        ];
        claim_values.extend(receipt_key_values(identity, backend));
        let claimed = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                claim_receipt_sql(backend),
                claim_values,
            ))
            .await
            .map_err(storage_error)?;
        if claimed.rows_affected() == 1 {
            return Ok(SocialGraphIndexDlqPublishClaim::Claimed);
        }

        let current = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                select_receipt_sql(backend, true),
                receipt_key_values(identity, backend),
            ))
            .await
            .map_err(storage_error)?
            .ok_or_else(|| {
                SocialGraphIndexDlqReceiptError::Storage(
                    "DLQ receipt disappeared after failed claim".to_owned(),
                )
            })?;
        let current = decode_and_validate_receipt(&current, identity, backend)?;
        Ok(match current.state {
            SocialGraphIndexDlqReceiptState::Published => {
                SocialGraphIndexDlqPublishClaim::AlreadyPublished
            }
            SocialGraphIndexDlqReceiptState::Acknowledged => {
                SocialGraphIndexDlqPublishClaim::AlreadyAcknowledged
            }
            SocialGraphIndexDlqReceiptState::Reserved
            | SocialGraphIndexDlqReceiptState::Publishing => SocialGraphIndexDlqPublishClaim::Busy,
        })
    }

    pub async fn release_claim(
        &self,
        identity: &SocialGraphIndexDlqIdentity,
        publisher_id: Uuid,
    ) -> Result<(), SocialGraphIndexDlqReceiptError> {
        let backend = self.db.get_database_backend();
        ensure_supported_backend(backend)?;
        let mut values = vec![uuid_value(publisher_id, backend)];
        values.extend(receipt_key_values(identity, backend));
        self.db
            .execute_raw(Statement::from_sql_and_values(
                backend,
                release_claim_sql(backend),
                values,
            ))
            .await
            .map_err(storage_error)?;
        Ok(())
    }

    pub async fn mark_published(
        &self,
        identity: &SocialGraphIndexDlqIdentity,
        publisher_id: Uuid,
    ) -> Result<(), SocialGraphIndexDlqReceiptError> {
        let backend = self.db.get_database_backend();
        ensure_supported_backend(backend)?;
        let mut values = vec![uuid_value(publisher_id, backend)];
        values.extend(receipt_key_values(identity, backend));
        let updated = self
            .db
            .execute_raw(Statement::from_sql_and_values(
                backend,
                mark_published_sql(backend),
                values,
            ))
            .await
            .map_err(storage_error)?;
        if updated.rows_affected() == 1 {
            return Ok(());
        }
        match self.find(identity).await?.map(|receipt| receipt.state) {
            Some(
                SocialGraphIndexDlqReceiptState::Published
                | SocialGraphIndexDlqReceiptState::Acknowledged,
            ) => Ok(()),
            _ => Err(SocialGraphIndexDlqReceiptError::ClaimLost),
        }
    }

    pub async fn mark_acknowledged(
        &self,
        identity: &SocialGraphIndexDlqIdentity,
    ) -> Result<(), SocialGraphIndexDlqReceiptError> {
        let backend = self.db.get_database_backend();
        ensure_supported_backend(backend)?;
        let updated = self
            .db
            .execute_raw(Statement::from_sql_and_values(
                backend,
                mark_acknowledged_sql(backend),
                receipt_key_values(identity, backend),
            ))
            .await
            .map_err(storage_error)?;
        if updated.rows_affected() == 1 {
            return Ok(());
        }
        match self.find(identity).await?.map(|receipt| receipt.state) {
            Some(SocialGraphIndexDlqReceiptState::Acknowledged) => Ok(()),
            _ => Err(SocialGraphIndexDlqReceiptError::ClaimLost),
        }
    }
}

fn decode_and_validate_receipt(
    row: &sea_orm::QueryResult,
    identity: &SocialGraphIndexDlqIdentity,
    backend: DbBackend,
) -> Result<SocialGraphIndexDlqReceipt, SocialGraphIndexDlqReceiptError> {
    let stored_stream: String = row.try_get("", "source_stream").map_err(storage_error)?;
    let stored_topic: String = row.try_get("", "source_topic").map_err(storage_error)?;
    let stored_partition: i64 = row.try_get("", "source_partition").map_err(storage_error)?;
    let stored_offset: i64 = row.try_get("", "source_offset").map_err(storage_error)?;
    let stored_payload: Vec<u8> = row.try_get("", "payload").map_err(storage_error)?;
    let stored_tenant = stored_uuid(row, "tenant_id", backend)?;
    let stored_event = stored_uuid(row, "event_id", backend)?;
    let stored_group: String = row.try_get("", "consumer_group").map_err(storage_error)?;
    if stored_tenant != identity.tenant_id
        || stored_event != identity.event_id
        || stored_group != identity.consumer_group
        || stored_stream != identity.source_stream
        || stored_topic != identity.source_topic
        || stored_partition != i64::from(identity.source_partition)
        || stored_offset != source_offset_i64(identity.source_offset)?
        || stored_payload != identity.payload
    {
        return Err(SocialGraphIndexDlqReceiptError::IdentityConflict);
    }
    let state: String = row.try_get("", "state").map_err(storage_error)?;
    let stable_error_code: String = row
        .try_get("", "stable_error_code")
        .map_err(storage_error)?;
    let projection_attempt_count: i64 = row
        .try_get("", "projection_attempt_count")
        .map_err(storage_error)?;
    let projection_attempt_count = u32::try_from(projection_attempt_count).map_err(|_| {
        SocialGraphIndexDlqReceiptError::Storage(
            "stored DLQ projection attempt count is invalid".to_owned(),
        )
    })?;
    Ok(SocialGraphIndexDlqReceipt {
        state: SocialGraphIndexDlqReceiptState::parse(&state)?,
        stable_error_code,
        projection_attempt_count,
    })
}

fn validate_identity_part(
    field: &'static str,
    value: &str,
) -> Result<(), SocialGraphIndexDlqReceiptError> {
    if value.is_empty() {
        return Err(SocialGraphIndexDlqReceiptError::InvalidIdentity {
            field,
            reason: "must not be empty",
        });
    }
    if value.trim() != value {
        return Err(SocialGraphIndexDlqReceiptError::InvalidIdentity {
            field,
            reason: "must not have surrounding whitespace",
        });
    }
    if value.len() > MAX_IDENTITY_BYTES {
        return Err(SocialGraphIndexDlqReceiptError::InvalidIdentity {
            field,
            reason: "exceeds the durable receipt limit",
        });
    }
    if value.chars().any(char::is_control) {
        return Err(SocialGraphIndexDlqReceiptError::InvalidIdentity {
            field,
            reason: "must not contain control characters",
        });
    }
    Ok(())
}

fn validate_lease_duration(
    lease_duration: Duration,
) -> Result<u64, SocialGraphIndexDlqReceiptError> {
    if lease_duration.subsec_nanos() != 0 {
        return Err(SocialGraphIndexDlqReceiptError::InvalidIdentity {
            field: "lease_duration",
            reason: "must be a whole number of seconds",
        });
    }
    let seconds = lease_duration.as_secs();
    if seconds == 0 || seconds > MAX_LEASE_SECONDS {
        return Err(SocialGraphIndexDlqReceiptError::InvalidIdentity {
            field: "lease_duration",
            reason: "must be between 1 and 86400 seconds",
        });
    }
    Ok(seconds)
}

fn ensure_supported_backend(backend: DbBackend) -> Result<(), SocialGraphIndexDlqReceiptError> {
    match backend {
        DbBackend::Postgres => Ok(()),
        DbBackend::Sqlite if cfg!(test) => Ok(()),
        backend => Err(SocialGraphIndexDlqReceiptError::Storage(format!(
            "Social Graph Index DLQ receipts do not support {backend:?}"
        ))),
}
}

fn storage_error(error: impl std::fmt::Display) -> SocialGraphIndexDlqReceiptError {
    SocialGraphIndexDlqReceiptError::Storage(error.to_string())
}

fn placeholder_prefix(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => "$",
        DbBackend::Sqlite => "?",
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn uuid_value(value: Uuid, backend: DbBackend) -> SqlValue {
    match backend {
        DbBackend::Postgres => value.into(),
        DbBackend::Sqlite => value.to_string().into(),
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn stored_uuid(
    row: &sea_orm::QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<Uuid, SocialGraphIndexDlqReceiptError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(storage_error),
        DbBackend::Sqlite => {
            let value: String = row.try_get("", column).map_err(storage_error)?;
            Uuid::parse_str(&value).map_err(storage_error)
        }
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn source_offset_i64(offset: u64) -> Result<i64, SocialGraphIndexDlqReceiptError> {
    i64::try_from(offset).map_err(|_| SocialGraphIndexDlqReceiptError::InvalidIdentity {
        field: "source_offset",
        reason: "exceeds the durable receipt range",
    })
}

fn source_offset_value(offset: u64) -> Result<SqlValue, SocialGraphIndexDlqReceiptError> {
    Ok(source_offset_i64(offset)?.into())
}

fn receipt_key_values(identity: &SocialGraphIndexDlqIdentity, backend: DbBackend) -> Vec<SqlValue> {
    vec![
        uuid_value(identity.tenant_id, backend),
        identity.consumer_group.clone().into(),
        uuid_value(identity.event_id, backend),
    ]
}

fn insert_receipt_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "INSERT INTO social_graph_index_dlq_receipts (tenant_id, consumer_group, event_id, source_stream, source_topic, source_partition, source_offset, payload, stable_error_code, projection_attempt_count, state) VALUES ({prefix}1, {prefix}2, {prefix}3, {prefix}4, {prefix}5, {prefix}6, {prefix}7, {prefix}8, {prefix}9, {prefix}10, 'reserved') ON CONFLICT (tenant_id, consumer_group, event_id) DO NOTHING"
    )
}

fn select_receipt_sql(backend: DbBackend, lock: bool) -> String {
    let prefix = placeholder_prefix(backend);
    let lock = if lock && backend == DbBackend::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    format!(
        "SELECT tenant_id, consumer_group, event_id, source_stream, source_topic, source_partition, source_offset, payload, stable_error_code, projection_attempt_count, state FROM social_graph_index_dlq_receipts WHERE tenant_id = {prefix}1 AND consumer_group = {prefix}2 AND event_id = {prefix}3 LIMIT 1{lock}"
    )
}

fn claim_receipt_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let lease_expires = lease_expires_expression(backend, 2);
    format!(
        "UPDATE social_graph_index_dlq_receipts SET state = 'publishing', publisher_id = {prefix}1, lease_expires_at = {lease_expires}, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = {prefix}3 AND consumer_group = {prefix}4 AND event_id = {prefix}5 AND (state = 'reserved' OR (state = 'publishing' AND lease_expires_at <= CURRENT_TIMESTAMP))"
    )
}

fn release_claim_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE social_graph_index_dlq_receipts SET state = 'reserved', publisher_id = NULL, lease_expires_at = NULL, updated_at = CURRENT_TIMESTAMP WHERE publisher_id = {prefix}1 AND tenant_id = {prefix}2 AND consumer_group = {prefix}3 AND event_id = {prefix}4 AND state = 'publishing'"
    )
}

fn mark_published_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE social_graph_index_dlq_receipts SET state = 'published', published_at = COALESCE(published_at, CURRENT_TIMESTAMP), publisher_id = NULL, lease_expires_at = NULL, updated_at = CURRENT_TIMESTAMP WHERE publisher_id = {prefix}1 AND tenant_id = {prefix}2 AND consumer_group = {prefix}3 AND event_id = {prefix}4 AND state = 'publishing'"
    )
}

fn mark_acknowledged_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE social_graph_index_dlq_receipts SET state = 'acknowledged', acknowledged_at = COALESCE(acknowledged_at, CURRENT_TIMESTAMP), updated_at = CURRENT_TIMESTAMP WHERE tenant_id = {prefix}1 AND consumer_group = {prefix}2 AND event_id = {prefix}3 AND state IN ('published', 'acknowledged')"
    )
}

fn lease_expires_expression(backend: DbBackend, parameter: usize) -> String {
    let prefix = placeholder_prefix(backend);
    match backend {
        DbBackend::Postgres => {
            format!("CURRENT_TIMESTAMP + ({prefix}{parameter} * INTERVAL '1 second')")
        }
        DbBackend::Sqlite => {
            format!("datetime('now', '+' || {prefix}{parameter} || ' seconds')")
        }
        _ => unreachable!("unsupported database backend was validated"),
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::Database;

    use super::*;

    fn identity(tenant_id: Uuid, event_id: Uuid, payload: Vec<u8>) -> SocialGraphIndexDlqIdentity {
        SocialGraphIndexDlqIdentity::new(
            tenant_id,
            event_id,
            "rustok-social-graph-index",
            "rustok",
            "domain",
            1,
            42,
            payload,
        )
        .unwrap()
    }

    async fn sqlite_store() -> SocialGraphIndexDlqReceiptStore {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(
            "CREATE TABLE social_graph_index_dlq_receipts (\
                tenant_id TEXT NOT NULL,\
                consumer_group TEXT NOT NULL,\
                event_id TEXT NOT NULL,\
                source_stream TEXT NOT NULL,\
                source_topic TEXT NOT NULL,\
                source_partition INTEGER NOT NULL,\
                source_offset INTEGER NOT NULL,\
                payload BLOB NOT NULL,\
                stable_error_code TEXT NOT NULL,\
                projection_attempt_count INTEGER NOT NULL,\
                state TEXT NOT NULL,\
                publisher_id TEXT,\
                lease_expires_at TEXT,\
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                published_at TEXT,\
                acknowledged_at TEXT,\
                PRIMARY KEY (tenant_id, consumer_group, event_id)\
            );",
        )
        .await
        .unwrap();
        SocialGraphIndexDlqReceiptStore::new(db)
    }

    #[test]
    fn identity_rejects_missing_partition_and_payload() {
        let tenant_id = Uuid::new_v4();
        let event_id = Uuid::new_v4();
        assert!(
            SocialGraphIndexDlqIdentity::new(
                tenant_id,
                event_id,
                "group",
                "stream",
                "topic",
                0,
                1,
                vec![1],
            )
            .is_err()
        );
        assert!(
            SocialGraphIndexDlqIdentity::new(
                tenant_id,
                event_id,
                "group",
                "stream",
                "topic",
                1,
                1,
                Vec::new(),
            )
            .is_err()
        );
    }

    #[test]
    fn receipt_states_are_closed() {
        assert_eq!(
            SocialGraphIndexDlqReceiptState::parse("published").unwrap(),
            SocialGraphIndexDlqReceiptState::Published
        );
        assert!(SocialGraphIndexDlqReceiptState::parse("unknown").is_err());
    }

    #[tokio::test]
    async fn receipt_claim_publish_and_acknowledge_are_idempotent() {
        let store = sqlite_store().await;
        let identity = identity(Uuid::new_v4(), Uuid::new_v4(), vec![1, 2, 3]);
        let first_publisher = Uuid::new_v4();
        let second_publisher = Uuid::new_v4();
        assert_eq!(
            store
                .reserve_and_claim(
                    &identity,
                    "social_graph.index.envelope_invalid",
                    3,
                    first_publisher,
                    Duration::from_secs(30),
                )
                .await
                .unwrap(),
            SocialGraphIndexDlqPublishClaim::Claimed
        );
        assert_eq!(
            store
                .reserve_and_claim(
                    &identity,
                    "social_graph.index.envelope_invalid",
                    3,
                    second_publisher,
                    Duration::from_secs(30),
                )
                .await
                .unwrap(),
            SocialGraphIndexDlqPublishClaim::Busy
        );
        store
            .mark_published(&identity, first_publisher)
            .await
            .unwrap();
        assert_eq!(
            store
                .reserve_and_claim(
                    &identity,
                    "social_graph.index.envelope_invalid",
                    3,
                    second_publisher,
                    Duration::from_secs(30),
                )
                .await
                .unwrap(),
            SocialGraphIndexDlqPublishClaim::AlreadyPublished
        );
        store.mark_acknowledged(&identity).await.unwrap();
        assert_eq!(
            store
                .reserve_and_claim(
                    &identity,
                    "social_graph.index.envelope_invalid",
                    3,
                    second_publisher,
                    Duration::from_secs(30),
                )
                .await
                .unwrap(),
            SocialGraphIndexDlqPublishClaim::AlreadyAcknowledged
        );
    }

    #[tokio::test]
    async fn receipt_rejects_same_key_with_different_source_bytes() {
        let store = sqlite_store().await;
        let tenant_id = Uuid::new_v4();
        let event_id = Uuid::new_v4();
        let original = identity(tenant_id, event_id, vec![1, 2, 3]);
        store
            .reserve_and_claim(
                &original,
                "social_graph.index.envelope_invalid",
                1,
                Uuid::new_v4(),
                Duration::from_secs(30),
            )
            .await
            .unwrap();
        let conflicting = identity(tenant_id, event_id, vec![9, 9, 9]);
        assert!(matches!(
            store.find(&conflicting).await,
            Err(SocialGraphIndexDlqReceiptError::IdentityConflict)
        ));
    }
}
