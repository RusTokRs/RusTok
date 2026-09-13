use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ScriptPresentations::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(ScriptPresentations::TenantId).uuid().not_null())
                    .col(ColumnDef::new(ScriptPresentations::ScriptId).uuid().not_null())
                    .col(
                        ColumnDef::new(ScriptPresentations::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(ColumnDef::new(ScriptPresentations::Description).text())
                    .col(
                        ColumnDef::new(ScriptPresentations::CopyRevision)
                            .big_integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(ScriptPresentations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ScriptPresentations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(ScriptPresentations::TenantId)
                            .col(ScriptPresentations::ScriptId)
                            .col(ScriptPresentations::Locale),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_alloy_script_presentations_script")
                            .from(ScriptPresentations::Table, ScriptPresentations::ScriptId)
                            .to(Scripts::Table, Scripts::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_alloy_script_presentations_script")
                    .table(ScriptPresentations::Table)
                    .col(ScriptPresentations::ScriptId)
                    .to_owned(),
            )
            .await?;

        // Existing inline descriptions predate source-locale provenance. Keep
        // them truthfully under the storage-only `und` locale rather than
        // guessing English or a tenant default.
        let connection = manager.get_connection();
        let backend = connection.get_database_backend();
        connection
            .execute_raw(Statement::from_string(
                backend,
                r#"
INSERT INTO alloy_script_presentations (
    tenant_id,
    script_id,
    locale,
    description,
    copy_revision,
    created_at,
    updated_at
)
SELECT
    tenant_id,
    id,
    'und',
    description,
    1,
    created_at,
    updated_at
FROM scripts
WHERE description IS NOT NULL
ON CONFLICT (tenant_id, script_id, locale) DO NOTHING
"#
                .to_owned(),
            ))
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ScriptPresentations::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum Scripts {
    #[iden = "scripts"]
    Table,
    Id,
}

#[derive(Iden)]
enum ScriptPresentations {
    #[iden = "alloy_script_presentations"]
    Table,
    TenantId,
    ScriptId,
    Locale,
    Description,
    CopyRevision,
    CreatedAt,
    UpdatedAt,
}
