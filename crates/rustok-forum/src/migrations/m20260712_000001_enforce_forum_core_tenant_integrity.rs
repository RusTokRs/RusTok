use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            // SQLite is the lightweight development/test profile. Existing SQLite
            // databases cannot receive composite foreign keys through ALTER TABLE.
            // Clean SQLite schema parity is handled in FORUM-01B by rebuilding the
            // affected child tables.
            return Ok(());
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM forum_categories child
        JOIN forum_categories parent ON parent.id = child.parent_id
        WHERE child.parent_id IS NOT NULL
          AND child.tenant_id IS DISTINCT FROM parent.tenant_id
    ) THEN
        RAISE EXCEPTION
            'forum tenant-integrity migration blocked: category parent tenant mismatch';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM forum_category_translations translation
        JOIN forum_categories category ON category.id = translation.category_id
        WHERE translation.tenant_id IS DISTINCT FROM category.tenant_id
    ) THEN
        RAISE EXCEPTION
            'forum tenant-integrity migration blocked: category translation tenant mismatch';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM forum_topics topic
        JOIN forum_categories category ON category.id = topic.category_id
        WHERE topic.tenant_id IS DISTINCT FROM category.tenant_id
    ) THEN
        RAISE EXCEPTION
            'forum tenant-integrity migration blocked: topic category tenant mismatch';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM forum_replies reply
        JOIN forum_topics topic ON topic.id = reply.topic_id
        WHERE reply.tenant_id IS DISTINCT FROM topic.tenant_id
    ) THEN
        RAISE EXCEPTION
            'forum tenant-integrity migration blocked: reply topic tenant mismatch';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM forum_replies child
        JOIN forum_replies parent ON parent.id = child.parent_reply_id
        WHERE child.parent_reply_id IS NOT NULL
          AND child.tenant_id IS DISTINCT FROM parent.tenant_id
    ) THEN
        RAISE EXCEPTION
            'forum tenant-integrity migration blocked: parent reply tenant mismatch';
    END IF;
END $$;

ALTER TABLE forum_category_translations
    ALTER COLUMN locale TYPE VARCHAR(32);
ALTER TABLE forum_topic_translations
    ALTER COLUMN locale TYPE VARCHAR(32);
ALTER TABLE forum_reply_bodies
    ALTER COLUMN locale TYPE VARCHAR(32);

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'uq_forum_categories_tenant_id'
    ) THEN
        ALTER TABLE forum_categories
            ADD CONSTRAINT uq_forum_categories_tenant_id
            UNIQUE (tenant_id, id);
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'uq_forum_topics_tenant_id'
    ) THEN
        ALTER TABLE forum_topics
            ADD CONSTRAINT uq_forum_topics_tenant_id
            UNIQUE (tenant_id, id);
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'uq_forum_replies_tenant_id'
    ) THEN
        ALTER TABLE forum_replies
            ADD CONSTRAINT uq_forum_replies_tenant_id
            UNIQUE (tenant_id, id);
    END IF;
END $$;

ALTER TABLE forum_categories
    DROP CONSTRAINT IF EXISTS fk_forum_categories_parent;
ALTER TABLE forum_categories
    DROP CONSTRAINT IF EXISTS fk_forum_categories_parent_tenant;
ALTER TABLE forum_category_translations
    DROP CONSTRAINT IF EXISTS fk_forum_category_translations_category;
ALTER TABLE forum_category_translations
    DROP CONSTRAINT IF EXISTS fk_forum_category_translations_category_tenant;
ALTER TABLE forum_topics
    DROP CONSTRAINT IF EXISTS fk_forum_topics_category;
ALTER TABLE forum_topics
    DROP CONSTRAINT IF EXISTS fk_forum_topics_category_tenant;
ALTER TABLE forum_replies
    DROP CONSTRAINT IF EXISTS fk_forum_replies_topic;
ALTER TABLE forum_replies
    DROP CONSTRAINT IF EXISTS fk_forum_replies_topic_tenant;
ALTER TABLE forum_replies
    DROP CONSTRAINT IF EXISTS fk_forum_replies_parent_reply;
ALTER TABLE forum_replies
    DROP CONSTRAINT IF EXISTS fk_forum_replies_parent_reply_tenant;

ALTER TABLE forum_categories
    ADD CONSTRAINT fk_forum_categories_parent_tenant
    FOREIGN KEY (tenant_id, parent_id)
    REFERENCES forum_categories (tenant_id, id)
    ON UPDATE CASCADE
    ON DELETE RESTRICT;

ALTER TABLE forum_category_translations
    ADD CONSTRAINT fk_forum_category_translations_category_tenant
    FOREIGN KEY (tenant_id, category_id)
    REFERENCES forum_categories (tenant_id, id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;

ALTER TABLE forum_topics
    ADD CONSTRAINT fk_forum_topics_category_tenant
    FOREIGN KEY (tenant_id, category_id)
    REFERENCES forum_categories (tenant_id, id)
    ON UPDATE CASCADE
    ON DELETE RESTRICT;

ALTER TABLE forum_replies
    ADD CONSTRAINT fk_forum_replies_topic_tenant
    FOREIGN KEY (tenant_id, topic_id)
    REFERENCES forum_topics (tenant_id, id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;

ALTER TABLE forum_replies
    ADD CONSTRAINT fk_forum_replies_parent_reply_tenant
    FOREIGN KEY (tenant_id, parent_reply_id)
    REFERENCES forum_replies (tenant_id, id)
    ON UPDATE CASCADE
    ON DELETE RESTRICT;

DROP INDEX IF EXISTS idx_forum_category_translations_category_locale;
CREATE UNIQUE INDEX IF NOT EXISTS
    uq_forum_category_translations_tenant_category_locale
    ON forum_category_translations (tenant_id, category_id, locale);
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
            .execute_unprepared(
                r#"
ALTER TABLE forum_categories
    DROP CONSTRAINT IF EXISTS fk_forum_categories_parent_tenant;
ALTER TABLE forum_category_translations
    DROP CONSTRAINT IF EXISTS fk_forum_category_translations_category_tenant;
ALTER TABLE forum_topics
    DROP CONSTRAINT IF EXISTS fk_forum_topics_category_tenant;
ALTER TABLE forum_replies
    DROP CONSTRAINT IF EXISTS fk_forum_replies_topic_tenant;
ALTER TABLE forum_replies
    DROP CONSTRAINT IF EXISTS fk_forum_replies_parent_reply_tenant;

ALTER TABLE forum_categories
    DROP CONSTRAINT IF EXISTS fk_forum_categories_parent;
ALTER TABLE forum_category_translations
    DROP CONSTRAINT IF EXISTS fk_forum_category_translations_category;
ALTER TABLE forum_topics
    DROP CONSTRAINT IF EXISTS fk_forum_topics_category;
ALTER TABLE forum_replies
    DROP CONSTRAINT IF EXISTS fk_forum_replies_topic;
ALTER TABLE forum_replies
    DROP CONSTRAINT IF EXISTS fk_forum_replies_parent_reply;

ALTER TABLE forum_categories
    ADD CONSTRAINT fk_forum_categories_parent
    FOREIGN KEY (parent_id)
    REFERENCES forum_categories (id)
    ON UPDATE CASCADE
    ON DELETE SET NULL;

ALTER TABLE forum_category_translations
    ADD CONSTRAINT fk_forum_category_translations_category
    FOREIGN KEY (category_id)
    REFERENCES forum_categories (id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;

ALTER TABLE forum_topics
    ADD CONSTRAINT fk_forum_topics_category
    FOREIGN KEY (category_id)
    REFERENCES forum_categories (id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;

ALTER TABLE forum_replies
    ADD CONSTRAINT fk_forum_replies_topic
    FOREIGN KEY (topic_id)
    REFERENCES forum_topics (id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;

ALTER TABLE forum_replies
    ADD CONSTRAINT fk_forum_replies_parent_reply
    FOREIGN KEY (parent_reply_id)
    REFERENCES forum_replies (id)
    ON UPDATE CASCADE
    ON DELETE SET NULL;

DROP INDEX IF EXISTS
    uq_forum_category_translations_tenant_category_locale;
CREATE UNIQUE INDEX IF NOT EXISTS
    idx_forum_category_translations_category_locale
    ON forum_category_translations (category_id, locale);

ALTER TABLE forum_categories
    DROP CONSTRAINT IF EXISTS uq_forum_categories_tenant_id;
ALTER TABLE forum_topics
    DROP CONSTRAINT IF EXISTS uq_forum_topics_tenant_id;
ALTER TABLE forum_replies
    DROP CONSTRAINT IF EXISTS uq_forum_replies_tenant_id;

-- Locale widths are intentionally not reduced. Existing values may exceed
-- the former VARCHAR(16) limit after this migration has been deployed.
"#,
            )
            .await?;

        Ok(())
    }
}
