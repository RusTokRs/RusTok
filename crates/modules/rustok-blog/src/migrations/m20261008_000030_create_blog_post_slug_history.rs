use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Blog post slugs are global canonical identifiers. Retired slugs are
        // stored here so that old public URLs can be permanently redirected to
        // the current canonical slug. The table is owned by Blog and is a
        // derived lookup aid: the authoritative slug stays in blog_posts.slug.
        manager
            .get_connection()
            .execute_unprepared(
                r#"
CREATE TABLE IF NOT EXISTS blog_post_slug_history (
    tenant_id UUID NOT NULL,
    slug VARCHAR(255) NOT NULL,
    post_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (tenant_id, slug),
    CONSTRAINT fk_blog_post_slug_history_post_tenant
        FOREIGN KEY (tenant_id, post_id)
        REFERENCES blog_posts (tenant_id, id)
        ON UPDATE CASCADE
        ON DELETE CASCADE
);
"#,
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
CREATE INDEX IF NOT EXISTS idx_blog_post_slug_history_post
    ON blog_post_slug_history (tenant_id, post_id);
"#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS blog_post_slug_history;")
            .await?;
        Ok(())
    }
}
