//! Enforces the integrity of the Blog comment projection delivery table.
//!
//! The table stores one row per processed comment event. It referenced the Blog
//! post only by an unconstrained column, so a crash between the post delete and
//! the next comment event could leave orphaned delivery rows behind, and the
//! per-comment "latest delivery" lookup had no supporting index on
//! `comment_id`.
//!
//! The migration
//!
//! 1. removes the delivery rows whose post no longer exists (derived state
//!    without a subject, so it carries no information),
//! 2. blocks when a delivery row points at a post of a different tenant (a
//!    genuine cross-tenant integrity violation that must be repaired by hand),
//! 3. adds the tenant-scoped foreign key to `blog_posts` with
//!    `ON DELETE CASCADE`, so a post delete can no longer leave orphans, and
//! 4. adds the supporting index on `comment_id`.
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use sea_orm_migration::prelude::*;

const DELIVERY_POST_FK: &str = "fk_blog_comment_projection_deliveries_tenant_post";
const COMMENT_INDEX: &str = "idx_blog_comment_projection_deliveries_comment";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        remove_orphan_deliveries(manager).await?;
        validate_tenant_relations(manager).await?;
        create_comment_index(manager).await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "Blog comment projection delivery integrity migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await?,
            DatabaseBackend::Sqlite => down_sqlite(manager).await?,
            backend => {
                return Err(DbErr::Custom(format!(
                    "Blog comment projection delivery integrity migration does not support {backend:?}"
                )));
            }
        };
        drop_comment_index(manager).await
    }
}

/// Removes the delivery rows whose Blog post no longer exists.
///
/// The row is derived projection state of a post that is gone: it has no
/// subject left and can never be read again, so it is dropped instead of
/// blocking the foreign key.
async fn remove_orphan_deliveries(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    let result = connection
        .execute_unprepared(
            r#"
DELETE FROM blog_comment_projection_deliveries
WHERE NOT EXISTS (
    SELECT 1
    FROM blog_posts post
    WHERE post.id = blog_comment_projection_deliveries.post_id
)
"#,
        )
        .await?;
    if result.rows_affected() > 0 {
        tracing::info!(
            removed = result.rows_affected(),
            "removed Blog comment projection deliveries without a Blog post",
        );
    }
    Ok(())
}

/// Blocks the migration when a delivery row points at a post of another tenant.
async fn validate_tenant_relations(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    let row = connection
        .query_one_raw(Statement::from_string(
            connection.get_database_backend(),
            r#"
SELECT COUNT(*) AS invalid_count
FROM blog_comment_projection_deliveries delivery
JOIN blog_posts post
  ON post.id = delivery.post_id
WHERE post.tenant_id <> delivery.tenant_id
"#
            .to_string(),
        ))
        .await?
        .ok_or_else(|| {
            DbErr::Custom(
                "failed to validate Blog comment projection delivery tenant integrity".to_string(),
            )
        })?;
    let invalid_count: i64 = row.try_get("", "invalid_count")?;
    if invalid_count != 0 {
        return Err(DbErr::Migration(format!(
            "Blog comment projection delivery tenant-integrity migration blocked: {invalid_count} invalid relations exist"
        )));
    }
    Ok(())
}

async fn create_comment_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .name(COMMENT_INDEX)
                .table(BlogCommentProjectionDeliveries::Table)
                .col(BlogCommentProjectionDeliveries::TenantId)
                .col(BlogCommentProjectionDeliveries::CommentId)
                .if_not_exists()
                .to_owned(),
        )
        .await
}

async fn drop_comment_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name(COMMENT_INDEX)
                .table(BlogCommentProjectionDeliveries::Table)
                .if_exists()
                .to_owned(),
        )
        .await
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            r#"
ALTER TABLE blog_comment_projection_deliveries
    ADD CONSTRAINT {DELIVERY_POST_FK}
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
    manager
        .get_connection()
        .execute_unprepared(&format!(
            "ALTER TABLE blog_comment_projection_deliveries DROP CONSTRAINT IF EXISTS {DELIVERY_POST_FK}"
        ))
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        r#"CREATE TRIGGER blog_comment_projection_deliveries_tenant_insert
           BEFORE INSERT ON blog_comment_projection_deliveries
           FOR EACH ROW
           WHEN NOT EXISTS (
               SELECT 1
               FROM blog_posts post
               WHERE post.id = NEW.post_id
                 AND post.tenant_id = NEW.tenant_id
           )
           BEGIN
               SELECT RAISE(ABORT, 'blog comment projection delivery tenant mismatch');
           END"#,
        r#"CREATE TRIGGER blog_comment_projection_deliveries_tenant_update
           BEFORE UPDATE OF tenant_id, post_id ON blog_comment_projection_deliveries
           FOR EACH ROW
           WHEN NOT EXISTS (
               SELECT 1
               FROM blog_posts post
               WHERE post.id = NEW.post_id
                 AND post.tenant_id = NEW.tenant_id
           )
           BEGIN
               SELECT RAISE(ABORT, 'blog comment projection delivery tenant mismatch');
           END"#,
        r#"CREATE TRIGGER blog_posts_delete_comment_projection_deliveries
           AFTER DELETE ON blog_posts
           FOR EACH ROW
           BEGIN
               DELETE FROM blog_comment_projection_deliveries
               WHERE tenant_id = OLD.tenant_id
                 AND post_id = OLD.id;
           END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS blog_comment_projection_deliveries_tenant_insert",
        "DROP TRIGGER IF EXISTS blog_comment_projection_deliveries_tenant_update",
        "DROP TRIGGER IF EXISTS blog_posts_delete_comment_projection_deliveries",
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}

#[derive(DeriveIden)]
enum BlogCommentProjectionDeliveries {
    Table,
    TenantId,
    CommentId,
}
