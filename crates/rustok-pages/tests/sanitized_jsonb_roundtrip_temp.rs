use std::env;
use std::error::Error;

use rustok_page_builder::sanitize_static_landing_project;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DbBackend, Statement};
use serde_json::{Value, json};

const DATABASE_ENV: &str = "RUSTOK_PAGES_TEST_DATABASE_URL";
type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test]
async fn sanitized_project_hash_survives_postgres_jsonb_roundtrip() -> TestResult<()> {
    let Ok(database_url) = env::var(DATABASE_ENV) else {
        eprintln!("{DATABASE_ENV} is not set; skipping PostgreSQL JSONB hash diagnostic");
        return Ok(());
    };
    if !database_url.starts_with("postgres://") && !database_url.starts_with("postgresql://") {
        eprintln!("{DATABASE_ENV} is not a PostgreSQL URL; skipping JSONB hash diagnostic");
        return Ok(());
    }

    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;

    db.execute_unprepared(
        "CREATE TEMP TABLE pages_sanitized_jsonb_roundtrip_temp (payload JSONB NOT NULL)",
    )
    .await?;

    let source = json!({
        "pages": [{
            "id": "home-en",
            "flyPageMeta": {
                "title": "Rollback activated A EN",
                "description": "Rollback-activated artifact-loss recovery",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": "Rollback activated A EN"
                }]
            }
        }]
    });

    let before = sanitize_static_landing_project(&source)?;
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO pages_sanitized_jsonb_roundtrip_temp (payload) VALUES ($1)",
        vec![before.project_data().clone().into()],
    ))
    .await?;

    let row = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT payload FROM pages_sanitized_jsonb_roundtrip_temp".to_string(),
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("JSONB diagnostic row is missing"))?;
    let roundtripped: Value = row.try_get("", "payload")?;
    let after = sanitize_static_landing_project(&roundtripped)?;

    eprintln!("before_hash={}", before.sanitized_hash());
    eprintln!("after_hash={}", after.sanitized_hash());
    eprintln!("project_equal={}", before.project_data() == &roundtripped);
    if before.project_data() != &roundtripped {
        eprintln!(
            "before_project={}\nafter_jsonb={}",
            serde_json::to_string_pretty(before.project_data())?,
            serde_json::to_string_pretty(&roundtripped)?
        );
    }

    assert_eq!(
        before.project_data(),
        &roundtripped,
        "PostgreSQL JSONB changed the retained sanitized project"
    );
    assert_eq!(
        before.sanitized_hash(),
        after.sanitized_hash(),
        "PostgreSQL JSONB roundtrip changed the policy-bound sanitized hash"
    );
    Ok(())
}
