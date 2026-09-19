use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use sea_orm_migration::prelude::*;

const POST_CATEGORY_FK: &str = "fk_blog_posts_tenant_category";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        validate_existing_relations(manager).await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "Blog post category tenant-integrity migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "Blog post category tenant-integrity migration does not support {backend:?}"
            ))),
        }
    }
}

async fn validate_existing_relations(manager: &SchemaManager) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    let row = connection
        .query_one_raw(Statement::from_string(
            connection.get_database_backend(),
            r#"
SELECT COUNT(*) AS invalid_count
FROM blog_posts post
LEFT JOIN blog_categories category
  ON category.id = post.category_id
WHERE post.category_id IS NOT NULL
  AND (
      category.id IS NULL
      OR category.tenant_id <> post.tenant_id
  )
"#
            .to_string(),
        ))
        .await?
        .ok_or_else(|| {
            DbErr::Custom("failed to validate Blog post category tenant integrity".to_string())
        })?;
    let invalid_count: i64 = row.try_get("", "invalid_count")?;
    if invalid_count != 0 {
        return Err(DbErr::Migration(format!(
            "Blog post category tenant-integrity migration blocked: {invalid_count} invalid relations exist"
        )));
    }
    Ok(())
}

async fn up_postgres(manager: &SchemaManager) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            "CREATE UNIQUE INDEX IF NOT EXISTS uq_blog_categories_tenant_id ON blog_categories (tenant_id, id)",
        )
        .await?;
    manager
        .get_connection()
        .execute_unprepared(&format!(
            r#"
ALTER TABLE blog_posts
    ADD CONSTRAINT {POST_CATEGORY_FK}
    FOREIGN KEY (tenant_id, category_id)
    REFERENCES blog_categories (tenant_id, id)
    ON UPDATE CASCADE
    ON DELETE SET NULL;
"#
        ))
        .await?;
    Ok(())
}

async fn down_postgres(manager: &SchemaManager) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            "ALTER TABLE blog_posts DROP CONSTRAINT IF EXISTS {POST_CATEGORY_FK}"
        ))
        .await?;
    manager
        .get_connection()
        .execute_unprepared("DROP INDEX IF EXISTS uq_blog_categories_tenant_id")
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        r#"CREATE TRIGGER blog_posts_category_tenant_insert
           BEFORE INSERT ON blog_posts
           FOR EACH ROW
           WHEN NEW.category_id IS NOT NULL
             AND NOT EXISTS (
                 SELECT 1
                 FROM blog_categories category
                 WHERE category.id = NEW.category_id
                   AND category.tenant_id = NEW.tenant_id
             )
           BEGIN
               SELECT RAISE(ABORT, 'blog post category tenant mismatch');
           END"#,
        r#"CREATE TRIGGER blog_posts_category_tenant_update
           BEFORE UPDATE OF tenant_id, category_id ON blog_posts
           FOR EACH ROW
           WHEN NEW.category_id IS NOT NULL
             AND NOT EXISTS (
                 SELECT 1
                 FROM blog_categories category
                 WHERE category.id = NEW.category_id
                   AND category.tenant_id = NEW.tenant_id
             )
           BEGIN
               SELECT RAISE(ABORT, 'blog post category tenant mismatch');
           END"#,
        r#"CREATE TRIGGER blog_categories_delete_null_post_category
           AFTER DELETE ON blog_categories
           FOR EACH ROW
           BEGIN
               UPDATE blog_posts
               SET category_id = NULL
               WHERE tenant_id = OLD.tenant_id
                 AND category_id = OLD.id;
           END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}

async fn down_sqlite(manager: &SchemaManager) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS blog_posts_category_tenant_insert",
        "DROP TRIGGER IF EXISTS blog_posts_category_tenant_update",
        "DROP TRIGGER IF EXISTS blog_categories_delete_null_post_category",
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
