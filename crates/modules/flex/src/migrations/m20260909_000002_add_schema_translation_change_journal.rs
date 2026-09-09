use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Translation ChangeCursor is a PostgreSQL production capability. Keep SQLite
        // migrations usable for lightweight owner tests without pretending to provide
        // durable sequence semantics there.
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
CREATE TABLE flex_schema_translation_change_journal (
    change_seq BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    root_event_id UUID NOT NULL,
    tenant_id UUID NOT NULL,
    schema_id UUID NOT NULL,
    resource_revision VARCHAR(128) NOT NULL,
    lifecycle VARCHAR(16) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_flex_schema_translation_change_root_target
        UNIQUE (root_event_id, schema_id),
    CONSTRAINT chk_flex_schema_translation_change_seq_positive
        CHECK (change_seq > 0),
    CONSTRAINT chk_flex_schema_translation_change_root_event_non_nil
        CHECK (root_event_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_flex_schema_translation_change_tenant_non_nil
        CHECK (tenant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_flex_schema_translation_change_schema_non_nil
        CHECK (schema_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_flex_schema_translation_change_revision_nonblank
        CHECK (length(trim(resource_revision)) > 0),
    CONSTRAINT chk_flex_schema_translation_change_lifecycle
        CHECK (lifecycle IN ('active', 'archived', 'deleted'))
);

CREATE INDEX idx_flex_schema_translation_change_tenant_seq
    ON flex_schema_translation_change_journal (tenant_id, change_seq);

CREATE INDEX idx_flex_schema_translation_change_target_seq
    ON flex_schema_translation_change_journal (
        tenant_id,
        schema_id,
        change_seq DESC
    );
"#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS flex_schema_translation_change_journal;")
            .await?;
        Ok(())
    }
}
