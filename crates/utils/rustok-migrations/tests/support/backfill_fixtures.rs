use sea_orm_migration::sea_orm::{
    AccessMode, ConnectionTrait, DatabaseConnection, DbBackend, IsolationLevel, Statement,
    TransactionTrait,
};
use serde_json::Value;
use std::collections::HashSet;
use std::error::Error;
use std::fs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackfillFixture {
    pub id: String,
    pub migration: String,
    pub setup_sql: String,
    pub assertion_sql: String,
}

pub fn validate_pending(
    fixtures: &[BackfillFixture],
    pending: &HashSet<String>,
) -> Result<(), Box<dyn Error>> {
    for fixture in fixtures {
        if !pending.contains(&fixture.migration) {
            return Err(format!(
                "backfill fixture {} must target a pending migration: {}",
                fixture.id, fixture.migration
            )
            .into());
        }
    }
    Ok(())
}

pub fn load_from_environment() -> Result<Vec<BackfillFixture>, Box<dyn Error>> {
    let path = match std::env::var("RUSTOK_MIGRATION_SMOKE_BACKFILL_FIXTURES") {
        Ok(path) if path.trim().is_empty() => return Ok(Vec::new()),
        Ok(path) => path,
        Err(std::env::VarError::NotPresent) => return Ok(Vec::new()),
        Err(error) => return Err(format!("invalid backfill fixture environment: {error}").into()),
    };

    let content = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read backfill fixture file {path}: {error}"))?;
    parse_fixture_document(&content)
        .map_err(|error| format!("invalid backfill fixture file {path}: {error}").into())
}

pub async fn apply_setup(
    db: &DatabaseConnection,
    fixtures: &[BackfillFixture],
) -> Result<(), Box<dyn Error>> {
    for fixture in fixtures {
        db.execute_unprepared(&fixture.setup_sql)
            .await
            .map_err(|error| {
                format!(
                    "backfill fixture {} setup for migration {} failed: {error}",
                    fixture.id, fixture.migration
                )
            })?;
    }
    Ok(())
}

pub async fn assert_results(
    db: &DatabaseConnection,
    fixtures: &[BackfillFixture],
) -> Result<(), Box<dyn Error>> {
    for fixture in fixtures {
        let transaction = db
            .begin_with_config(
                Some(IsolationLevel::RepeatableRead),
                Some(AccessMode::ReadOnly),
            )
            .await?;
        let result = async {
            let rows = transaction
                .query_all_raw(Statement::from_string(
                    DbBackend::Postgres,
                    fixture.assertion_sql.clone(),
                ))
                .await
                .map_err(|error| {
                    format!(
                        "backfill fixture {} assertion for migration {} failed: {error}",
                        fixture.id, fixture.migration
                    )
                })?;
            if rows.len() != 1 {
                return Err(format!(
                    "backfill fixture {} assertion must return exactly one row, got {}",
                    fixture.id,
                    rows.len()
                )
                .into());
            }
            let passed: bool = rows[0].try_get("", "passed").map_err(|error| {
                format!(
                    "backfill fixture {} assertion must return boolean column `passed`: {error}",
                    fixture.id
                )
            })?;
            if !passed {
                return Err(format!(
                    "backfill fixture {} failed after migration {}",
                    fixture.id, fixture.migration
                )
                .into());
            }
            Ok::<_, Box<dyn Error>>(())
        }
        .await;
        let rollback = transaction.rollback().await;
        match (result, rollback) {
            (Err(error), Err(cleanup)) => {
                return Err(format!("{error}; assertion rollback: {cleanup}").into());
            }
            (Err(error), _) => return Err(error),
            (_, Err(error)) => return Err(error.into()),
            _ => {}
        }
    }
    Ok(())
}

fn parse_fixture_document(content: &str) -> Result<Vec<BackfillFixture>, String> {
    let document: Value =
        serde_json::from_str(content).map_err(|error| format!("JSON decode failed: {error}"))?;
    if document.get("schema_version").and_then(Value::as_u64) != Some(1) {
        return Err("schema_version must be 1".to_string());
    }
    let fixtures = document
        .get("fixtures")
        .and_then(Value::as_array)
        .ok_or_else(|| "fixtures must be an array".to_string())?;

    let mut ids = HashSet::new();
    let mut migrations = HashSet::new();
    let mut parsed = Vec::with_capacity(fixtures.len());
    for (index, fixture) in fixtures.iter().enumerate() {
        let parsed_fixture = BackfillFixture {
            id: required_string(fixture, index, "id")?,
            migration: required_string(fixture, index, "migration")?,
            setup_sql: required_string(fixture, index, "setup_sql")?,
            assertion_sql: required_string(fixture, index, "assertion_sql")?,
        };
        if !ids.insert(parsed_fixture.id.clone()) {
            return Err(format!(
                "fixtures[{index}].id duplicates {}",
                parsed_fixture.id
            ));
        }
        if !migrations.insert(parsed_fixture.migration.clone()) {
            return Err(format!(
                "fixtures[{index}].migration duplicates {}",
                parsed_fixture.migration
            ));
        }
        parsed.push(parsed_fixture);
    }
    Ok(parsed)
}

fn required_string(fixture: &Value, index: usize, field: &str) -> Result<String, String> {
    fixture
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("fixtures[{index}].{field} must be a non-empty string"))
}

#[cfg(test)]
mod tests {
    use super::{BackfillFixture, parse_fixture_document, validate_pending};

    #[test]
    fn fixtures_must_target_the_pending_range() {
        let fixtures = parse_fixture_document(
            r#"{"schema_version":1,"fixtures":[{
            "id":"sample", "migration":"backfill", "setup_sql":"SELECT 1",
            "assertion_sql":"SELECT true AS passed"}]}"#,
        )
        .unwrap();
        assert!(validate_pending(&fixtures, &["backfill".to_string()].into()).is_ok());
        for pending in [
            std::collections::HashSet::new(),
            ["another".to_string()].into(),
        ] {
            assert!(validate_pending(&fixtures, &pending).is_err());
        }
    }

    #[tokio::test]
    #[ignore = "requires isolated PostgreSQL and RUSTOK_MIGRATION_VERIFY_TEST_ADMIN_URL"]
    async fn assertions_are_read_only_and_strict() -> Result<(), Box<dyn std::error::Error>> {
        use sea_orm_migration::sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
        let admin_url = std::env::var("RUSTOK_MIGRATION_VERIFY_TEST_ADMIN_URL")?;
        let mut url = url::Url::parse(&admin_url)?;
        assert!(matches!(
            url.host_str(),
            Some("127.0.0.1" | "localhost" | "::1")
        ));
        let name = format!("rustok_assert_{}", uuid::Uuid::new_v4().simple());
        url.set_path(&name);
        let admin = Database::connect(&admin_url).await?;
        admin
            .execute_unprepared(&format!("CREATE DATABASE {name} TEMPLATE template0"))
            .await?;
        let result = async {
            let db = Database::connect(url.as_str()).await?;
            let check = async {
                db.execute_unprepared("CREATE TABLE assertion_probe (value integer); INSERT INTO assertion_probe VALUES (1)").await?;
                for (sql, expected) in [
                    ("SELECT true AS passed", true),
                    ("SELECT false AS passed", false),
                    ("SELECT NULL::boolean AS passed", false),
                    ("SELECT 1 AS passed", false),
                    ("SELECT true AS passed WHERE false", false),
                    ("SELECT true AS passed UNION ALL SELECT true", false),
                    ("WITH changed AS (UPDATE assertion_probe SET value=2 RETURNING value) SELECT true AS passed FROM changed", false),
                ] {
                    let fixtures = [super::BackfillFixture {
                        id: "strict".into(), migration: "target".into(), setup_sql: "SELECT 1".into(), assertion_sql: sql.into(),
                    }];
                    if super::assert_results(&db, &fixtures).await.is_ok() != expected {
                        return Err(format!("unexpected assertion result for {sql}").into());
                    }
                }
                let row = db.query_one_raw(Statement::from_string(DbBackend::Postgres, "SELECT value FROM assertion_probe")).await?.ok_or("probe disappeared")?;
                if row.try_get::<i32>("", "value")? != 1 { return Err("assertion modified fixture data".into()); }
                Ok::<_, Box<dyn std::error::Error>>(())
            }.await;
            db.close().await?;
            check
        }.await;
        let cleanup = admin
            .execute_unprepared(&format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)"))
            .await;
        admin.close().await?;
        match (result, cleanup) {
            (Err(error), Err(cleanup)) => {
                Err(format!("{error}; fixture cleanup: {cleanup}").into())
            }
            (Err(error), _) => Err(error),
            (_, Err(error)) => Err(error.into()),
            _ => Ok(()),
        }
    }

    #[test]
    fn empty_fixture_document_is_valid() {
        assert_eq!(
            parse_fixture_document(r#"{"schema_version":1,"fixtures":[]}"#)
                .expect("empty fixture document should parse"),
            Vec::<BackfillFixture>::new()
        );
    }

    #[test]
    fn fixture_document_requires_assertion_sql() {
        let error = parse_fixture_document(
            r#"{
                "schema_version": 1,
                "fixtures": [{
                    "id": "sample",
                    "migration": "m1",
                    "setup_sql": "SELECT 1"
                }]
            }"#,
        )
        .expect_err("missing assertion_sql must fail");
        assert!(error.contains("assertion_sql"));
    }

    #[test]
    fn fixture_document_rejects_duplicate_migrations() {
        let error = parse_fixture_document(
            r#"{
                "schema_version": 1,
                "fixtures": [
                    {
                        "id": "first",
                        "migration": "m1",
                        "setup_sql": "SELECT 1",
                        "assertion_sql": "SELECT true AS passed"
                    },
                    {
                        "id": "second",
                        "migration": "m1",
                        "setup_sql": "SELECT 1",
                        "assertion_sql": "SELECT true AS passed"
                    }
                ]
            }"#,
        )
        .expect_err("duplicate migrations must fail");
        assert!(error.contains("migration duplicates m1"));
    }
}
