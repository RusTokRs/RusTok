use anyhow::{Context, Result, ensure};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde::Serialize;

const DATABASE_METADATA_SQL: &str = concat!(
    "SELECT version() AS version,",
    " current_setting('server_version_num') AS server_version_num,",
    " current_setting('shared_buffers') AS shared_buffers,",
    " current_setting('effective_cache_size') AS effective_cache_size,",
    " current_setting('work_mem') AS work_mem,",
    " current_setting('random_page_cost') AS random_page_cost,",
    " current_setting('jit') AS jit,",
    " current_setting('standard_conforming_strings') AS standard_conforming_strings,",
    " current_setting('TimeZone') AS timezone,",
    " current_setting('DateStyle') AS date_style,",
    " current_setting('extra_float_digits') AS extra_float_digits",
);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DatabaseMetadata {
    pub version: String,
    pub server_version_num: String,
    pub shared_buffers: String,
    pub effective_cache_size: String,
    pub work_mem: String,
    pub random_page_cost: String,
    pub jit: String,
    pub standard_conforming_strings: String,
    pub timezone: String,
    pub date_style: String,
    pub extra_float_digits: String,
}

pub(crate) async fn read_database_metadata(db: &DatabaseConnection) -> Result<DatabaseMetadata> {
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            DATABASE_METADATA_SQL.to_owned(),
        ))
        .await?
        .context("database metadata query returned no row")?;

    Ok(DatabaseMetadata {
        version: row.try_get("", "version")?,
        server_version_num: row.try_get("", "server_version_num")?,
        shared_buffers: row.try_get("", "shared_buffers")?,
        effective_cache_size: row.try_get("", "effective_cache_size")?,
        work_mem: row.try_get("", "work_mem")?,
        random_page_cost: row.try_get("", "random_page_cost")?,
        jit: row.try_get("", "jit")?,
        standard_conforming_strings: row.try_get("", "standard_conforming_strings")?,
        timezone: row.try_get("", "timezone")?,
        date_style: row.try_get("", "date_style")?,
        extra_float_digits: row.try_get("", "extra_float_digits")?,
    })
}

pub(crate) async fn ensure_database_metadata_stable(
    db: &DatabaseConnection,
    expected: &DatabaseMetadata,
    benchmark: &str,
) -> Result<()> {
    let actual = read_database_metadata(db)
        .await
        .with_context(|| format!("failed to re-read {benchmark} database metadata"))?;
    ensure!(
        &actual == expected,
        "{benchmark} PostgreSQL database/session metadata drifted during evidence collection: expected {expected:?}, got {actual:?}"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::DATABASE_METADATA_SQL;

    #[test]
    fn database_metadata_query_observes_comparable_session_settings() {
        for marker in [
            "current_setting('server_version_num') AS server_version_num",
            "current_setting('shared_buffers') AS shared_buffers",
            "current_setting('effective_cache_size') AS effective_cache_size",
            "current_setting('work_mem') AS work_mem",
            "current_setting('random_page_cost') AS random_page_cost",
            "current_setting('jit') AS jit",
            "current_setting('standard_conforming_strings') AS standard_conforming_strings",
            "current_setting('TimeZone') AS timezone",
            "current_setting('DateStyle') AS date_style",
            "current_setting('extra_float_digits') AS extra_float_digits",
        ] {
            assert!(
                DATABASE_METADATA_SQL.contains(marker),
                "database metadata query is missing {marker}"
            );
        }
    }
}
