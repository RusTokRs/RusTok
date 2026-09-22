use crate::{oauth_app_translation_lifecycle, oauth_app_translation_resource_revision};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use sea_orm_migration::prelude::*;
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let mut table = Table::create();
        table
            .table(OAuthAppTranslationChangeJournal::Table)
            .if_not_exists();
        if manager.get_database_backend() == DatabaseBackend::Sqlite {
            table.col(
                ColumnDef::new(OAuthAppTranslationChangeJournal::ChangeSeq)
                    .integer()
                    .not_null()
                    .auto_increment()
                    .primary_key(),
            );
        } else {
            table.col(
                ColumnDef::new(OAuthAppTranslationChangeJournal::ChangeSeq)
                    .big_integer()
                    .not_null()
                    .auto_increment()
                    .primary_key(),
            );
        }
        table
            .col(ColumnDef::new(OAuthAppTranslationChangeJournal::OperationId).uuid())
            .col(
                ColumnDef::new(OAuthAppTranslationChangeJournal::TenantId)
                    .uuid()
                    .not_null(),
            )
            .col(
                ColumnDef::new(OAuthAppTranslationChangeJournal::AppId)
                    .uuid()
                    .not_null(),
            )
            .col(
                ColumnDef::new(OAuthAppTranslationChangeJournal::ResourceRevision)
                    .string_len(96)
                    .not_null(),
            )
            .col(
                ColumnDef::new(OAuthAppTranslationChangeJournal::Lifecycle)
                    .string_len(16)
                    .not_null(),
            )
            .col(
                ColumnDef::new(OAuthAppTranslationChangeJournal::CreatedAt)
                    .timestamp_with_time_zone()
                    .not_null()
                    .default(Expr::current_timestamp()),
            );
        manager.create_table(table.to_owned()).await?;

        for index in [
            Index::create()
                .name("ux_oauth_app_translation_change_operation")
                .table(OAuthAppTranslationChangeJournal::Table)
                .col(OAuthAppTranslationChangeJournal::OperationId)
                .col(OAuthAppTranslationChangeJournal::AppId)
                .unique()
                .to_owned(),
            Index::create()
                .name("idx_oauth_app_translation_change_tenant_seq")
                .table(OAuthAppTranslationChangeJournal::Table)
                .col(OAuthAppTranslationChangeJournal::TenantId)
                .col(OAuthAppTranslationChangeJournal::ChangeSeq)
                .to_owned(),
            Index::create()
                .name("idx_oauth_app_translation_change_app_seq")
                .table(OAuthAppTranslationChangeJournal::Table)
                .col(OAuthAppTranslationChangeJournal::TenantId)
                .col(OAuthAppTranslationChangeJournal::AppId)
                .col(OAuthAppTranslationChangeJournal::ChangeSeq)
                .to_owned(),
        ] {
            manager.create_index(index).await?;
        }

        backfill_existing_resources(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(OAuthAppTranslationChangeJournal::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

async fn backfill_existing_resources(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let db = manager.get_connection();
    let backend = db.get_database_backend();
    let apps = db
        .query_all_raw(Statement::from_string(
            backend,
            "SELECT id, tenant_id, is_active, revoked_at FROM oauth_apps ORDER BY tenant_id, id"
                .to_string(),
        ))
        .await?;

    for app in apps {
        let app_id: Uuid = app.try_get("", "id")?;
        let tenant_id: Uuid = app.try_get("", "tenant_id")?;
        let is_active: bool = app.try_get("", "is_active")?;
        let revoked_at: Option<chrono::DateTime<chrono::FixedOffset>> =
            app.try_get("", "revoked_at")?;
        let translations = query_translations(db, backend, tenant_id, app_id).await?;
        if translations.is_empty() {
            continue;
        }
        let revision = oauth_app_translation_resource_revision(
            tenant_id,
            app_id,
            translations
                .into_iter()
                .map(|row| (row.locale, row.name, row.description)),
        );
        let lifecycle = oauth_app_translation_lifecycle(is_active, revoked_at.is_some());
        execute(
            db,
            backend,
            "INSERT INTO oauth_app_translation_change_journal (tenant_id, app_id, resource_revision, lifecycle) VALUES ({v1}, {v2}, {v3}, {v4})",
            vec![
                tenant_id.into(),
                app_id.into(),
                revision.into(),
                lifecycle.as_str().into(),
            ],
        )
        .await?;
    }
    Ok(())
}

struct TranslationRow {
    locale: String,
    name: String,
    description: Option<String>,
}

async fn query_translations(
    db: &SchemaManagerConnection<'_>,
    backend: DatabaseBackend,
    tenant_id: Uuid,
    app_id: Uuid,
) -> Result<Vec<TranslationRow>, DbErr> {
    let sql = placeholder_sql(
        backend,
        "SELECT locale, name, description FROM oauth_app_translations WHERE tenant_id = {v1} AND app_id = {v2} ORDER BY locale",
        2,
    );
    db.query_all_raw(Statement::from_sql_and_values(
        backend,
        sql,
        vec![tenant_id.into(), app_id.into()],
    ))
    .await?
    .into_iter()
    .map(|row| {
        Ok(TranslationRow {
            locale: row.try_get("", "locale")?,
            name: row.try_get("", "name")?,
            description: row.try_get("", "description")?,
        })
    })
    .collect()
}

async fn execute(
    db: &SchemaManagerConnection<'_>,
    backend: DatabaseBackend,
    template: &str,
    values: Vec<sea_orm::Value>,
) -> Result<(), DbErr> {
    let sql = placeholder_sql(backend, template, values.len());
    db.execute_raw(Statement::from_sql_and_values(backend, sql, values))
        .await?;
    Ok(())
}

fn placeholder_sql(backend: DatabaseBackend, template: &str, value_count: usize) -> String {
    let mut sql = template.to_string();
    for index in 0..value_count {
        let placeholder = match backend {
            DatabaseBackend::Postgres => format!("${}", index + 1),
            DatabaseBackend::MySql => "?".to_string(),
            DatabaseBackend::Sqlite => format!("?{}", index + 1),
            _ => unreachable!("unsupported SeaORM database backend"),
        };
        sql = sql.replace(&format!("{{v{}}}", index + 1), &placeholder);
    }
    sql
}

#[derive(DeriveIden)]
enum OAuthAppTranslationChangeJournal {
    Table,
    ChangeSeq,
    OperationId,
    TenantId,
    AppId,
    ResourceRevision,
    Lifecycle,
    CreatedAt,
}
