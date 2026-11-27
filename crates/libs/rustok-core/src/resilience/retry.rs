/// Retry Policy with Exponential Backoff
///
/// Automatically retries failed operations with configurable strategies.
use std::time::Duration;

/// Retry strategy
#[derive(Debug, Clone)]
pub enum RetryStrategy {
    /// Fixed delay between retries
    Fixed(Duration),

    /// Exponential backoff: delay = base * 2^attempt
    Exponential { base: Duration, max: Duration },

    /// Linear backoff: delay = base * attempt
    Linear { base: Duration, max: Duration },
}

impl RetryStrategy {
    pub fn delay(&self, attempt: u32) -> Duration {
        match self {
            RetryStrategy::Fixed(duration) => *duration,

            RetryStrategy::Exponential { base, max } => {
                if base.is_zero() || max.is_zero() {
                    return Duration::ZERO;
                }
                let mut delay = *base;
                for _ in 0..attempt.min(64) {
                    if delay >= *max {
                        return *max;
                    }
                    delay = delay.saturating_mul(2);
                }
                delay.min(*max)
            }

            RetryStrategy::Linear { base, max } => {
                let delay = base.saturating_mul(attempt);
                delay.min(*max)
            }
        }
    }
}

/// Retry policy configuration
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts
    pub max_attempts: u32,

    /// Retry strategy
    pub strategy: RetryStrategy,

    /// Predicate to determine if error is retryable
    /// None means retry all errors
    pub retryable_predicate: Option<fn(&str) -> bool>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            strategy: RetryStrategy::Exponential {
                base: Duration::from_millis(100),
                max: Duration::from_secs(5),
            },
            retryable_predicate: None,
        }
    }
}

impl RetryPolicy {
    /// Execute operation with retry
    pub async fn execute<F, Fut, T, E>(&self, mut f: F) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        let mut attempt = 0;

        loop {
            match f().await {
                Ok(value) => return Ok(value),
                Err(err) => {
                    attempt += 1;

                    // Check if we should retry
                    let should_retry = if let Some(predicate) = self.retryable_predicate {
                        predicate(&err.to_string())
                    } else {
                        true
                    };

                    if !should_retry || attempt >= self.max_attempts {
                        return Err(err);
                    }

                    // Calculate delay and sleep
                    let delay = self.strategy.delay(attempt);

                    tracing::debug!(
                        attempt = attempt,
                        delay_ms = delay.as_millis(),
                        error = %err,
                        "Retrying operation"
                    );

                    tokio::time::sleep(delay).await;
                }
            }
        }
    }
}

/// The only PostgreSQL statement failures eligible for a fresh SQL-only
/// transaction attempt. Never classify formatted diagnostics or transport errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostgresTransactionConflict {
    LockUnavailable,
    Deadlock,
    Serialization,
}

impl PostgresTransactionConflict {
    pub fn classify(backend: sea_orm::DbBackend, error: &sea_orm::DbErr) -> Option<Self> {
        use sea_orm::{DbBackend, DbErr, RuntimeErr};
        if backend != DbBackend::Postgres {
            return None;
        }
        let driver = match error {
            DbErr::Exec(RuntimeErr::SqlxError(driver))
            | DbErr::Query(RuntimeErr::SqlxError(driver)) => driver,
            _ => return None,
        };
        let database = driver.as_database_error()?;
        match database.code()?.as_ref() {
            "55P03" => Some(Self::LockUnavailable),
            "40P01" => Some(Self::Deadlock),
            "40001" => Some(Self::Serialization),
            _ => None,
        }
    }
}

/// A bounded policy for an owner-controlled SQL-only transaction. The caller
/// must run fresh admission and authorization on every attempt; never pass a
/// commit error, or an operation with irreversible effects, to this function.
/// Consumes the transaction and requires a successful explicit rollback before
/// returning permission to retry. Even a recognized conflict is insufficient
/// when rollback itself fails. The generic RetryPolicy is not this contract.
pub async fn rollback_for_transaction_retry(
    tx: sea_orm::DatabaseTransaction,
    error: &sea_orm::DbErr,
    attempt: u32,
) -> Result<bool, sea_orm::DbErr> {
    use sea_orm::ConnectionTrait;
    let delay = transaction_retry_delay(
        PostgresTransactionConflict::classify(tx.get_database_backend(), error),
        attempt,
    );
    tx.rollback().await?;
    if let Some(delay) = delay {
        tokio::time::sleep(transaction_retry_jitter(delay)).await;
        Ok(true)
    } else {
        Ok(false)
    }
}

// Attempt numbers are one-based. A fixed platform ceiling prevents callers
// accidentally converting contention into an unbounded retry loop.
fn transaction_retry_delay(
    conflict: Option<PostgresTransactionConflict>,
    attempt: u32,
) -> Option<Duration> {
    if conflict.is_none() || !(1..3).contains(&attempt) {
        return None;
    }
    Some(RetryStrategy::Exponential {
        base: Duration::from_millis(25),
        max: Duration::from_millis(100),
    }.delay(attempt - 1))
}

// The platform delay is bounded to 25/50 ms; add up to 50% jitter to
// prevent synchronized contenders from immediately colliding again.
fn transaction_retry_jitter(base: Duration) -> Duration {
    let upper_ms = (base.as_millis() / 2) as u64;
    base.saturating_add(Duration::from_millis(rand::random_range(0..=upper_ms)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn transaction_retry_rejects_untyped_errors_and_exhausted_attempts() {
        use sea_orm::{DbBackend, DbErr};
        for backend in [DbBackend::Postgres, DbBackend::Sqlite] {
            assert_eq!(
                PostgresTransactionConflict::classify(backend, &DbErr::Custom("55P03 40P01 40001".into())),
                None
            );
        }
        for conflict in [
            PostgresTransactionConflict::LockUnavailable,
            PostgresTransactionConflict::Deadlock,
            PostgresTransactionConflict::Serialization,
        ] {
            assert_eq!(transaction_retry_delay(Some(conflict), 0), None);
            assert_eq!(transaction_retry_delay(Some(conflict), 1), Some(Duration::from_millis(25)));
            assert_eq!(transaction_retry_delay(Some(conflict), 2), Some(Duration::from_millis(50)));
            assert_eq!(transaction_retry_delay(Some(conflict), 3), None);
            assert_eq!(transaction_retry_delay(Some(conflict), u32::MAX), None);
        }
        assert_eq!(transaction_retry_delay(None, 1), None);
    }

    #[test]
    fn transaction_retry_jitter_stays_within_platform_bounds() {
        for (base, upper) in [(25, 37), (50, 75)] {
            for _ in 0..128 {
                let delay = transaction_retry_jitter(Duration::from_millis(base));
                assert!(delay >= Duration::from_millis(base));
                assert!(delay <= Duration::from_millis(upper));
            }
        }
    }

    #[tokio::test]
    async fn transaction_retry_rolls_back_even_when_error_is_not_retryable() {
        use sea_orm::{ConnectionTrait, Database, DbErr, Statement, TransactionTrait};
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("CREATE TABLE retry_probe (id INTEGER PRIMARY KEY)").await.unwrap();
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared("INSERT INTO retry_probe VALUES (1)").await.unwrap();
        assert!(!rollback_for_transaction_retry(tx, &DbErr::Custom("55P03".into()), 1).await.unwrap());
        let row = db.query_one_raw(Statement::from_string(db.get_database_backend(), "SELECT COUNT(*) AS count FROM retry_probe"))
            .await.unwrap().unwrap();
        assert_eq!(row.try_get::<i64>("", "count").unwrap(), 0);
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_CORE_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_transaction_retry_classifies_driver_codes_and_requires_rollback() {
        use sea_orm::{ConnectionTrait, Database, DbBackend, Statement, TransactionTrait};
        let db = Database::connect(std::env::var("RUSTOK_CORE_TEST_DATABASE_URL").unwrap()).await.unwrap();
        assert_eq!(db.get_database_backend(), DbBackend::Postgres);
        let schema = format!("transaction_retry_{}", uuid::Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}; CREATE TABLE {schema}.probe (id INTEGER PRIMARY KEY)")).await.unwrap();
        for (code, expected) in [
            ("55P03", Some(PostgresTransactionConflict::LockUnavailable)),
            ("40P01", Some(PostgresTransactionConflict::Deadlock)),
            ("40001", Some(PostgresTransactionConflict::Serialization)),
            ("23503", None),
        ] {
            for attempt in [1, 3] {
                let tx = db.begin().await.unwrap();
                tx.execute_unprepared(&format!("INSERT INTO {schema}.probe VALUES (1)")).await.unwrap();
                let error = tx.execute_unprepared(&format!(
                    "DO $$ BEGIN RAISE EXCEPTION 'fixed diagnostic' USING ERRCODE = '{code}'; END $$"
                )).await.unwrap_err();
                assert_eq!(PostgresTransactionConflict::classify(DbBackend::Postgres, &error), expected);
                assert_eq!(rollback_for_transaction_retry(tx, &error, attempt).await.unwrap(), expected.is_some() && attempt == 1);
                let row = db.query_one_raw(Statement::from_string(DbBackend::Postgres, format!("SELECT COUNT(*) AS count FROM {schema}.probe")))
                    .await.unwrap().unwrap();
                assert_eq!(row.try_get::<i64>("", "count").unwrap(), 0);
            }
        }
        // Exercise the actual lock-not-available path, not only RAISE.
        db.execute_unprepared(&format!("INSERT INTO {schema}.probe VALUES (1)")).await.unwrap();
        let holder = db.begin().await.unwrap();
        holder.execute_unprepared(&format!("SELECT id FROM {schema}.probe FOR UPDATE")).await.unwrap();
        let writer = db.begin().await.unwrap();
        let error = writer.execute_unprepared(&format!("SELECT id FROM {schema}.probe FOR UPDATE NOWAIT")).await.unwrap_err();
        assert_eq!(PostgresTransactionConflict::classify(DbBackend::Postgres, &error), Some(PostgresTransactionConflict::LockUnavailable));
        assert!(rollback_for_transaction_retry(writer, &error, 1).await.unwrap());
        holder.rollback().await.unwrap();
        let retry = db.begin().await.unwrap();
        retry.execute_unprepared(&format!("SELECT id FROM {schema}.probe FOR UPDATE NOWAIT")).await.unwrap();
        retry.rollback().await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }

    #[tokio::test]
    async fn test_retry_success_on_second_attempt() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let policy = RetryPolicy::default();

        let result = policy
            .execute(|| async {
                let count = counter_clone.fetch_add(1, Ordering::SeqCst);

                if count == 0 {
                    Err("first attempt fails")
                } else {
                    Ok(42)
                }
            })
            .await;

        assert_eq!(result, Ok(42));
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_retry_exhausted() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let policy = RetryPolicy {
            max_attempts: 3,
            ..Default::default()
        };

        let result = policy
            .execute(|| async {
                counter_clone.fetch_add(1, Ordering::SeqCst);
                Err::<i32, _>("always fails")
            })
            .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_exponential_backoff() {
        let strategy = RetryStrategy::Exponential {
            base: Duration::from_millis(100),
            max: Duration::from_secs(10),
        };

        assert_eq!(strategy.delay(0), Duration::from_millis(100));
        assert_eq!(strategy.delay(1), Duration::from_millis(200));
        assert_eq!(strategy.delay(2), Duration::from_millis(400));
        assert_eq!(strategy.delay(3), Duration::from_millis(800));
    }

    #[test]
    fn test_linear_backoff() {
        let strategy = RetryStrategy::Linear {
            base: Duration::from_millis(100),
            max: Duration::from_secs(1),
        };

        assert_eq!(strategy.delay(1), Duration::from_millis(100));
        assert_eq!(strategy.delay(2), Duration::from_millis(200));
        assert_eq!(strategy.delay(3), Duration::from_millis(300));
    }

    #[test]
    fn test_exponential_backoff_large_attempt_does_not_panic() {
        let max = Duration::from_secs(10);
        let strategy = RetryStrategy::Exponential {
            base: Duration::from_millis(100),
            max,
        };

        assert_eq!(strategy.delay(64), max);
        assert_eq!(strategy.delay(10_000), max);
        assert_eq!(strategy.delay(u32::MAX), max);
    }
}
