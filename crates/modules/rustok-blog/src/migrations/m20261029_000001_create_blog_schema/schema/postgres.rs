//! PostgreSQL-specific constraints and append-only audit guards.

use super::*;
use crate::domain::BLOG_CATEGORY_SETTINGS_MAX_BYTES;
use sea_orm::ConnectionTrait;
use sea_orm_migration::prelude::*;

pub(super) async fn up(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();

    // 1. Column integrity constraints on blog_posts
    for (constraint, predicate) in [
        (
            "ck_blog_posts_status_vocabulary",
            "status IN ('draft', 'published', 'archived')",
        ),
        ("ck_blog_posts_version_positive", "version > 0"),
        ("ck_blog_posts_comment_count_non_negative", "comment_count >= 0"),
        (
            "ck_blog_posts_pin_state_coherent",
            "(is_pinned AND pinned_at IS NOT NULL) OR (NOT is_pinned AND pinned_at IS NULL)",
        ),
        (
            "ck_blog_posts_published_requires_published_at",
            "status <> 'published' OR published_at IS NOT NULL",
        ),
        (
            "ck_blog_posts_schedule_is_draft_only",
            "status = 'draft' OR scheduled_at IS NULL",
        ),
    ] {
        connection
            .execute_unprepared(&format!(
                "ALTER TABLE blog_posts ADD CONSTRAINT {constraint} CHECK ({predicate});"
            ))
            .await?;
    }

    // 2. Settings JSON contract constraints on blog_categories
    connection
        .execute_unprepared(&format!(
            r#"
ALTER TABLE blog_categories
    ADD CONSTRAINT ck_blog_categories_settings_object
    CHECK (jsonb_typeof(settings) = 'object');

ALTER TABLE blog_categories
    ADD CONSTRAINT ck_blog_categories_settings_size
    CHECK (octet_length(settings::text) <= {BLOG_CATEGORY_SETTINGS_MAX_BYTES});
"#
        ))
        .await?;

    // 3. Category tenant-integrity FK with ON DELETE SET NULL (category_id)
    connection
        .execute_unprepared(
            r#"
ALTER TABLE blog_posts
    ADD CONSTRAINT fk_blog_posts_tenant_category
    FOREIGN KEY (tenant_id, category_id)
    REFERENCES blog_categories (tenant_id, id)
    ON UPDATE CASCADE
    ON DELETE SET NULL (category_id);
"#,
        )
        .await?;

    // 4. Channel visibility composite tenant foreign key
    connection
        .execute_unprepared(
            r#"
ALTER TABLE blog_post_channel_visibility
    ADD CONSTRAINT fk_blog_post_channel_visibility_tenant_post
    FOREIGN KEY (tenant_id, post_id)
    REFERENCES blog_posts (tenant_id, id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;
"#,
        )
        .await?;

    // 5. Post-to-post composite tenant foreign key; Taxonomy FKs live in CREATE TABLE
    connection
        .execute_unprepared(
            r#"
ALTER TABLE blog_post_tags
    ADD CONSTRAINT fk_blog_post_tags_tenant_post
    FOREIGN KEY (tenant_id, post_id)
    REFERENCES blog_posts (tenant_id, id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;

"#,
        )
        .await?;

    // 6. Preview tokens composite tenant foreign key
    connection
        .execute_unprepared(
            r#"
ALTER TABLE blog_preview_tokens
    ADD CONSTRAINT fk_blog_preview_tokens_tenant_post
    FOREIGN KEY (tenant_id, post_id)
    REFERENCES blog_posts (tenant_id, id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;
"#,
        )
        .await?;

    // 7. Comment projection deliveries composite tenant foreign key
    connection
        .execute_unprepared(
            r#"
ALTER TABLE blog_comment_projection_deliveries
    ADD CONSTRAINT fk_blog_comment_projection_deliveries_tenant_post
    FOREIGN KEY (tenant_id, post_id)
    REFERENCES blog_posts (tenant_id, id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;
"#,
        )
        .await?;

    // 8. Comments schedule audit recovery facts immutability triggers (Postgres)
    connection
        .execute_unprepared(
            r#"
CREATE OR REPLACE FUNCTION blog_comments_delegation_audit_recovery_reject_mutation()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'Comments schedule audit recovery facts are append-only';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER blog_comments_delegation_audit_recovery_immutable_update
BEFORE UPDATE ON blog_comments_tcp_delegation_schedule_audit_recovery_audits
FOR EACH ROW EXECUTE FUNCTION blog_comments_delegation_audit_recovery_reject_mutation();

CREATE TRIGGER blog_comments_delegation_audit_recovery_immutable_delete
BEFORE DELETE ON blog_comments_tcp_delegation_schedule_audit_recovery_audits
FOR EACH ROW EXECUTE FUNCTION blog_comments_delegation_audit_recovery_reject_mutation();
"#,
        )
        .await?;

    Ok(())
}
