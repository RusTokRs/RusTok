use sea_orm::{ConnectionTrait, DatabaseBackend};
use sea_orm_migration::prelude::*;

const POST_CATEGORY_FK: &str = "fk_blog_posts_tenant_category";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => Ok(()),
            backend => Err(DbErr::Custom(format!(
                "Blog post category tenant-delete-action migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Intentionally irreversible: rolling back this migration would restore
        // the unsafe composite SET NULL action that can null a non-null tenant_id.
        Err(DbErr::Migration(
            "Blog post category tenant-delete-action migration is intentionally irreversible"
                .to_string(),
        ))
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    connection
        .execute_unprepared(&format!(
            "ALTER TABLE blog_posts DROP CONSTRAINT IF EXISTS {POST_CATEGORY_FK}"
        ))
        .await?;
    connection
        .execute_unprepared(&format!(
            r#"
ALTER TABLE blog_posts
    ADD CONSTRAINT {POST_CATEGORY_FK}
    FOREIGN KEY (tenant_id, category_id)
    REFERENCES blog_categories (tenant_id, id)
    ON UPDATE CASCADE
    ON DELETE SET NULL (category_id);
"#
        ))
        .await?;
    Ok(())
}
