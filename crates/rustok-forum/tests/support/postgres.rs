use rustok_core::MigrationSource;
use rustok_forum::ForumModule;
use rustok_outbox::OutboxModule;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

use super::{TestResult, test_error};

const FORUM_TEST_DATABASE_ENV: &str = "RUSTOK_FORUM_TEST_DATABASE_URL";

pub struct PostgresForumTestDb {
    control: DatabaseConnection,
    database_url: String,
    pub db: DatabaseConnection,
    pub schema_name: String,
}

impl PostgresForumTestDb {
    pub async fn setup(prefix: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{FORUM_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping PostgreSQL forum regression test"
            );
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_forum_{}_{}",
            sanitize_identifier(prefix),
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let db = connect(&database_url).await?;
        set_search_path(&db, &schema_name).await?;

        let manager = SchemaManager::new(&db);
        let migration_result = async {
            for migration in OutboxModule.migrations() {
                migration.up(&manager).await?;
            }
            for migration in TaxonomyModule.migrations() {
                migration.up(&manager).await?;
            }
            for migration in ForumModule.migrations() {
                migration.up(&manager).await?;
            }
            Ok::<(), sea_orm::DbErr>(())
        }
        .await;

        if let Err(error) = migration_result {
            let _ = control
                .execute_unprepared(&format!(r#"DROP SCHEMA IF EXISTS "{schema_name}" CASCADE"#))
                .await;
            return Err(error.into());
        }

        Ok(Some(Self {
            control,
            database_url,
            db,
            schema_name,
        }))
    }

    pub async fn peer(&self) -> TestResult<DatabaseConnection> {
        let db = connect(&self.database_url).await?;
        set_search_path(&db, &self.schema_name).await?;
        Ok(db)
    }

    pub async fn cleanup(self) -> TestResult<()> {
        self.control
            .execute_unprepared(&format!(
                r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
                self.schema_name
            ))
            .await?;
        Ok(())
    }
}

pub fn postgres_database_url() -> Option<String> {
    std::env::var(FORUM_TEST_DATABASE_ENV)
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

pub async fn execute(db: &DatabaseConnection, sql: impl AsRef<str>) -> TestResult<()> {
    db.execute_unprepared(sql.as_ref()).await?;
    Ok(())
}

pub async fn expect_rejected(
    db: &DatabaseConnection,
    sql: impl AsRef<str>,
    label: &str,
) -> TestResult<()> {
    if db.execute_unprepared(sql.as_ref()).await.is_ok() {
        return Err(test_error(format!(
            "{label} must be rejected by the PostgreSQL runtime"
        )));
    }
    Ok(())
}

async fn connect(database_url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

async fn set_search_path(db: &DatabaseConnection, schema_name: &str) -> TestResult<()> {
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}""#))
        .await?;
    Ok(())
}

fn sanitize_identifier(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    let normalized = normalized.trim_matches('_');
    if normalized.is_empty() {
        "test".to_string()
    } else {
        normalized.to_string()
    }
}
