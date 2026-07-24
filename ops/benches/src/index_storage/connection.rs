use anyhow::{Context, Result};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};

pub(crate) const BENCHMARK_SESSION_METADATA: [(&str, &str); 4] = [
    ("standard_conforming_strings", "on"),
    ("timezone", "UTC"),
    ("date_style", "ISO, YMD"),
    ("extra_float_digits", "3"),
];

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

#[cfg(test)]
mod tests {
    use super::{BENCHMARK_SESSION_METADATA, BENCHMARK_SESSION_SQL};

    #[test]
    fn benchmark_session_contract_is_explicit_and_deterministic() {
        for (field, value, statement) in [
            (
                "standard_conforming_strings",
                "on",
                "SET standard_conforming_strings = on;",
            ),
            ("timezone", "UTC", "SET TIME ZONE 'UTC';"),
            ("date_style", "ISO, YMD", "SET DateStyle = 'ISO, YMD';"),
            ("extra_float_digits", "3", "SET extra_float_digits = 3;"),
        ] {
            assert!(
                BENCHMARK_SESSION_SQL.contains(statement),
                "benchmark session SQL is missing {statement}"
            );
            assert!(
                BENCHMARK_SESSION_METADATA.contains(&(field, value)),
                "benchmark session metadata is missing {field}={value}"
            );
        }
        assert!(!BENCHMARK_SESSION_SQL.contains("standard_conforming_strings = off"));
    }
}
