use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use sea_orm_migration::prelude::*;

use crate::domain::BLOG_CATEGORY_SETTINGS_MAX_BYTES;

const SETTINGS_OBJECT_CONSTRAINT: &str = "ck_blog_categories_settings_object";
const SETTINGS_SIZE_CONSTRAINT: &str = "ck_blog_categories_settings_size";
const SETTINGS_INSERT_TRIGGER: &str = "blog_categories_settings_contract_insert";
const SETTINGS_UPDATE_TRIGGER: &str = "blog_categories_settings_contract_update";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        validate_existing_settings(manager).await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "Blog Category settings contract migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "Blog Category settings contract migration does not support {backend:?}"
            ))),
        }
    }
}

async fn validate_existing_settings(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    let query = match connection.get_database_backend() {
        DatabaseBackend::Postgres => format!(
            r#"
SELECT COUNT(*) AS invalid_count
FROM blog_categories
WHERE jsonb_typeof(settings) <> 'object'
   OR octet_length(settings::text) > {BLOG_CATEGORY_SETTINGS_MAX_BYTES}
"#
        ),
        DatabaseBackend::Sqlite => format!(
            r#"
SELECT COUNT(*) AS invalid_count
FROM blog_categories
WHERE CASE
    WHEN json_valid(settings) = 0 THEN 1
    WHEN json_type(settings) <> 'object' THEN 1
    WHEN length(CAST(json(settings) AS BLOB)) > {BLOG_CATEGORY_SETTINGS_MAX_BYTES} THEN 1
    ELSE 0
END = 1
"#
        ),
        backend => {
            return Err(DbErr::Custom(format!(
                "Blog Category settings contract migration does not support {backend:?}"
            )));
        }
    };

    let row = connection
        .query_one_raw(Statement::from_string(
            connection.get_database_backend(),
            query,
        ))
        .await?
        .ok_or_else(|| {
            DbErr::Custom("failed to validate Blog Category settings contract".to_string())
        })?;
    let invalid_count: i64 = row.try_get("", "invalid_count")?;

    if invalid_count != 0 {
        return Err(DbErr::Migration(format!(
            "Blog Category settings contract migration blocked: {invalid_count} invalid row(s) exist; settings must be JSON objects no larger than {} bytes",
            BLOG_CATEGORY_SETTINGS_MAX_BYTES
        )));
    }

    Ok(())
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            r#"
ALTER TABLE blog_categories
    ADD CONSTRAINT {SETTINGS_OBJECT_CONSTRAINT}
    CHECK (jsonb_typeof(settings) = 'object');
"#
        ))
        .await?;

    manager
        .get_connection()
        .execute_unprepared(&format!(
            r#"
ALTER TABLE blog_categories
    ADD CONSTRAINT {SETTINGS_SIZE_CONSTRAINT}
    CHECK (octet_length(settings::text) <= {BLOG_CATEGORY_SETTINGS_MAX_BYTES});
"#
        ))
        .await?;

    Ok(())
}

async fn down_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            "ALTER TABLE blog_categories DROP CONSTRAINT IF EXISTS {SETTINGS_SIZE_CONSTRAINT}"
        ))
        .await?;
    manager
        .get_connection()
        .execute_unprepared(&format!(
            "ALTER TABLE blog_categories DROP CONSTRAINT IF EXISTS {SETTINGS_OBJECT_CONSTRAINT}"
        ))
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            r#"
CREATE TRIGGER {SETTINGS_INSERT_TRIGGER}
BEFORE INSERT ON blog_categories
FOR EACH ROW
WHEN CASE
    WHEN json_valid(NEW.settings) = 0 THEN 1
    WHEN json_type(NEW.settings) <> 'object' THEN 1
    WHEN length(CAST(json(NEW.settings) AS BLOB)) > {BLOG_CATEGORY_SETTINGS_MAX_BYTES} THEN 1
    ELSE 0
END = 1
BEGIN
    SELECT RAISE(ABORT, 'blog category settings must be a JSON object no larger than 64 KiB');
END;
"#
        ))
        .await?;

    manager
        .get_connection()
        .execute_unprepared(&format!(
            r#"
CREATE TRIGGER {SETTINGS_UPDATE_TRIGGER}
BEFORE UPDATE OF settings ON blog_categories
FOR EACH ROW
WHEN CASE
    WHEN json_valid(NEW.settings) = 0 THEN 1
    WHEN json_type(NEW.settings) <> 'object' THEN 1
    WHEN length(CAST(json(NEW.settings) AS BLOB)) > {BLOG_CATEGORY_SETTINGS_MAX_BYTES} THEN 1
    ELSE 0
END = 1
BEGIN
    SELECT RAISE(ABORT, 'blog category settings must be a JSON object no larger than 64 KiB');
END;
"#
        ))
        .await?;

    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            "DROP TRIGGER IF EXISTS {SETTINGS_UPDATE_TRIGGER}"
        ))
        .await?;
    manager
        .get_connection()
        .execute_unprepared(&format!(
            "DROP TRIGGER IF EXISTS {SETTINGS_INSERT_TRIGGER}"
        ))
        .await?;
    Ok(())
}
