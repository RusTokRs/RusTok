use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};

use super::connection::TestResult;

pub async fn i64_value(db: &DatabaseConnection, sql: &str) -> TestResult<i64> {
    Ok(db
        .query_one(Statement::from_string(DbBackend::Postgres, sql.to_owned()))
        .await?
        .ok_or("scalar query returned no row")?
        .try_get("", "value")?)
}

pub async fn bool_value(db: &DatabaseConnection, sql: &str) -> TestResult<bool> {
    Ok(db
        .query_one(Statement::from_string(DbBackend::Postgres, sql.to_owned()))
        .await?
        .ok_or("scalar query returned no row")?
        .try_get("", "value")?)
}

pub async fn string_value(db: &DatabaseConnection, sql: &str) -> TestResult<String> {
    Ok(db
        .query_one(Statement::from_string(DbBackend::Postgres, sql.to_owned()))
        .await?
        .ok_or("scalar query returned no row")?
        .try_get("", "value")?)
}
