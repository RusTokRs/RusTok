use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
CREATE TABLE IF NOT EXISTS blog_tag_usage (
    tenant_id UUID NOT NULL,
    tag_id UUID NOT NULL,
    canonical_key VARCHAR(120) NOT NULL,
    use_count INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (tenant_id, tag_id),
    CONSTRAINT ck_blog_tag_usage_non_negative CHECK (use_count >= 0),
    CONSTRAINT fk_blog_tag_usage_taxonomy_term_tenant
        FOREIGN KEY (tenant_id, tag_id)
        REFERENCES taxonomy_terms (tenant_id, id)
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
CREATE INDEX IF NOT EXISTS idx_blog_tag_usage_list
    ON blog_tag_usage (tenant_id, use_count DESC, canonical_key ASC, tag_id ASC);
"#,
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
INSERT INTO blog_tag_usage (tenant_id, tag_id, canonical_key, use_count)
SELECT
    term.tenant_id,
    term.id,
    term.canonical_key,
    CAST(COALESCE(usage.use_count, 0) AS INTEGER)
FROM taxonomy_terms term
LEFT JOIN (
    SELECT tenant_id, tag_id, COUNT(*) AS use_count
    FROM blog_post_tags
    GROUP BY tenant_id, tag_id
) usage
    ON usage.tenant_id = term.tenant_id
   AND usage.tag_id = term.id
WHERE term.kind = 'tag'
  AND (
      (term.scope_type = 'module' AND term.scope_value = 'blog')
      OR (
          term.scope_type = 'global'
          AND usage.tag_id IS NOT NULL
      )
  );
"#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS blog_tag_usage;")
            .await?;
        Ok(())
    }
}
