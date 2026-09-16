use sea_orm::DatabaseBackend;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Drop table forum_category_taxonomy_bindings
        manager
            .drop_table(
                Table::drop()
                    .table(ForumCategoryTaxonomyBindings::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        // 2. Drop parent_id, position, icon, color based on backend and update triggers
        let db = manager.get_connection();
        if manager.get_database_backend() == DatabaseBackend::Sqlite {
            db.execute_unprepared(
                r#"
DROP TRIGGER IF EXISTS forum_category_lifecycle_write_insert;
DROP TRIGGER IF EXISTS forum_category_lifecycle_write_update;
DROP TRIGGER IF EXISTS forum_category_lifecycle_delete;
DROP TRIGGER IF EXISTS forum_categories_parent_lifecycle_insert;
DROP TRIGGER IF EXISTS forum_categories_parent_lifecycle_update;
DROP TRIGGER IF EXISTS forum_reject_nonempty_category_delete;

DROP INDEX IF EXISTS idx_forum_categories_tenant_parent_position;
PRAGMA legacy_alter_table = ON;
CREATE TABLE forum_categories_clean (
    id TEXT PRIMARY KEY NOT NULL,
    tenant_id TEXT NOT NULL,
    moderated BOOLEAN NOT NULL DEFAULT 0,
    topic_count INTEGER NOT NULL DEFAULT 0,
    reply_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    updated_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);
INSERT INTO forum_categories_clean (id, tenant_id, moderated, topic_count, reply_count, created_at, updated_at)
    SELECT id, tenant_id, moderated, topic_count, reply_count, created_at, updated_at FROM forum_categories;
DROP TABLE forum_categories;
ALTER TABLE forum_categories_clean RENAME TO forum_categories;
CREATE UNIQUE INDEX IF NOT EXISTS uq_forum_categories_tenant_id ON forum_categories (tenant_id, id);
PRAGMA legacy_alter_table = OFF;

CREATE TRIGGER forum_reject_nonempty_category_delete
BEFORE DELETE ON forum_categories
FOR EACH ROW
WHEN EXISTS (
    SELECT 1
    FROM forum_topics topic
    WHERE topic.tenant_id = OLD.tenant_id
      AND topic.category_id = OLD.id
)
BEGIN
    SELECT RAISE(ABORT, 'non-empty forum category cannot be physically deleted');
END;
"#,
            )
            .await?;
        } else {
            db.execute_unprepared(
                r#"
DROP TRIGGER IF EXISTS forum_00_reject_nonempty_category_delete ON forum_categories;
DROP TRIGGER IF EXISTS forum_categories_parent_lifecycle_guard ON forum_categories;
DROP TRIGGER IF EXISTS forum_category_lifecycle_write_guard ON forum_category_lifecycle;
DROP TRIGGER IF EXISTS forum_category_lifecycle_delete_guard ON forum_category_lifecycle;
DROP FUNCTION IF EXISTS forum_reject_nonempty_category_delete();
DROP FUNCTION IF EXISTS forum_validate_category_parent_lifecycle();
DROP FUNCTION IF EXISTS forum_validate_category_lifecycle_write();
DROP FUNCTION IF EXISTS forum_validate_category_lifecycle_delete();

DROP INDEX IF EXISTS idx_forum_categories_tenant_parent_position;
ALTER TABLE forum_categories DROP CONSTRAINT IF EXISTS fk_forum_categories_parent_tenant;
ALTER TABLE forum_categories DROP CONSTRAINT IF EXISTS fk_forum_categories_parent;
ALTER TABLE forum_categories DROP COLUMN IF EXISTS parent_id;
ALTER TABLE forum_categories DROP COLUMN IF EXISTS position;
ALTER TABLE forum_categories DROP COLUMN IF EXISTS icon;
ALTER TABLE forum_categories DROP COLUMN IF EXISTS color;

CREATE OR REPLACE FUNCTION forum_reject_nonempty_category_delete()
RETURNS trigger AS $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM forum_topics topic
        WHERE topic.tenant_id = OLD.tenant_id
          AND topic.category_id = OLD.id
    ) THEN
        RAISE EXCEPTION 'non-empty forum category cannot be physically deleted';
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER forum_00_reject_nonempty_category_delete
BEFORE DELETE ON forum_categories
FOR EACH ROW EXECUTE FUNCTION forum_reject_nonempty_category_delete();
"#,
            )
            .await?;
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Intentionally irreversible under Zero-Legacy Policy.
        // Taxonomy is the single canonical source of truth for Category hierarchy and presentation.
        Ok(())
    }
}

#[derive(Iden)]
enum ForumCategoryTaxonomyBindings {
    #[iden = "forum_category_taxonomy_bindings"]
    Table,
}
