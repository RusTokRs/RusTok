use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, Value as SqlValue,
};

use super::consumer_poison_receipt::ConsumerPoisonReceiptError;

const MAX_CONSUMER_GROUP_BYTES: usize = 191;

/// Bounded aggregate view of neutral poison-result progress for one consumer group.
///
/// The snapshot intentionally excludes delivery identifiers, source coordinates, exact
/// payloads, error classifications, publisher identities, and timestamps. Expired
/// publication claims are a subset of `publishing` and are reported only as a count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumerPoisonReceiptSummary {
    total: u64,
    reserved: u64,
    publishing: u64,
    expired_publishing: u64,
    published: u64,
    acknowledged: u64,
}

impl ConsumerPoisonReceiptSummary {
    pub const fn total(&self) -> u64 {
        self.total
    }

    pub const fn reserved(&self) -> u64 {
        self.reserved
    }

    pub const fn publishing(&self) -> u64 {
        self.publishing
    }

    pub const fn expired_publishing(&self) -> u64 {
        self.expired_publishing
    }

    pub const fn published(&self) -> u64 {
        self.published
    }

    pub const fn acknowledged(&self) -> u64 {
        self.acknowledged
    }

    pub const fn has_recovery_work(&self) -> bool {
        self.reserved > 0 || self.publishing > 0
    }

    pub const fn has_expired_claims(&self) -> bool {
        self.expired_publishing > 0
    }
}

/// Read-only aggregate inspector for connector-owned neutral poison receipts.
///
/// Inspection never claims, releases, publishes, acknowledges, deletes, or repairs a
/// receipt. Callers must keep any alerting policy outside this storage boundary.
#[derive(Clone)]
pub struct ConsumerPoisonReceiptInspector {
    db: DatabaseConnection,
}

impl ConsumerPoisonReceiptInspector {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn summarize(
        &self,
        consumer_group: &str,
    ) -> Result<ConsumerPoisonReceiptSummary, ConsumerPoisonReceiptError> {
        validate_consumer_group(consumer_group)?;
        let backend = self.db.get_database_backend();
        ensure_supported_backend(backend)?;
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                summary_sql(backend),
                vec![SqlValue::from(consumer_group.to_owned())],
            ))
            .await
            .map_err(storage_error)?
            .ok_or_else(|| {
                ConsumerPoisonReceiptError::Storage(
                    "consumer poison summary query returned no aggregate row".to_string(),
                )
            })?;
        decode_summary(&row)
    }
}

fn decode_summary(
    row: &QueryResult,
) -> Result<ConsumerPoisonReceiptSummary, ConsumerPoisonReceiptError> {
    let summary = ConsumerPoisonReceiptSummary {
        total: decode_count(row, "total")?,
        reserved: decode_count(row, "reserved_count")?,
        publishing: decode_count(row, "publishing_count")?,
        expired_publishing: decode_count(row, "expired_publishing_count")?,
        published: decode_count(row, "published_count")?,
        acknowledged: decode_count(row, "acknowledged_count")?,
    };
    let recognized = summary
        .reserved
        .checked_add(summary.publishing)
        .and_then(|value| value.checked_add(summary.published))
        .and_then(|value| value.checked_add(summary.acknowledged))
        .ok_or_else(|| invalid_summary("aggregate state count overflow"))?;
    if recognized != summary.total {
        return Err(invalid_summary(
            "aggregate state counts do not match the total receipt count",
        ));
    }
    if summary.expired_publishing > summary.publishing {
        return Err(invalid_summary(
            "expired publishing receipts exceed total publishing count",
        ));
    }
    Ok(summary)
}

fn decode_count(row: &QueryResult, column: &str) -> Result<u64, ConsumerPoisonReceiptError> {
    let raw: i64 = row.try_get("", column).map_err(storage_error)?;
    u64::try_from(raw).map_err(|_| invalid_summary("aggregate count is negative"))
}

fn invalid_summary(reason: &'static str) -> ConsumerPoisonReceiptError {
    ConsumerPoisonReceiptError::Storage(format!(
        "invalid consumer poison receipt summary: {reason}"
    ))
}

fn validate_consumer_group(consumer_group: &str) -> Result<(), ConsumerPoisonReceiptError> {
    if consumer_group.is_empty() {
        return Err(ConsumerPoisonReceiptError::InvalidIdentity {
            field: "consumer_group",
            reason: "must not be empty",
        });
    }
    if consumer_group.trim() != consumer_group {
        return Err(ConsumerPoisonReceiptError::InvalidIdentity {
            field: "consumer_group",
            reason: "must not have surrounding whitespace",
        });
    }
    if consumer_group.len() > MAX_CONSUMER_GROUP_BYTES {
        return Err(ConsumerPoisonReceiptError::InvalidIdentity {
            field: "consumer_group",
            reason: "exceeds the durable receipt limit",
        });
    }
    if consumer_group.chars().any(char::is_control) {
        return Err(ConsumerPoisonReceiptError::InvalidIdentity {
            field: "consumer_group",
            reason: "must not contain control characters",
        });
    }
    Ok(())
}

fn ensure_supported_backend(backend: DbBackend) -> Result<(), ConsumerPoisonReceiptError> {
    match backend {
        DbBackend::Postgres => Ok(()),
        DbBackend::Sqlite if cfg!(test) => Ok(()),
        backend => Err(ConsumerPoisonReceiptError::Storage(format!(
            "consumer poison inspection does not support {backend:?}"
        ))),
    }
}

fn summary_sql(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Sqlite => {
            "SELECT \
                COUNT(1) AS total, \
                COALESCE(SUM(CASE WHEN state = 'reserved' THEN 1 ELSE 0 END), 0) AS reserved_count, \
                COALESCE(SUM(CASE WHEN state = 'publishing' THEN 1 ELSE 0 END), 0) AS publishing_count, \
                COALESCE(SUM(CASE WHEN state = 'publishing' AND lease_expires_at IS NOT NULL AND lease_expires_at <= CURRENT_TIMESTAMP THEN 1 ELSE 0 END), 0) AS expired_publishing_count, \
                COALESCE(SUM(CASE WHEN state = 'published' THEN 1 ELSE 0 END), 0) AS published_count, \
                COALESCE(SUM(CASE WHEN state = 'acknowledged' THEN 1 ELSE 0 END), 0) AS acknowledged_count \
             FROM iggy_consumer_poison_receipts \
             WHERE consumer_group = ?1;"
        }
        DbBackend::Postgres => {
            "SELECT \
                COUNT(1)::bigint AS total, \
                COALESCE(SUM(CASE WHEN state = 'reserved' THEN 1 ELSE 0 END), 0)::bigint AS reserved_count, \
                COALESCE(SUM(CASE WHEN state = 'publishing' THEN 1 ELSE 0 END), 0)::bigint AS publishing_count, \
                COALESCE(SUM(CASE WHEN state = 'publishing' AND lease_expires_at IS NOT NULL AND lease_expires_at <= NOW() THEN 1 ELSE 0 END), 0)::bigint AS expired_publishing_count, \
                COALESCE(SUM(CASE WHEN state = 'published' THEN 1 ELSE 0 END), 0)::bigint AS published_count, \
                COALESCE(SUM(CASE WHEN state = 'acknowledged' THEN 1 ELSE 0 END), 0)::bigint AS acknowledged_count \
             FROM iggy_consumer_poison_receipts \
             WHERE consumer_group = $1;"
        }
        _ => {
            "SELECT \
                CAST(COUNT(1) AS SIGNED) AS total, \
                COALESCE(SUM(CASE WHEN state = 'reserved' THEN 1 ELSE 0 END), 0) AS reserved_count, \
                COALESCE(SUM(CASE WHEN state = 'publishing' THEN 1 ELSE 0 END), 0) AS publishing_count, \
                COALESCE(SUM(CASE WHEN state = 'publishing' AND lease_expires_at IS NOT NULL AND lease_expires_at <= CURRENT_TIMESTAMP THEN 1 ELSE 0 END), 0) AS expired_publishing_count, \
                COALESCE(SUM(CASE WHEN state = 'published' THEN 1 ELSE 0 END), 0) AS published_count, \
                COALESCE(SUM(CASE WHEN state = 'acknowledged' THEN 1 ELSE 0 END), 0) AS acknowledged_count \
             FROM iggy_consumer_poison_receipts \
             WHERE consumer_group = ?;"
        }
    }
}

fn storage_error(error: impl std::fmt::Display) -> ConsumerPoisonReceiptError {
    ConsumerPoisonReceiptError::Storage(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database};

    async fn sqlite_inspector() -> ConsumerPoisonReceiptInspector {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(
            "CREATE TABLE iggy_consumer_poison_receipts ( \
                id INTEGER PRIMARY KEY AUTOINCREMENT, \
                delivery_id BLOB, \
                consumer_group TEXT NOT NULL, \
                source_stream TEXT, \
                source_topic TEXT, \
                source_partition INTEGER, \
                source_offset INTEGER, \
                payload BLOB, \
                stable_error_code TEXT, \
                state TEXT NOT NULL, \
                publisher_id BLOB, \
                delivery_attempt_count INTEGER DEFAULT 1, \
                lease_expires_at TEXT, \
                published_at TEXT, \
                acknowledged_at TEXT, \
                created_at TEXT DEFAULT CURRENT_TIMESTAMP, \
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP \
            );",
        )
        .await
        .unwrap();
        ConsumerPoisonReceiptInspector::new(db)
    }

    #[tokio::test]
    async fn summary_is_bounded_and_counts_expired_claims() {
        let inspector = sqlite_inspector().await;
        inspector
            .db
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "INSERT INTO iggy_consumer_poison_receipts (consumer_group, state, lease_expires_at) VALUES \
                 ('group-a', 'reserved', NULL), \
                 ('group-a', 'publishing', '2000-01-01 00:00:00'), \
                 ('group-a', 'publishing', '2999-01-01 00:00:00'), \
                 ('group-a', 'published', NULL), \
                 ('group-a', 'acknowledged', NULL), \
                 ('group-b', 'reserved', NULL)"
                    .to_string(),
            ))
            .await
            .unwrap();

        let summary = inspector.summarize("group-a").await.unwrap();
        assert_eq!(summary.total(), 5);
        assert_eq!(summary.reserved(), 1);
        assert_eq!(summary.publishing(), 2);
        assert_eq!(summary.expired_publishing(), 1);
        assert_eq!(summary.published(), 1);
        assert_eq!(summary.acknowledged(), 1);
        assert!(summary.has_recovery_work());
        assert!(summary.has_expired_claims());
    }

    #[tokio::test]
    async fn unknown_state_fails_closed() {
        let inspector = sqlite_inspector().await;
        inspector
            .db
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "INSERT INTO iggy_consumer_poison_receipts (consumer_group, state, lease_expires_at) VALUES ('group-a', 'corrupt', NULL)"
                    .to_string(),
            ))
            .await
            .unwrap();

        assert!(matches!(
            inspector.summarize("group-a").await,
            Err(ConsumerPoisonReceiptError::InvalidStoredState(_))
        ));
    }

    #[tokio::test]
    async fn invalid_consumer_group_is_rejected_before_query() {
        let inspector = sqlite_inspector().await;
        assert!(matches!(
            inspector.summarize(" group-a").await,
            Err(ConsumerPoisonReceiptError::InvalidIdentity {
                field: "consumer_group",
                ..
            })
        ));
    }
}
