use anyhow::{Context, Result};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};

const BENCHMARK_SESSION_SQL: &str = concat!(
    "SET standard_conforming_strings = on;",
    " SET TIME ZONE 'UTC';",
    " SET DateStyle = 'ISO, YMD';",
    " SET extra_float_digits = 3;",
);

pub async fn connect(database_url: &str) -> Result<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .min_connections(1)
        .max_connections(1)
        .sqlx_logging(false);

    let db = Database::connect(options)
        .await
        .context("failed to connect to PostgreSQL with a single benchmark session")?;
    db.execute_unprepared(BENCHMARK_SESSION_SQL)
        .await
        .context(
            "failed to pin deterministic PostgreSQL benchmark session (failed to pin PostgreSQL standard-conforming string semantics)",
        )?;
    Ok(db)
}

/// Returns `error`, naming the rollback failure too when the rollback that followed it failed.
///
/// The benchmark binaries install no `tracing` subscriber and the crate does not depend on
/// `tracing`, so a rollback failure is reported through the returned error, not logged.
pub(crate) fn with_rollback_failure<E: std::fmt::Display>(
    error: anyhow::Error,
    rollback: std::result::Result<(), E>,
) -> anyhow::Error {
    match rollback {
        Ok(()) => error,
        Err(rollback_error) => error.context(format!(
            "the transaction rollback after this failure also failed: {rollback_error:#}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{BENCHMARK_SESSION_SQL, with_rollback_failure};

    #[test]
    fn benchmark_session_contract_is_explicit_and_deterministic() {
        for statement in [
            "SET standard_conforming_strings = on;",
            "SET TIME ZONE 'UTC';",
            "SET DateStyle = 'ISO, YMD';",
            "SET extra_float_digits = 3;",
        ] {
            assert!(
                BENCHMARK_SESSION_SQL.contains(statement),
                "benchmark session SQL is missing {statement}"
            );
        }
        assert!(!BENCHMARK_SESSION_SQL.contains("standard_conforming_strings = off"));
    }

    #[test]
    fn rollback_failure_is_added_to_the_reported_error() {
        let clean = with_rollback_failure(anyhow::anyhow!("primary failure"), Ok::<(), String>(()));
        assert_eq!(format!("{clean:#}"), "primary failure");

        let both = with_rollback_failure(
            anyhow::anyhow!("primary failure"),
            Err("connection reset".to_string()),
        );
        let rendered = format!("{both:#}");
        assert!(rendered.contains("primary failure"));
        assert!(rendered.contains("connection reset"));
    }
}
