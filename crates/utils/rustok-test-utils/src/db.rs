//! Database testing utilities
//!
//! Provides functions for setting up test databases with migrations.

use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
    TransactionTrait,
};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use uuid::Uuid;

static DB_LOCK: tokio::sync::OnceCell<Arc<Mutex<()>>> = tokio::sync::OnceCell::const_new();

/// Sets up an in-memory SQLite database for testing.
///
/// This creates a fresh SQLite database in memory.
///
/// Note: this helper does **not** run migrations automatically.
/// Use `setup_test_db_with_migrations::<M>()` when schema is required.
/// The database is isolated per test.
///
/// # Example
///
/// ```rust
/// use rustok_test_utils::setup_test_db;
///
/// #[tokio::test]
/// async fn test_with_db() {
///     let db = setup_test_db().await;
///     // Use db for testing...
/// }
/// ```
pub async fn setup_test_db() -> DatabaseConnection {
    // Use a lock to prevent concurrent migration runs which can cause conflicts
    let lock = DB_LOCK
        .get_or_init(|| async { Arc::new(Mutex::new(())) })
        .await;
    let _guard = lock.lock().await;

    let db_url = format!(
        "sqlite:file:rustok_test_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut opts = ConnectOptions::new(db_url);
    // In-memory SQLite schema state is connection-scoped enough that
    // migrations can observe inconsistent DDL across pooled connections.
    // Keep test databases on a single connection for deterministic schema
    // visibility during migrations and test execution.
    opts.max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);

    let db = Database::connect(opts)
        .await
        .expect("Failed to connect to test database");

    // Intentionally no automatic migrations here.
    // Tests can opt in to module-specific migrations via
    // `setup_test_db_with_migrations::<M>()`.

    db
}

/// Sets up a test database with specific migrations.
///
/// This is useful when you want to test a specific module without
/// running all migrations.
///
/// # Type Parameters
///
/// * `M` - The migration type that implements `MigratorTrait`
///
/// # Example
///
/// ```rust,ignore
/// use rustok_test_utils::setup_test_db_with_migrations;
/// use rustok_content::migrations::Migrator;
///
/// #[tokio::test]
/// async fn test_content_module() {
///     let db = setup_test_db_with_migrations::<Migrator>().await;
///     // Test with content migrations only...
/// }
/// ```
pub async fn setup_test_db_with_migrations<M>() -> DatabaseConnection
where
    M: sea_orm_migration::MigratorTrait,
{
    let lock = DB_LOCK
        .get_or_init(|| async { Arc::new(Mutex::new(())) })
        .await;
    let _guard = lock.lock().await;

    let db_url = format!(
        "sqlite:file:rustok_test_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut opts = ConnectOptions::new(db_url);
    // For in-memory SQLite, migrations and subsequent queries must share
    // the same connection to avoid "no such table" races across the pool.
    opts.max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);

    let db = Database::connect(opts)
        .await
        .expect("Failed to connect to test database");

    let pending = M::get_pending_migrations(&db)
        .await
        .expect("Failed to load pending migrations");
    let pending_names: Vec<String> = pending
        .into_iter()
        .map(|migration| migration.name().to_string())
        .collect();

    if !pending_names.is_empty() {
        M::up(&db, None).await.unwrap_or_else(|error| {
            panic!(
                "Failed to run pending migrations {:?}: {error:?}",
                pending_names
            )
        });
    }

    db
}

/// Connects to a PostgreSQL database with bounded test-friendly timeouts.
pub async fn connect_postgres(url: &str) -> Result<DatabaseConnection, sea_orm::DbErr> {
    let mut options = ConnectOptions::new(url.to_owned());
    options
        .connect_timeout(Duration::from_secs(5))
        .acquire_timeout(Duration::from_secs(5))
        .sqlx_logging(false);
    Database::connect(options).await
}

/// Creates an isolated PostgreSQL database after validating its identifier.
pub async fn create_postgres_database(
    admin: &DatabaseConnection,
    database_name: &str,
) -> Result<(), sea_orm::DbErr> {
    assert_valid_postgres_database_name(database_name);
    admin
        .execute_raw(Statement::from_string(
            DbBackend::Postgres,
            format!("CREATE DATABASE {}", quoted_identifier(database_name)),
        ))
        .await?;
    Ok(())
}

/// Force-drops an isolated PostgreSQL test database when it exists.
pub async fn drop_postgres_database_if_exists(
    admin: &DatabaseConnection,
    database_name: &str,
) -> Result<(), sea_orm::DbErr> {
    assert_valid_postgres_database_name(database_name);
    admin
        .execute_raw(Statement::from_string(
            DbBackend::Postgres,
            format!(
                "DROP DATABASE IF EXISTS {} WITH (FORCE)",
                quoted_identifier(database_name)
            ),
        ))
        .await?;
    Ok(())
}

/// Replaces the database path in an admin PostgreSQL URL.
pub fn postgres_database_url(admin_url: &str, database_name: &str) -> String {
    assert_postgres_url(admin_url);
    assert_valid_postgres_database_name(database_name);
    let (base, suffix) = admin_url
        .split_once('?')
        .map(|(base, query)| (base, format!("?{query}")))
        .unwrap_or((admin_url, String::new()));
    let scheme_end = base
        .find("://")
        .expect("PostgreSQL URL must include a scheme separator")
        + 3;
    let authority_end = base[scheme_end..]
        .find('/')
        .map(|offset| scheme_end + offset)
        .unwrap_or(base.len());
    format!(
        "{}{}/{}{}",
        &base[..authority_end],
        "",
        database_name,
        suffix
    )
}

/// Generates a process-unique PostgreSQL test database name.
pub fn unique_postgres_database_name(prefix: &str) -> String {
    assert_valid_postgres_database_name(prefix);
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after UNIX_EPOCH")
        .as_millis();
    let name = format!("{prefix}_{millis}_{}", std::process::id());
    assert_valid_postgres_database_name(&name);
    name
}

/// Verifies that a test URL targets PostgreSQL.
pub fn assert_postgres_url(url: &str) {
    assert!(
        url.starts_with("postgres://") || url.starts_with("postgresql://"),
        "PostgreSQL test URL must use postgres:// or postgresql://"
    );
}

/// Verifies that a database name is safe to embed as a quoted identifier.
pub fn assert_valid_postgres_database_name(database_name: &str) {
    let mut chars = database_name.chars();
    let Some(first) = chars.next() else {
        panic!("PostgreSQL test database name must not be empty");
    };
    assert!(
        first == '_' || first.is_ascii_alphabetic(),
        "PostgreSQL test database name must start with a letter or underscore"
    );
    assert!(
        chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric()),
        "PostgreSQL test database name may contain only letters, digits, and underscores"
    );
    assert!(
        database_name.len() <= 63,
        "PostgreSQL test database name must not exceed 63 bytes"
    );
}

/// Verifies that a table is absent from the PostgreSQL `public` schema.
pub async fn assert_postgres_table_missing(
    db: &DatabaseConnection,
    table: &str,
) -> Result<(), sea_orm::DbErr> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT to_regclass($1) IS NULL AS missing",
            [format!("public.{table}").into()],
        ))
        .await?
        .ok_or_else(|| {
            sea_orm::DbErr::Custom(format!("table absence query for {table} returned no row"))
        })?;
    let missing: bool = row.try_get("", "missing")?;
    if !missing {
        return Err(sea_orm::DbErr::Custom(format!(
            "expected table {table} to be removed"
        )));
    }
    Ok(())
}

/// Verifies that a column is absent from a PostgreSQL `public` schema table.
pub async fn assert_postgres_column_missing(
    db: &DatabaseConnection,
    table: &str,
    column: &str,
) -> Result<(), sea_orm::DbErr> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
SELECT NOT EXISTS (
    SELECT 1
    FROM information_schema.columns
    WHERE table_schema = 'public'
      AND table_name = $1
      AND column_name = $2
) AS missing
"#,
            [table.to_owned().into(), column.to_owned().into()],
        ))
        .await?
        .ok_or_else(|| {
            sea_orm::DbErr::Custom(format!(
                "column absence query for {table}.{column} returned no row"
            ))
        })?;
    let missing: bool = row.try_get("", "missing")?;
    if !missing {
        return Err(sea_orm::DbErr::Custom(format!(
            "expected column {table}.{column} to be removed"
        )));
    }
    Ok(())
}

/// Verifies a PostgreSQL column's information-schema type and nullability.
pub async fn assert_postgres_column_contract(
    db: &DatabaseConnection,
    table: &str,
    column: &str,
    data_type: &str,
    nullable: bool,
) -> Result<(), sea_orm::DbErr> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
SELECT data_type, is_nullable
FROM information_schema.columns
WHERE table_schema = 'public'
  AND table_name = $1
  AND column_name = $2
"#,
            [table.to_owned().into(), column.to_owned().into()],
        ))
        .await?
        .ok_or_else(|| {
            sea_orm::DbErr::Custom(format!(
                "column contract query for {table}.{column} returned no row"
            ))
        })?;
    let actual_type: String = row.try_get("", "data_type")?;
    let actual_nullable: String = row.try_get("", "is_nullable")?;
    let expected_nullable = if nullable { "YES" } else { "NO" };
    if actual_type != data_type || actual_nullable != expected_nullable {
        return Err(sea_orm::DbErr::Custom(format!(
            "column {table}.{column} expected type={data_type}, nullable={expected_nullable}; \
             got type={actual_type}, nullable={actual_nullable}"
        )));
    }
    Ok(())
}

fn quoted_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

/// Creates a test transaction that will be rolled back after the test.
///
/// This is useful for tests that should not commit changes to the database.
///
/// # Example
///
/// ```rust,ignore
/// use rustok_test_utils::db::with_test_transaction;
///
/// #[tokio::test]
/// async fn test_with_transaction() {
///     with_test_transaction(|txn| async move {
///         // Perform database operations...
///         // Changes are automatically rolled back
///     }).await;
/// }
/// ```
pub async fn with_test_transaction<F, Fut, R>(f: F) -> R
where
    F: FnOnce(&sea_orm::DatabaseTransaction) -> Fut,
    Fut: std::future::Future<Output = R>,
{
    let db = setup_test_db().await;
    let txn = db.begin().await.expect("Failed to begin transaction");

    let result = f(&txn).await;

    // Transaction is dropped here, causing rollback
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires sqlite driver feature in SeaORM"]
    async fn test_setup_test_db() {
        let db = setup_test_db().await;

        // Just verify we can connect.
        assert!(db.ping().await.is_ok());
    }
}
