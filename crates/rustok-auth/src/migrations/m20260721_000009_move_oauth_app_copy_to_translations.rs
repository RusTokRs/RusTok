use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;
use uuid::Uuid;

use super::m20260308_000001_create_oauth_apps::OAuthApps;

const LEGACY_UNDETERMINED_LOCALE: &str = "und";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // A composite parent identity is required so a translation cannot pair a
        // tenant with an OAuth application owned by another tenant.
        manager
            .create_index(
                Index::create()
                    .name("uq_oauth_apps_tenant_id_translation_parent")
                    .table(OAuthApps::Table)
                    .col(OAuthApps::TenantId)
                    .col(OAuthApps::Id)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(OAuthAppTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OAuthAppTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(OAuthAppTranslations::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthAppTranslations::AppId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthAppTranslations::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OAuthAppTranslations::Name)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(OAuthAppTranslations::Description).text())
                    .col(
                        ColumnDef::new(OAuthAppTranslations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(OAuthAppTranslations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oauth_app_translations_tenant_app")
                            .from_tbl(OAuthAppTranslations::Table)
                            .from_col(OAuthAppTranslations::TenantId)
                            .from_col(OAuthAppTranslations::AppId)
                            .to_tbl(OAuthApps::Table)
                            .to_col(OAuthApps::TenantId)
                            .to_col(OAuthApps::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_oauth_app_translations_tenant_app_locale")
                    .table(OAuthAppTranslations::Table)
                    .col(OAuthAppTranslations::TenantId)
                    .col(OAuthAppTranslations::AppId)
                    .col(OAuthAppTranslations::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;

        let db = manager.get_connection();
        let backend = db.get_database_backend();
        let apps = db
            .query_all(Statement::from_string(
                backend,
                "SELECT id, tenant_id, name, description FROM oauth_apps".to_string(),
            ))
            .await?;

        for app in apps {
            let app_id: Uuid = app.try_get("", "id")?;
            let tenant_id: Uuid = app.try_get("", "tenant_id")?;
            let name: String = app.try_get("", "name")?;
            let description: Option<String> = app.try_get("", "description")?;

            execute_statement(
                db,
                "INSERT INTO oauth_app_translations (id, tenant_id, app_id, locale, name, description, created_at, updated_at) VALUES ({v1}, {v2}, {v3}, {v4}, {v5}, {v6}, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
                vec![
                    Uuid::new_v4().into(),
                    tenant_id.into(),
                    app_id.into(),
                    LEGACY_UNDETERMINED_LOCALE.into(),
                    name.into(),
                    description.into(),
                ],
            )
            .await?;
        }

        manager
            .alter_table(
                Table::alter()
                    .table(OAuthApps::Table)
                    .drop_column(OAuthApps::Name)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(OAuthApps::Table)
                    .drop_column(OAuthApps::Description)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(OAuthApps::Table)
                    .add_column(ColumnDef::new(OAuthApps::Name).string_len(255).null())
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(OAuthApps::Table)
                    .add_column(ColumnDef::new(OAuthApps::Description).text().null())
                    .to_owned(),
            )
            .await?;

        let db = manager.get_connection();
        let backend = db.get_database_backend();
        let apps = db
            .query_all(Statement::from_string(
                backend,
                "SELECT id, tenant_id FROM oauth_apps".to_string(),
            ))
            .await?;

        for app in apps {
            let app_id: Uuid = app.try_get("", "id")?;
            let tenant_id: Uuid = app.try_get("", "tenant_id")?;
            let translation = query_one(
                db,
                "SELECT name, description FROM oauth_app_translations WHERE tenant_id = {v1} AND app_id = {v2} ORDER BY CASE WHEN locale = {v3} THEN 0 ELSE 1 END, locale LIMIT 1",
                vec![
                    tenant_id.into(),
                    app_id.into(),
                    LEGACY_UNDETERMINED_LOCALE.into(),
                ],
            )
            .await?
            .ok_or_else(|| {
                DbErr::Custom(format!(
                    "cannot restore oauth_apps display copy for tenant {tenant_id}, app {app_id}: no translation exists"
                ))
            })?;
            let name: String = translation.try_get("", "name")?;
            let description: Option<String> = translation.try_get("", "description")?;
            execute_statement(
                db,
                "UPDATE oauth_apps SET name = {v1}, description = {v2} WHERE tenant_id = {v3} AND id = {v4}",
                vec![name.into(), description.into(), tenant_id.into(), app_id.into()],
            )
            .await?;
        }

        manager
            .drop_table(
                Table::drop()
                    .table(OAuthAppTranslations::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("uq_oauth_apps_tenant_id_translation_parent")
                    .table(OAuthApps::Table)
                    .to_owned(),
            )
            .await
    }
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

async fn query_one(
    db: &SchemaManagerConnection<'_>,
    template: &str,
    values: Vec<sea_orm::Value>,
) -> Result<Option<sea_orm::QueryResult>, DbErr> {
    let backend = db.get_database_backend();
    let sql = placeholder_sql(backend, template, values.len());
    db.query_one(Statement::from_sql_and_values(backend, sql, values))
        .await
}

fn placeholder_sql(backend: DbBackend, template: &str, value_count: usize) -> String {
    let mut sql = template.to_string();
    for index in 0..value_count {
        let placeholder = match backend {
            DbBackend::Postgres => format!("${}", index + 1),
            DbBackend::MySql => "?".to_string(),
            DbBackend::Sqlite => format!("?{}", index + 1),
        };
        sql = sql.replace(&format!("{{v{}}}", index + 1), &placeholder);
    }
    sql
}

#[derive(DeriveIden)]
enum OAuthAppTranslations {
    Table,
    Id,
    TenantId,
    AppId,
    Locale,
    Name,
    Description,
    CreatedAt,
    UpdatedAt,
}
