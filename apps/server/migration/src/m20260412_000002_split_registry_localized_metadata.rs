use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const DEFAULT_REGISTRY_LOCALE: &str = "en";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_registry_translation_tables(manager).await?;
        add_registry_default_locale_columns(manager).await?;
        backfill_registry_translation_tables(manager.get_connection()).await?;
        drop_legacy_registry_metadata_columns(manager.get_connection()).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        add_legacy_registry_metadata_columns(manager).await?;
        restore_legacy_registry_metadata_columns(manager.get_connection()).await?;
        drop_registry_translation_tables(manager).await?;
        drop_registry_default_locale_columns(manager.get_connection()).await
    }
}

async fn create_registry_translation_tables(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(RegistryPublishRequestTranslations::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(RegistryPublishRequestTranslations::RequestId)
                        .string_len(64)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(RegistryPublishRequestTranslations::Locale)
                        .string_len(32)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(RegistryPublishRequestTranslations::Name)
                        .string_len(160)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(RegistryPublishRequestTranslations::Description)
                        .text()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(RegistryPublishRequestTranslations::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .col(
                    ColumnDef::new(RegistryPublishRequestTranslations::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .primary_key(
                    Index::create()
                        .col(RegistryPublishRequestTranslations::RequestId)
                        .col(RegistryPublishRequestTranslations::Locale),
                )
                .to_owned(),
        )
        .await?;

    manager
        .create_table(
            Table::create()
                .table(RegistryModuleReleaseTranslations::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(RegistryModuleReleaseTranslations::ReleaseId)
                        .string_len(64)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(RegistryModuleReleaseTranslations::Locale)
                        .string_len(32)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(RegistryModuleReleaseTranslations::Name)
                        .string_len(160)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(RegistryModuleReleaseTranslations::Description)
                        .text()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(RegistryModuleReleaseTranslations::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .col(
                    ColumnDef::new(RegistryModuleReleaseTranslations::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .primary_key(
                    Index::create()
                        .col(RegistryModuleReleaseTranslations::ReleaseId)
                        .col(RegistryModuleReleaseTranslations::Locale),
                )
                .to_owned(),
        )
        .await
}

async fn add_registry_default_locale_columns(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .alter_table(
            Table::alter()
                .table(RegistryPublishRequests::Table)
                .add_column(
                    ColumnDef::new(RegistryPublishRequests::DefaultLocale)
                        .string_len(32)
                        .not_null()
                        .default(DEFAULT_REGISTRY_LOCALE),
                )
                .to_owned(),
        )
        .await?;

    manager
        .alter_table(
            Table::alter()
                .table(RegistryModuleReleases::Table)
                .add_column(
                    ColumnDef::new(RegistryModuleReleases::DefaultLocale)
                        .string_len(32)
                        .not_null()
                        .default(DEFAULT_REGISTRY_LOCALE),
                )
                .to_owned(),
        )
        .await
}

async fn backfill_registry_translation_tables(
    db: &SchemaManagerConnection<'_>,
) -> Result<(), DbErr> {
    backfill_publish_request_translations(db).await?;
    backfill_release_translations(db).await
}

async fn backfill_publish_request_translations(
    db: &SchemaManagerConnection<'_>,
) -> Result<(), DbErr> {
    let backend = db.get_database_backend();
    let rows = db
        .query_all(Statement::from_string(
            backend,
            "SELECT id, default_locale, module_name, description FROM registry_publish_requests"
                .to_string(),
        ))
        .await?;

    for row in rows {
        let request_id = row.try_get::<String>("", "id")?;
        let locale = row.try_get::<String>("", "default_locale")?;
        let name = row.try_get::<String>("", "module_name")?;
        let description = row.try_get::<String>("", "description")?;
        execute_statement(
            db,
            "INSERT INTO registry_publish_request_translations (request_id, locale, name, description, created_at, updated_at) VALUES ({v1}, {v2}, {v3}, {v4}, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![request_id.into(), locale.into(), name.into(), description.into()],
        )
        .await?;
    }

    Ok(())
}

async fn backfill_release_translations(db: &SchemaManagerConnection<'_>) -> Result<(), DbErr> {
    let backend = db.get_database_backend();
    let rows = db
        .query_all(Statement::from_string(
            backend,
            "SELECT id, default_locale, module_name, description FROM registry_module_releases"
                .to_string(),
        ))
        .await?;

    for row in rows {
        let release_id = row.try_get::<String>("", "id")?;
        let locale = row.try_get::<String>("", "default_locale")?;
        let name = row.try_get::<String>("", "module_name")?;
        let description = row.try_get::<String>("", "description")?;
        execute_statement(
            db,
            "INSERT INTO registry_module_release_translations (release_id, locale, name, description, created_at, updated_at) VALUES ({v1}, {v2}, {v3}, {v4}, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![release_id.into(), locale.into(), name.into(), description.into()],
        )
        .await?;
    }

    Ok(())
}

async fn drop_legacy_registry_metadata_columns(
    db: &SchemaManagerConnection<'_>,
) -> Result<(), DbErr> {
    drop_columns(
        db,
        "registry_publish_requests",
        &["module_name", "description"],
    )
    .await?;
    drop_columns(
        db,
        "registry_module_releases",
        &["module_name", "description"],
    )
    .await
}

async fn add_legacy_registry_metadata_columns(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .alter_table(
            Table::alter()
                .table(RegistryPublishRequests::Table)
                .add_column(
                    ColumnDef::new(RegistryPublishRequests::ModuleName)
                        .string_len(160)
                        .null(),
                )
                .add_column(
                    ColumnDef::new(RegistryPublishRequests::Description)
                        .text()
                        .null(),
                )
                .to_owned(),
        )
        .await?;

    manager
        .alter_table(
            Table::alter()
                .table(RegistryModuleReleases::Table)
                .add_column(
                    ColumnDef::new(RegistryModuleReleases::ModuleName)
                        .string_len(160)
                        .null(),
                )
                .add_column(
                    ColumnDef::new(RegistryModuleReleases::Description)
                        .text()
                        .null(),
                )
                .to_owned(),
        )
        .await
}

async fn restore_legacy_registry_metadata_columns(
    db: &SchemaManagerConnection<'_>,
) -> Result<(), DbErr> {
    restore_publish_request_metadata(db).await?;
    restore_release_metadata(db).await
}

async fn restore_publish_request_metadata(db: &SchemaManagerConnection<'_>) -> Result<(), DbErr> {
    let backend = db.get_database_backend();
    let rows = db
        .query_all(Statement::from_string(
            backend,
            "SELECT id, default_locale FROM registry_publish_requests".to_string(),
        ))
        .await?;

    for row in rows {
        let request_id = row.try_get::<String>("", "id")?;
        let default_locale = row.try_get::<String>("", "default_locale")?;
        let translation = load_translation_row(
            db,
            "registry_publish_request_translations",
            "request_id",
            &request_id,
            &default_locale,
        )
        .await?;
        execute_statement(
            db,
            "UPDATE registry_publish_requests SET module_name = {v1}, description = {v2} WHERE id = {v3}",
            vec![
                translation.name.into(),
                translation.description.into(),
                request_id.into(),
            ],
        )
        .await?;
    }

    Ok(())
}

async fn restore_release_metadata(db: &SchemaManagerConnection<'_>) -> Result<(), DbErr> {
    let backend = db.get_database_backend();
    let rows = db
        .query_all(Statement::from_string(
            backend,
            "SELECT id, default_locale FROM registry_module_releases".to_string(),
        ))
        .await?;

    for row in rows {
        let release_id = row.try_get::<String>("", "id")?;
        let default_locale = row.try_get::<String>("", "default_locale")?;
        let translation = load_translation_row(
            db,
            "registry_module_release_translations",
            "release_id",
            &release_id,
            &default_locale,
        )
        .await?;
        execute_statement(
            db,
            "UPDATE registry_module_releases SET module_name = {v1}, description = {v2} WHERE id = {v3}",
            vec![
                translation.name.into(),
                translation.description.into(),
                release_id.into(),
            ],
        )
        .await?;
    }

    Ok(())
}

async fn drop_registry_translation_tables(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_table(
            Table::drop()
                .table(RegistryPublishRequestTranslations::Table)
                .if_exists()
                .to_owned(),
        )
        .await?;

    manager
        .drop_table(
            Table::drop()
                .table(RegistryModuleReleaseTranslations::Table)
                .if_exists()
                .to_owned(),
        )
        .await
}

async fn drop_registry_default_locale_columns(
    db: &SchemaManagerConnection<'_>,
) -> Result<(), DbErr> {
    drop_columns(db, "registry_publish_requests", &["default_locale"]).await?;
    drop_columns(db, "registry_module_releases", &["default_locale"]).await
}

async fn execute_statement(
    db: &SchemaManagerConnection<'_>,
    template: &str,
    values: Vec<sea_orm::Value>,
) -> Result<(), DbErr> {
    let backend = db.get_database_backend();
    let sql = placeholder_sql(backend, template, values.len());
    db.execute(Statement::from_sql_and_values(backend, sql, values))
        .await?;
    Ok(())
}

fn placeholder_sql(backend: DbBackend, template: &str, value_count: usize) -> String {
    let mut sql = template.to_string();
    for index in 0..value_count {
        let placeholder = match backend {
            DbBackend::Sqlite => format!("?{}", index + 1),
            _ => format!("${}", index + 1),
        };
        sql = sql.replace(&format!("{{v{}}}", index + 1), &placeholder);
    }
    sql
}

async fn drop_columns(
    db: &SchemaManagerConnection<'_>,
    table: &str,
    columns: &[&str],
) -> Result<(), DbErr> {
    let backend = db.get_database_backend();
    for column in columns {
        db.execute(Statement::from_string(
            backend,
            format!("ALTER TABLE {table} DROP COLUMN {column}"),
        ))
        .await?;
    }
    Ok(())
}

async fn load_translation_row(
    db: &SchemaManagerConnection<'_>,
    table: &str,
    owner_column: &str,
    owner_id: &str,
    default_locale: &str,
) -> Result<LocalizedMetadataRow, DbErr> {
    let backend = db.get_database_backend();
    let sql = format!(
        "SELECT locale, name, description FROM {table} WHERE {owner_column} = {{v1}} ORDER BY CASE WHEN locale = {{v2}} THEN 0 ELSE 1 END, locale ASC LIMIT 1"
    );
    let sql = placeholder_sql(backend, &sql, 2);
    let row = db
        .query_one(Statement::from_sql_and_values(
            backend,
            sql,
            vec![owner_id.into(), default_locale.into()],
        ))
        .await?
        .ok_or_else(|| {
            DbErr::Custom(format!(
                "missing registry translation row in {table} for {owner_column}={owner_id}"
            ))
        })?;

    Ok(LocalizedMetadataRow {
        locale: row.try_get("", "locale")?,
        name: row.try_get("", "name")?,
        description: row.try_get("", "description")?,
    })
}

struct LocalizedMetadataRow {
    #[allow(dead_code)]
    locale: String,
    name: String,
    description: String,
}

#[derive(DeriveIden)]
enum RegistryPublishRequests {
    Table,
    DefaultLocale,
    ModuleName,
    Description,
}

#[derive(DeriveIden)]
enum RegistryModuleReleases {
    Table,
    DefaultLocale,
    ModuleName,
    Description,
}

#[derive(DeriveIden)]
enum RegistryPublishRequestTranslations {
    Table,
    RequestId,
    Locale,
    Name,
    Description,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum RegistryModuleReleaseTranslations {
    Table,
    ReleaseId,
    Locale,
    Name,
    Description,
    CreatedAt,
    UpdatedAt,
}
