use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use sea_orm_migration::prelude::*;

const CHANNEL_VISIBILITY_FK: &str = "fk_blog_post_channel_visibility_tenant_post";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        validate_existing_rows(manager).await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "Blog channel-visibility tenant-integrity migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "Blog channel-visibility tenant-integrity migration does not support {backend:?}"
            ))),
        }
    }
}

async fn validate_existing_rows(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    let row = connection
        .query_one_raw(Statement::from_string(
            connection.get_database_backend(),
            r#"
SELECT COUNT(*) AS invalid_count
FROM blog_post_channel_visibility visibility
LEFT JOIN blog_posts post
  ON post.id = visibility.post_id
WHERE post.id IS NULL
   OR post.tenant_id <> visibility.tenant_id
"#
            .to_string(),
        ))
        .await?
        .ok_or_else(|| {
            DbErr::Custom(
                "failed to validate Blog channel-visibility tenant integrity".to_string(),
            )
        })?;
    let invalid_count: i64 = row.try_get("", "invalid_count")?;
    if invalid_count != 0 {
        return Err(DbErr::Migration(format!(
            "Blog channel-visibility tenant-integrity migration blocked: {invalid_count} invalid relations exist"
        )));
    }
    Ok(())
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    // PostgreSQL requires the referenced column set of a composite foreign key
    // to have its own unique constraint/index, even though id is already a
    // primary key. The composite key is therefore explicit and tenant-shaped.
    connection
        .execute_unprepared(
            "CREATE UNIQUE INDEX IF NOT EXISTS uq_blog_posts_tenant_id ON blog_posts (tenant_id, id)",
        )
        .await?;
    connection
        .execute_unprepared(
            "ALTER TABLE blog_post_channel_visibility DROP CONSTRAINT IF EXISTS fk_blog_post_channel_visibility_post",
        )
        .await?;
    connection
        .execute_unprepared(&format!(
            r#"
ALTER TABLE blog_post_channel_visibility
    ADD CONSTRAINT {CHANNEL_VISIBILITY_FK}
    FOREIGN KEY (tenant_id, post_id)
    REFERENCES blog_posts (tenant_id, id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;
"#
        ))
        .await?;
    Ok(())
}

async fn down_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    connection
        .execute_unprepared(&format!(
            "ALTER TABLE blog_post_channel_visibility DROP CONSTRAINT IF EXISTS {CHANNEL_VISIBILITY_FK}"
        ))
        .await?;
    connection
        .execute_unprepared(
            r#"
ALTER TABLE blog_post_channel_visibility
    ADD CONSTRAINT fk_blog_post_channel_visibility_post
    FOREIGN KEY (post_id)
    REFERENCES blog_posts (id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;
"#,
        )
        .await?;
    connection
        .execute_unprepared("DROP INDEX IF EXISTS uq_blog_posts_tenant_id")
        .await?;
    Ok(())
}
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        r#"CREATE TRIGGER blog_post_channel_visibility_tenant_insert
           BEFORE INSERT ON blog_post_channel_visibility
           FOR EACH ROW
           WHEN NOT EXISTS (
               SELECT 1
               FROM blog_posts post
               WHERE post.id = NEW.post_id
                 AND post.tenant_id = NEW.tenant_id
           )
           BEGIN
               SELECT RAISE(ABORT, 'blog channel visibility tenant mismatch');
           END"#,
        r#"CREATE TRIGGER blog_post_channel_visibility_tenant_update
           BEFORE UPDATE OF tenant_id, post_id ON blog_post_channel_visibility
           FOR EACH ROW
           WHEN NOT EXISTS (
               SELECT 1
               FROM blog_posts post
               WHERE post.id = NEW.post_id
                 AND post.tenant_id = NEW.tenant_id
           )
           BEGIN
               SELECT RAISE(ABORT, 'blog channel visibility tenant mismatch');
           END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS blog_post_channel_visibility_tenant_insert",
        "DROP TRIGGER IF EXISTS blog_post_channel_visibility_tenant_update",
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
