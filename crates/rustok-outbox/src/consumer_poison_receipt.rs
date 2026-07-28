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
pub enum ConsumerPoisonReceiptState {
    Reserved,
    Publishing,
    Published,
    Acknowledged,
}

impl ConsumerPoisonReceiptState {
    fn parse(value: &str) -> Result<Self, ConsumerPoisonReceiptError> {
        match value {
            "reserved" => Ok(Self::Reserved),
            "publishing" => Ok(Self::Publishing),
            "published" => Ok(Self::Published),
            "acknowledged" => Ok(Self::Acknowledged),
            other => Err(ConsumerPoisonReceiptError::InvalidStoredState(
                other.to_owned(),
            )),
        }
    }
}

/// Immutable connector delivery identity for bytes that cannot be trusted as a
/// decoded domain event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumerPoisonIdentity {
    pub delivery_id: Uuid,
    pub consumer_group: String,
    pub source_stream: String,
    pub source_topic: String,
    pub source_partition: u32,
    pub source_offset: u64,
    pub payload: Vec<u8>,
}

impl ConsumerPoisonIdentity {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        delivery_id: Uuid,
        consumer_group: impl Into<String>,
        source_stream: impl Into<String>,
        source_topic: impl Into<String>,
        source_partition: u32,
        source_offset: u64,
        payload: Vec<u8>,
    ) -> Result<Self, ConsumerPoisonReceiptError> {
        let consumer_group = consumer_group.into();
        let source_stream = source_stream.into();
        let source_topic = source_topic.into();
        validate_identity_part("consumer_group", &consumer_group)?;
        validate_identity_part("source_stream", &source_stream)?;
        validate_identity_part("source_topic", &source_topic)?;
        if delivery_id.is_nil() {
            return Err(ConsumerPoisonReceiptError::InvalidIdentity {
                field: "delivery_id",
                reason: "must not be nil",
            });
        }
        if source_partition == 0 {
            return Err(ConsumerPoisonReceiptError::InvalidIdentity {
                field: "source_partition",
                reason: "must be positive",
            });
        }
        if payload.is_empty() {
            return Err(ConsumerPoisonReceiptError::InvalidIdentity {
                field: "payload",
                reason: "must not be empty",
            });
        }
        source_offset_i64(source_offset)?;

        Ok(Self {
            delivery_id,
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
pub struct ConsumerPoisonReceipt {
    pub state: ConsumerPoisonReceiptState,
    pub stable_error_code: String,
    pub delivery_attempt_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsumerPoisonPublishClaim {
    Claimed,
    Busy,
    AlreadyPublished,
    AlreadyAcknowledged,
}

#[derive(Debug, Error)]
pub enum ConsumerPoisonReceiptError {
    #[error("invalid consumer poison identity field {field}: {reason}")]
    InvalidIdentity {
        field: &'static str,
        reason: &'static str,
    },
    #[error("consumer poison source coordinates were reused for different bytes or identity")]
    IdentityConflict,
    #[error("stored consumer poison receipt state is invalid: {0}")]
    InvalidStoredState(String),
    #[error("consumer poison publish claim was lost")]
    ClaimLost,
    #[error("consumer poison receipt storage failed")]
    Storage(String),
}

impl ConsumerPoisonReceiptError {
    pub const fn stable_code(&self) -> &'static str {
        match self {
            Self::InvalidIdentity { .. } => "outbox.consumer_poison_receipt_invalid",
            Self::IdentityConflict => "outbox.consumer_poison_identity_conflict",
            Self::InvalidStoredState(_) => "outbox.consumer_poison_state_invalid",
            Self::ClaimLost => "outbox.consumer_poison_claim_lost",
            Self::Storage(_) => "outbox.consumer_poison_storage_failed",
        }
    }

    pub const fn is_retryable(&self) -> bool {
        matches!(self, Self::ClaimLost | Self::Storage(_))
    }
}

/// Durable neutral result store for malformed broker deliveries.
///
/// It owns persistence and state transitions only. It does not publish to a broker,
/// acknowledge a source cursor, decode tenant/event identity, or choose consumer policy.
#[derive(Clone)]
pub struct ConsumerPoisonReceiptStore {
    db: DatabaseConnection,
}

impl ConsumerPoisonReceiptStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn find(
        &self,
        identity: &ConsumerPoisonIdentity,
    ) -> Result<Option<ConsumerPoisonReceipt>, ConsumerPoisonReceiptError> {
        let backend = self.db.get_database_backend();
        ensure_supported_backend(backend)?;
        let row = self
            .db
            .query_one(Statement::from_sql_and_values(
                backend,
                select_receipt_sql(backend, false),
                receipt_key_values(identity),
            ))
            .await
            .map_err(storage_error)?;
        row.map(|row| decode_and_validate_receipt(&row, identity, backend))
            .transpose()
    }

    pub async fn reserve_and_claim(
        &self,
        identity: &ConsumerPoisonIdentity,
        stable_error_code: &str,
        delivery_attempt_count: u32,
        publisher_id: Uuid,
        lease_duration: Duration,
    ) -> Result<ConsumerPoisonPublishClaim, ConsumerPoisonReceiptError> {
        validate_identity_part("stable_error_code", stable_error_code)?;
        if delivery_attempt_count == 0 {
            return Err(ConsumerPoisonReceiptError::InvalidIdentity {
                field: "delivery_attempt_count",
                reason: "must be positive",
            });
        }
        if publisher_id.is_nil() {
            return Err(ConsumerPoisonReceiptError::InvalidIdentity {
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
                delivery_attempt_count,
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
        identity: &ConsumerPoisonIdentity,
        stable_error_code: &str,
        delivery_attempt_count: u32,
        publisher_id: Uuid,
        lease_seconds: u64,
    ) -> Result<ConsumerPoisonPublishClaim, ConsumerPoisonReceiptError> {
        let backend = transaction.get_database_backend();
        ensure_supported_backend(backend)?;
        transaction
            .execute(Statement::from_sql_and_values(
                backend,
                insert_receipt_sql(backend),
                vec![
                    uuid_value(identity.delivery_id, backend),
                    identity.consumer_group.clone().into(),
                    identity.source_stream.clone().into(),
                    identity.source_topic.clone().into(),
                    i64::from(identity.source_partition).into(),
                    source_offset_value(identity.source_offset)?,
                    identity.payload.clone().into(),
                    stable_error_code.to_owned().into(),
                    i64::from(delivery_attempt_count).into(),
                ],
            ))
            .await
            .map_err(storage_error)?;

        let row = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                select_receipt_sql(backend, true),
                receipt_key_values(identity),
            ))
            .await
            .map_err(storage_error)?
            .ok_or_else(|| {
                ConsumerPoisonReceiptError::Storage(
                    "reserved consumer poison receipt disappeared before claim".to_owned(),
                )
            })?;
        let receipt = decode_and_validate_receipt(&row, identity, backend)?;
        if receipt.stable_error_code != stable_error_code
            || receipt.delivery_attempt_count != delivery_attempt_count
        {
            return Err(ConsumerPoisonReceiptError::IdentityConflict);
        }
        match receipt.state {
            ConsumerPoisonReceiptState::Published => {
                return Ok(ConsumerPoisonPublishClaim::AlreadyPublished);
            }
            ConsumerPoisonReceiptState::Acknowledged => {
                return Ok(ConsumerPoisonPublishClaim::AlreadyAcknowledged);
            }
            ConsumerPoisonReceiptState::Reserved | ConsumerPoisonReceiptState::Publishing => {}
        }

        let mut values = vec![
            uuid_value(publisher_id, backend),
            i64::try_from(lease_seconds)
                .map_err(|_| ConsumerPoisonReceiptError::InvalidIdentity {
                    field: "lease_duration",
                    reason: "is out of range",
                })?
                .into(),
        ];
        values.extend(receipt_key_values(identity));
        let claimed = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                claim_receipt_sql(backend),
                values,
            ))
            .await
            .map_err(storage_error)?;
        if claimed.rows_affected() == 1 {
            return Ok(ConsumerPoisonPublishClaim::Claimed);
        }

        let current = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                select_receipt_sql(backend, true),
                receipt_key_values(identity),
            ))
            .await
            .map_err(storage_error)?
            .ok_or_else(|| {
                ConsumerPoisonReceiptError::Storage(
                    "consumer poison receipt disappeared after failed claim".to_owned(),
                )
            })?;
        let current = decode_and_validate_receipt(&current, identity, backend)?;
        Ok(match current.state {
            ConsumerPoisonReceiptState::Published => {
                ConsumerPoisonPublishClaim::AlreadyPublished
            }
            ConsumerPoisonReceiptState::Acknowledged => {
                ConsumerPoisonPublishClaim::AlreadyAcknowledged
            }
            ConsumerPoisonReceiptState::Reserved | ConsumerPoisonReceiptState::Publishing => {
                ConsumerPoisonPublishClaim::Busy
            }
        })
    }

    pub async fn release_claim(
        &self,
        identity: &ConsumerPoisonIdentity,
        publisher_id: Uuid,
    ) -> Result<(), ConsumerPoisonReceiptError> {
        let backend = self.db.get_database_backend();
        ensure_supported_backend(backend)?;
        let mut values = vec![uuid_value(publisher_id, backend)];
        values.extend(receipt_key_values(identity));
        self.db
            .execute(Statement::from_sql_and_values(
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
        identity: &ConsumerPoisonIdentity,
        publisher_id: Uuid,
    ) -> Result<(), ConsumerPoisonReceiptError> {
        let backend = self.db.get_database_backend();
        ensure_supported_backend(backend)?;
        let mut values = vec![uuid_value(publisher_id, backend)];
        values.extend(receipt_key_values(identity));
        let updated = self
            .db
            .execute(Statement::from_sql_and_values(
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
            Some(ConsumerPoisonReceiptState::Published | ConsumerPoisonReceiptState::Acknowledged) => {
                Ok(())
            }
            _ => Err(ConsumerPoisonReceiptError::ClaimLost),
        }
    }

    pub async fn mark_acknowledged(
        &self,
        identity: &ConsumerPoisonIdentity,
    ) -> Result<(), ConsumerPoisonReceiptError> {
        let backend = self.db.get_database_backend();
        ensure_supported_backend(backend)?;
        let updated = self
            .db
            .execute(Statement::from_sql_and_values(
                backend,
                mark_acknowledged_sql(backend),
                receipt_key_values(identity),
            ))
            .await
            .map_err(storage_error)?;
        if updated.rows_affected() == 1 {
            return Ok(());
        }
        match self.find(identity).await?.map(|receipt| receipt.state) {
            Some(ConsumerPoisonReceiptState::Acknowledged) => Ok(()),
            _ => Err(ConsumerPoisonReceiptError::ClaimLost),
        }
    }
}

fn decode_and_validate_receipt(
    row: &sea_orm::QueryResult,
    identity: &ConsumerPoisonIdentity,
    backend: DbBackend,
) -> Result<ConsumerPoisonReceipt, ConsumerPoisonReceiptError> {
    let stored_delivery_id = stored_uuid(row, "delivery_id", backend)?;
    let stored_group: String = row.try_get("", "consumer_group").map_err(storage_error)?;
    let stored_stream: String = row.try_get("", "source_stream").map_err(storage_error)?;
    let stored_topic: String = row.try_get("", "source_topic").map_err(storage_error)?;
    let stored_partition: i64 = row
        .try_get("", "source_partition")
        .map_err(storage_error)?;
    let stored_offset: i64 = row.try_get("", "source_offset").map_err(storage_error)?;
    let stored_payload: Vec<u8> = row.try_get("", "payload").map_err(storage_error)?;
    if stored_delivery_id != identity.delivery_id
        || stored_group != identity.consumer_group
        || stored_stream != identity.source_stream
        || stored_topic != identity.source_topic
        || stored_partition != i64::from(identity.source_partition)
        || stored_offset != source_offset_i64(identity.source_offset)?
        || stored_payload != identity.payload
    {
        return Err(ConsumerPoisonReceiptError::IdentityConflict);
    }
    let state: String = row.try_get("", "state").map_err(storage_error)?;
    let stable_error_code: String = row
        .try_get("", "stable_error_code")
        .map_err(storage_error)?;
    let delivery_attempt_count: i64 = row
        .try_get("", "delivery_attempt_count")
        .map_err(storage_error)?;
    let delivery_attempt_count = u32::try_from(delivery_attempt_count).map_err(|_| {
        ConsumerPoisonReceiptError::Storage(
            "stored consumer poison delivery attempt count is invalid".to_owned(),
        )
    })?;
    Ok(ConsumerPoisonReceipt {
        state: ConsumerPoisonReceiptState::parse(&state)?,
        stable_error_code,
        delivery_attempt_count,
    })
}

fn validate_identity_part(
    field: &'static str,
    value: &str,
) -> Result<(), ConsumerPoisonReceiptError> {
    if value.is_empty() {
        return Err(ConsumerPoisonReceiptError::InvalidIdentity {
            field,
            reason: "must not be empty",
        });
    }
    if value.trim() != value {
        return Err(ConsumerPoisonReceiptError::InvalidIdentity {
            field,
            reason: "must not have surrounding whitespace",
        });
    }
    if value.len() > MAX_IDENTITY_BYTES {
        return Err(ConsumerPoisonReceiptError::InvalidIdentity {
            field,
            reason: "exceeds the durable receipt limit",
        });
    }
    if value.chars().any(char::is_control) {
        return Err(ConsumerPoisonReceiptError::InvalidIdentity {
            field,
            reason: "must not contain control characters",
        });
    }
    Ok(())
}

fn validate_lease_duration(
    lease_duration: Duration,
) -> Result<u64, ConsumerPoisonReceiptError> {
    if lease_duration.subsec_nanos() != 0 {
        return Err(ConsumerPoisonReceiptError::InvalidIdentity {
            field: "lease_duration",
            reason: "must be a whole number of seconds",
        });
    }
    let seconds = lease_duration.as_secs();
    if seconds == 0 || seconds > MAX_LEASE_SECONDS {
        return Err(ConsumerPoisonReceiptError::InvalidIdentity {
            field: "lease_duration",
            reason: "must be between 1 and 86400 seconds",
        });
    }
    Ok(seconds)
}

fn ensure_supported_backend(
    backend: DbBackend,
) -> Result<(), ConsumerPoisonReceiptError> {
    match backend {
        DbBackend::Postgres => Ok(()),
        DbBackend::Sqlite if cfg!(test) => Ok(()),
        backend => Err(ConsumerPoisonReceiptError::Storage(format!(
            "consumer poison receipts do not support {backend:?}"
        ))),
    }
}

fn storage_error(error: impl std::fmt::Display) -> ConsumerPoisonReceiptError {
    ConsumerPoisonReceiptError::Storage(error.to_string())
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
) -> Result<Uuid, ConsumerPoisonReceiptError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(storage_error),
        DbBackend::Sqlite => {
            let value: String = row.try_get("", column).map_err(storage_error)?;
            Uuid::parse_str(&value).map_err(storage_error)
        }
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn source_offset_i64(offset: u64) -> Result<i64, ConsumerPoisonReceiptError> {
    i64::try_from(offset).map_err(|_| ConsumerPoisonReceiptError::InvalidIdentity {
        field: "source_offset",
        reason: "exceeds the durable receipt range",
    })
}

fn source_offset_value(offset: u64) -> Result<SqlValue, ConsumerPoisonReceiptError> {
    Ok(source_offset_i64(offset)?.into())
}

fn receipt_key_values(identity: &ConsumerPoisonIdentity) -> Vec<SqlValue> {
    vec![
        identity.consumer_group.clone().into(),
        identity.source_stream.clone().into(),
        identity.source_topic.clone().into(),
        i64::from(identity.source_partition).into(),
        source_offset_i64(identity.source_offset)
            .expect("validated poison source offset must fit i64")
            .into(),
    ]
}

fn insert_receipt_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "INSERT INTO sys_consumer_poison_receipts (delivery_id, consumer_group, source_stream, source_topic, source_partition, source_offset, payload, stable_error_code, delivery_attempt_count, state) VALUES ({prefix}1, {prefix}2, {prefix}3, {prefix}4, {prefix}5, {prefix}6, {prefix}7, {prefix}8, {prefix}9, 'reserved') ON CONFLICT DO NOTHING"
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
        "SELECT delivery_id, consumer_group, source_stream, source_topic, source_partition, source_offset, payload, stable_error_code, delivery_attempt_count, state FROM sys_consumer_poison_receipts WHERE consumer_group = {prefix}1 AND source_stream = {prefix}2 AND source_topic = {prefix}3 AND source_partition = {prefix}4 AND source_offset = {prefix}5 LIMIT 1{lock}"
    )
}

fn claim_receipt_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let lease_expires = lease_expires_expression(backend, 2);
    format!(
        "UPDATE sys_consumer_poison_receipts SET state = 'publishing', publisher_id = {prefix}1, lease_expires_at = {lease_expires}, updated_at = CURRENT_TIMESTAMP WHERE consumer_group = {prefix}3 AND source_stream = {prefix}4 AND source_topic = {prefix}5 AND source_partition = {prefix}6 AND source_offset = {prefix}7 AND (state = 'reserved' OR (state = 'publishing' AND lease_expires_at <= CURRENT_TIMESTAMP))"
    )
}

fn release_claim_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE sys_consumer_poison_receipts SET state = 'reserved', publisher_id = NULL, lease_expires_at = NULL, updated_at = CURRENT_TIMESTAMP WHERE publisher_id = {prefix}1 AND consumer_group = {prefix}2 AND source_stream = {prefix}3 AND source_topic = {prefix}4 AND source_partition = {prefix}5 AND source_offset = {prefix}6 AND state = 'publishing'"
    )
}

fn mark_published_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE sys_consumer_poison_receipts SET state = 'published', publisher_id = NULL, lease_expires_at = NULL, published_at = COALESCE(published_at, CURRENT_TIMESTAMP), updated_at = CURRENT_TIMESTAMP WHERE publisher_id = {prefix}1 AND consumer_group = {prefix}2 AND source_stream = {prefix}3 AND source_topic = {prefix}4 AND source_partition = {prefix}5 AND source_offset = {prefix}6 AND state = 'publishing'"
    )
}

fn mark_acknowledged_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE sys_consumer_poison_receipts SET state = 'acknowledged', acknowledged_at = COALESCE(acknowledged_at, CURRENT_TIMESTAMP), updated_at = CURRENT_TIMESTAMP WHERE consumer_group = {prefix}1 AND source_stream = {prefix}2 AND source_topic = {prefix}3 AND source_partition = {prefix}4 AND source_offset = {prefix}5 AND state IN ('published', 'acknowledged')"
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

    fn identity(delivery_id: Uuid, payload: Vec<u8>) -> ConsumerPoisonIdentity {
        ConsumerPoisonIdentity::new(
            delivery_id,
            "rustok-social-graph-index",
            "rustok",
            "domain",
            1,
            42,
            payload,
        )
        .unwrap()
    }

    async fn sqlite_store() -> ConsumerPoisonReceiptStore {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(
            "CREATE TABLE sys_consumer_poison_receipts (\
                delivery_id TEXT PRIMARY KEY,\
                consumer_group TEXT NOT NULL,\
                source_stream TEXT NOT NULL,\
                source_topic TEXT NOT NULL,\
                source_partition INTEGER NOT NULL,\
                source_offset INTEGER NOT NULL,\
                payload BLOB NOT NULL,\
                stable_error_code TEXT NOT NULL,\
                delivery_attempt_count INTEGER NOT NULL,\
                state TEXT NOT NULL,\
                publisher_id TEXT,\
                lease_expires_at TEXT,\
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                published_at TEXT,\
                acknowledged_at TEXT,\
                UNIQUE (consumer_group, source_stream, source_topic, source_partition, source_offset)\
            );",
        )
        .await
        .unwrap();
        ConsumerPoisonReceiptStore::new(db)
    }

    #[test]
    fn identity_rejects_nil_delivery_and_missing_payload() {
        assert!(
            ConsumerPoisonIdentity::new(
                Uuid::nil(),
                "group",
                "stream",
                "topic",
                1,
                1,
                vec![1],
            )
            .is_err()
        );
        assert!(
            ConsumerPoisonIdentity::new(
                Uuid::new_v4(),
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

    #[tokio::test]
    async fn claim_publish_and_acknowledge_are_idempotent() {
        let store = sqlite_store().await;
        let identity = identity(Uuid::new_v4(), vec![1, 2, 3]);
        let first_publisher = Uuid::new_v4();
        let second_publisher = Uuid::new_v4();
        assert_eq!(
            store
                .reserve_and_claim(
                    &identity,
                    "iggy.contract.decode_invalid",
                    1,
                    first_publisher,
                    Duration::from_secs(30),
                )
                .await
                .unwrap(),
            ConsumerPoisonPublishClaim::Claimed
        );
        assert_eq!(
            store
                .reserve_and_claim(
                    &identity,
                    "iggy.contract.decode_invalid",
                    1,
                    second_publisher,
                    Duration::from_secs(30),
                )
                .await
                .unwrap(),
            ConsumerPoisonPublishClaim::Busy
        );
        store
            .mark_published(&identity, first_publisher)
            .await
            .unwrap();
        assert_eq!(
            store
                .reserve_and_claim(
                    &identity,
                    "iggy.contract.decode_invalid",
                    1,
                    second_publisher,
                    Duration::from_secs(30),
                )
                .await
                .unwrap(),
            ConsumerPoisonPublishClaim::AlreadyPublished
        );
        store.mark_acknowledged(&identity).await.unwrap();
        assert_eq!(
            store
                .reserve_and_claim(
                    &identity,
                    "iggy.contract.decode_invalid",
                    1,
                    second_publisher,
                    Duration::from_secs(30),
                )
                .await
                .unwrap(),
            ConsumerPoisonPublishClaim::AlreadyAcknowledged
        );
    }

    #[tokio::test]
    async fn same_source_coordinates_reject_different_identity_or_bytes() {
        let store = sqlite_store().await;
        let original = identity(Uuid::new_v4(), vec![1, 2, 3]);
        store
            .reserve_and_claim(
                &original,
                "iggy.contract.decode_invalid",
                1,
                Uuid::new_v4(),
                Duration::from_secs(30),
            )
            .await
            .unwrap();
        let conflicting = identity(Uuid::new_v4(), vec![9, 9, 9]);
        assert!(matches!(
            store.find(&conflicting).await,
            Err(ConsumerPoisonReceiptError::IdentityConflict)
        ));
    }
}
