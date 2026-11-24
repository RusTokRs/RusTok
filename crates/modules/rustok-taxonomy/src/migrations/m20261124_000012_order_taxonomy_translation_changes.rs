use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{DbBackend, TransactionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

const ADD_POSITION: &str = "
    ALTER TABLE taxonomy_translation_changes
        ADD COLUMN position BIGINT CHECK (position > 0);

    WITH ranked AS (
        SELECT id, ROW_NUMBER() OVER (
            PARTITION BY tenant_id
            ORDER BY term_id, created_at, resource_revision, id
        ) AS position
        FROM taxonomy_translation_changes
    )
    UPDATE taxonomy_translation_changes
        SET position = ranked.position
        FROM ranked
        WHERE taxonomy_translation_changes.id = ranked.id;

    CREATE UNIQUE INDEX idx_taxonomy_translation_changes_tenant_position
        ON taxonomy_translation_changes (tenant_id, position);

    CREATE TABLE taxonomy_translation_journal_heads (
        tenant_id UUID PRIMARY KEY NOT NULL,
        position BIGINT NOT NULL CHECK (position > 0)
    );
    INSERT INTO taxonomy_translation_journal_heads (tenant_id, position)
        SELECT tenant_id, MAX(position) FROM taxonomy_translation_changes GROUP BY tenant_id;
";

// Take the journal lock only at commit. Owner transactions can still write
// multiple terms without taking this lock between aggregate writes.
const POSTGRES_ASSIGN_POSITION: &str = "
    CREATE FUNCTION taxonomy_assign_translation_change_position() RETURNS trigger
    LANGUAGE plpgsql SET search_path FROM CURRENT AS $$
    DECLARE
        next_position BIGINT;
    BEGIN
        INSERT INTO taxonomy_translation_journal_heads (tenant_id, position) VALUES (NEW.tenant_id, 1)
        ON CONFLICT (tenant_id) DO UPDATE
        SET position = taxonomy_translation_journal_heads.position + 1
        RETURNING position INTO next_position;

        UPDATE taxonomy_translation_changes
        SET position = next_position
        WHERE id = NEW.id;
        RETURN NULL;
    END;
    $$;

    CREATE CONSTRAINT TRIGGER taxonomy_assign_translation_change_position
        AFTER INSERT ON taxonomy_translation_changes
        DEFERRABLE INITIALLY DEFERRED
        FOR EACH ROW EXECUTE FUNCTION taxonomy_assign_translation_change_position();
";

// SQLite serializes the entire write transaction.
const SQLITE_ASSIGN_POSITION: &str = "
    CREATE TRIGGER taxonomy_assign_translation_change_position
        AFTER INSERT ON taxonomy_translation_changes
    BEGIN
        INSERT INTO taxonomy_translation_journal_heads (tenant_id, position) VALUES (NEW.tenant_id, 1)
        ON CONFLICT (tenant_id) DO UPDATE SET position = position + 1;

        UPDATE taxonomy_translation_changes
        SET position = (SELECT position FROM taxonomy_translation_journal_heads WHERE tenant_id = NEW.tenant_id)
        WHERE id = NEW.id;
    END;
";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let assignment = match manager.get_database_backend() {
            DbBackend::Postgres => POSTGRES_ASSIGN_POSITION,
            DbBackend::Sqlite => SQLITE_ASSIGN_POSITION,
            backend => {
                return Err(DbErr::Custom(format!(
                    "Taxonomy translation change ordering is unsupported on {backend:?}"
                )));
            }
        };
        let transaction = manager.get_connection().begin().await?;
        transaction.execute_unprepared(ADD_POSITION).await?;
        transaction.execute_unprepared(assignment).await?;
        transaction.commit().await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let assignment = match manager.get_database_backend() {
            DbBackend::Postgres => "
                DROP TRIGGER taxonomy_assign_translation_change_position ON taxonomy_translation_changes;
                DROP FUNCTION taxonomy_assign_translation_change_position();
            ",
            DbBackend::Sqlite => "DROP TRIGGER taxonomy_assign_translation_change_position;",
            backend => {
                return Err(DbErr::Custom(format!(
                    "Taxonomy translation change ordering is unsupported on {backend:?}"
                )));
            }
        };
        let transaction = manager.get_connection().begin().await?;
        transaction.execute_unprepared(assignment).await?;
        transaction
            .execute_unprepared(
                "DROP INDEX idx_taxonomy_translation_changes_tenant_position;
                 DROP TABLE taxonomy_translation_journal_heads;
                 ALTER TABLE taxonomy_translation_changes DROP COLUMN position;",
            )
            .await?;
        transaction.commit().await
    }
}
