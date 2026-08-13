use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "blog post tag tenant integrity migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "blog post tag tenant integrity migration does not support {backend:?}"
            ))),
        }
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
ALTER TABLE blog_post_tags
    ADD COLUMN IF NOT EXISTS tenant_id UUID;

UPDATE blog_post_tags relation
SET tenant_id = post.tenant_id
FROM blog_posts post
WHERE relation.post_id = post.id
  AND relation.tenant_id IS NULL;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM blog_post_tags relation
        LEFT JOIN blog_posts post
          ON post.id = relation.post_id
        LEFT JOIN taxonomy_terms term
          ON term.id = relation.tag_id
        WHERE relation.tenant_id IS NULL
           OR post.id IS NULL
           OR post.tenant_id <> relation.tenant_id
           OR term.id IS NULL
           OR term.tenant_id <> relation.tenant_id
    ) THEN
        RAISE EXCEPTION
            'blog post tag tenant integrity migration blocked: invalid legacy relation';
    END IF;
END $$;

ALTER TABLE blog_post_tags
    ALTER COLUMN tenant_id SET NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS uq_blog_posts_tenant_id
    ON blog_posts (tenant_id, id);

ALTER TABLE blog_post_tags
    DROP CONSTRAINT IF EXISTS fk_blog_post_tags_post;
ALTER TABLE blog_post_tags
    DROP CONSTRAINT IF EXISTS fk_blog_post_tags_tag;
ALTER TABLE blog_post_tags
    DROP CONSTRAINT IF EXISTS fk_blog_post_tags_post_tenant;
ALTER TABLE blog_post_tags
    DROP CONSTRAINT IF EXISTS fk_blog_post_tags_tag_tenant;

ALTER TABLE blog_post_tags
    ADD CONSTRAINT fk_blog_post_tags_post_tenant
    FOREIGN KEY (tenant_id, post_id)
    REFERENCES blog_posts (tenant_id, id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;

ALTER TABLE blog_post_tags
    ADD CONSTRAINT fk_blog_post_tags_tag_tenant
    FOREIGN KEY (tenant_id, tag_id)
    REFERENCES taxonomy_terms (tenant_id, id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;

DROP INDEX IF EXISTS idx_blog_post_tags_tag_id;
CREATE INDEX IF NOT EXISTS idx_blog_post_tags_tenant_tag_id
    ON blog_post_tags (tenant_id, tag_id, post_id);
"#,
        )
        .await?;
    Ok(())
}

async fn down_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
ALTER TABLE blog_post_tags
    DROP CONSTRAINT IF EXISTS fk_blog_post_tags_post_tenant;
ALTER TABLE blog_post_tags
    DROP CONSTRAINT IF EXISTS fk_blog_post_tags_tag_tenant;

ALTER TABLE blog_post_tags
    ADD CONSTRAINT fk_blog_post_tags_post
    FOREIGN KEY (post_id)
    REFERENCES blog_posts (id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;
ALTER TABLE blog_post_tags
    ADD CONSTRAINT fk_blog_post_tags_tag
    FOREIGN KEY (tag_id)
    REFERENCES taxonomy_terms (id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;

DROP INDEX IF EXISTS idx_blog_post_tags_tenant_tag_id;
CREATE INDEX IF NOT EXISTS idx_blog_post_tags_tag_id
    ON blog_post_tags (tag_id);

ALTER TABLE blog_post_tags DROP COLUMN tenant_id;
DROP INDEX IF EXISTS uq_blog_posts_tenant_id;
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();

    for statement in [
        "ALTER TABLE blog_post_tags ADD COLUMN tenant_id TEXT",
        "UPDATE blog_post_tags
         SET tenant_id = (
             SELECT post.tenant_id
             FROM blog_posts post
             WHERE post.id = blog_post_tags.post_id
         )
         WHERE tenant_id IS NULL",
    ] {
        connection.execute_unprepared(statement).await?;
    }

    ensure_sqlite_legacy_relations_valid(manager).await?;

    for statement in [
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_blog_posts_tenant_id
         ON blog_posts (tenant_id, id)",
        "DROP INDEX IF EXISTS idx_blog_post_tags_tag_id",
        "CREATE INDEX IF NOT EXISTS idx_blog_post_tags_tenant_tag_id
         ON blog_post_tags (tenant_id, tag_id, post_id)",
        r#"CREATE TRIGGER blog_post_tags_tenant_insert
           BEFORE INSERT ON blog_post_tags
           FOR EACH ROW
           WHEN NEW.tenant_id IS NULL
             OR NOT EXISTS (
                 SELECT 1 FROM blog_posts post
                 WHERE post.id = NEW.post_id
                   AND post.tenant_id = NEW.tenant_id
             )
             OR NOT EXISTS (
                 SELECT 1 FROM taxonomy_terms term
                 WHERE term.id = NEW.tag_id
                   AND term.tenant_id = NEW.tenant_id
             )
           BEGIN
               SELECT RAISE(ABORT, 'blog post tag tenant mismatch');
           END"#,
        r#"CREATE TRIGGER blog_post_tags_tenant_update
           BEFORE UPDATE OF tenant_id, post_id, tag_id ON blog_post_tags
           FOR EACH ROW
           WHEN NEW.tenant_id IS NULL
             OR NOT EXISTS (
                 SELECT 1 FROM blog_posts post
                 WHERE post.id = NEW.post_id
                   AND post.tenant_id = NEW.tenant_id
             )
             OR NOT EXISTS (
                 SELECT 1 FROM taxonomy_terms term
                 WHERE term.id = NEW.tag_id
                   AND term.tenant_id = NEW.tenant_id
             )
           BEGIN
               SELECT RAISE(ABORT, 'blog post tag tenant mismatch');
           END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }

    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS blog_post_tags_tenant_insert",
        "DROP TRIGGER IF EXISTS blog_post_tags_tenant_update",
        "DROP INDEX IF EXISTS idx_blog_post_tags_tenant_tag_id",
        "ALTER TABLE blog_post_tags DROP COLUMN tenant_id",
        "CREATE INDEX IF NOT EXISTS idx_blog_post_tags_tag_id ON blog_post_tags (tag_id)",
        "DROP INDEX IF EXISTS uq_blog_posts_tenant_id",
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}

async fn ensure_sqlite_legacy_relations_valid(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let row = manager
        .get_connection()
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            r#"
SELECT COUNT(*) AS invalid_count
FROM blog_post_tags relation
LEFT JOIN blog_posts post
  ON post.id = relation.post_id
LEFT JOIN taxonomy_terms term
  ON term.id = relation.tag_id
WHERE relation.tenant_id IS NULL
   OR post.id IS NULL
   OR post.tenant_id <> relation.tenant_id
   OR term.id IS NULL
   OR term.tenant_id <> relation.tenant_id
"#
            .to_string(),
        ))
        .await?
        .ok_or_else(|| {
            DbErr::Custom("failed to validate blog post tag tenant backfill".to_string())
        })?;
    let invalid_count: i64 = row.try_get("", "invalid_count")?;
    if invalid_count != 0 {
        return Err(DbErr::Custom(
            "blog post tag tenant integrity migration blocked: invalid legacy relation".to_string(),
        ));
    }
    Ok(())
}
