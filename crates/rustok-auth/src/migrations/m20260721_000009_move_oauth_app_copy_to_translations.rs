use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;
use uuid::Uuid;

use super::m20260308_000001_create_oauth_apps::OAuthApps;
use super::shared::Tenants;

const LEGACY_UNDETERMINED_LOCALE: &str = "und";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
                            .name("fk_oauth_app_translations_tenant")
                            .from(
                                OAuthAppTranslations::Table,
                                OAuthAppTranslations::TenantId,
                            )
                            .to(Tenants::Table, Tenants::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_oauth_app_translations_app")
                            .from(OAuthAppTranslations::Table, OAuthAppTranslations::AppId)
                            .to(OAuthApps::Table, OAuthApps::Id)
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

            db.execute(Statement::from_sql_and_values(
                backend,
                "INSERT INTO oauth_app_translations \
                 (id, tenant_id, app_id, locale, name, description, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
                    .to_string(),
                vec![
                    Uuid::new_v4().into(),
                    tenant_id.into(),
                    app_id.into(),
                    LEGACY_UNDETERMINED_LOCALE.into(),
                    name.into(),
                    description.into(),
                ],
            ))
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
                "SELECT id FROM oauth_apps".to_string(),
            ))
            .await?;

        for app in apps {
            let app_id: Uuid = app.try_get("", "id")?;
            let translation = db
                .query_one(Statement::from_sql_and_values(
                    backend,
                    "SELECT name, description FROM oauth_app_translations \
                     WHERE app_id = ? \
                     ORDER BY CASE WHEN locale = 'und' THEN 0 ELSE 1 END, locale \
                     LIMIT 1"
                        .to_string(),
                    vec![app_id.into()],
                ))
                .await?
                .ok_or_else(|| {
                    DbErr::Custom(format!(
                        "cannot restore oauth_apps display copy for {app_id}: no translation exists"
                    ))
                })?;
            let name: String = translation.try_get("", "name")?;
            let description: Option<String> = translation.try_get("", "description")?;
            db.execute(Statement::from_sql_and_values(
                backend,
                "UPDATE oauth_apps SET name = ?, description = ? WHERE id = ?".to_string(),
                vec![name.into(), description.into(), app_id.into()],
            ))
            .await?;
        }

        manager
            .drop_table(
                Table::drop()
                    .table(OAuthAppTranslations::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
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
