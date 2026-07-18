use sea_orm_migration::sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, Statement,
};
use serde_json::Value;
use std::error::Error;
use std::fs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackfillFixture {
    pub id: String,
    pub migration: String,
    pub setup_sql: String,
    pub assertion_sql: String,
}

pub fn load_from_environment() -> Result<Vec<BackfillFixture>, Box<dyn Error>> {
    let Some(path) = std::env::var("RUSTOK_MIGRATION_SMOKE_BACKFILL_FIXTURES")
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(Vec::new());
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
        let row = db
            .query_one(Statement::from_string(
                DbBackend::Postgres,
                fixture.assertion_sql.clone(),
            ))
            .await
            .map_err(|error| {
                format!(
                    "backfill fixture {} assertion for migration {} failed: {error}",
                    fixture.id, fixture.migration
                )
            })?
            .ok_or_else(|| {
                format!(
                    "backfill fixture {} assertion returned no row",
                    fixture.id
                )
            })?;
        let passed: bool = row.try_get("", "passed").map_err(|error| {
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

    let mut parsed = Vec::with_capacity(fixtures.len());
    for (index, fixture) in fixtures.iter().enumerate() {
        parsed.push(BackfillFixture {
            id: required_string(fixture, index, "id")?,
            migration: required_string(fixture, index, "migration")?,
            setup_sql: required_string(fixture, index, "setup_sql")?,
            assertion_sql: required_string(fixture, index, "assertion_sql")?,
        });
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
    use super::{parse_fixture_document, BackfillFixture};

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
}
