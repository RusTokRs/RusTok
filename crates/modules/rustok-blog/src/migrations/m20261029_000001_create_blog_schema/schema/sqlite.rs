//! SQLite-specific integrity triggers for constraints that cannot be added with ALTER TABLE.

use super::*;
use crate::domain::BLOG_CATEGORY_SETTINGS_MAX_BYTES;
use sea_orm::ConnectionTrait;

pub(super) async fn up(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();

    // 1. Column integrity triggers on blog_posts
    const SQLITE_INVALID_ROW_PREDICATE: &str = "
        NEW.status NOT IN ('draft', 'published', 'archived')
        OR NEW.version <= 0
        OR NEW.comment_count < 0
        OR (NEW.is_pinned AND NEW.pinned_at IS NULL)
        OR (NOT NEW.is_pinned AND NEW.pinned_at IS NOT NULL)
        OR (NEW.status = 'published' AND NEW.published_at IS NULL)
        OR (NEW.status <> 'draft' AND NEW.scheduled_at IS NOT NULL)
    ";

    for (trigger, timing) in [
        ("blog_posts_column_integrity_insert", "BEFORE INSERT"),
        ("blog_posts_column_integrity_update", "BEFORE UPDATE"),
    ] {
        connection
            .execute_unprepared(&format!(
                r#"
CREATE TRIGGER {trigger}
{timing} ON blog_posts
FOR EACH ROW
WHEN {SQLITE_INVALID_ROW_PREDICATE}
BEGIN
    SELECT RAISE(ABORT, 'blog_posts column integrity violated');
END;
"#
            ))
            .await?;
    }

    // 2. Settings contract triggers on blog_categories
    for (trigger, timing) in [
        ("blog_categories_settings_contract_insert", "BEFORE INSERT"),
        (
            "blog_categories_settings_contract_update",
            "BEFORE UPDATE OF settings",
        ),
    ] {
        connection
            .execute_unprepared(&format!(
                r#"
CREATE TRIGGER {trigger}
{timing} ON blog_categories
FOR EACH ROW
WHEN json_valid(NEW.settings) = 0
  OR json_type(NEW.settings) <> 'object'
  OR length(CAST(json(NEW.settings) AS BLOB)) > {BLOG_CATEGORY_SETTINGS_MAX_BYTES}
BEGIN
    SELECT RAISE(ABORT, 'blog_categories settings contract violated');
END;
"#
            ))
            .await?;
    }

    // 3. Category tenant integrity triggers on blog_posts & blog_categories
    connection
        .execute_unprepared(
            r#"
CREATE TRIGGER blog_posts_category_tenant_insert
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
END;

CREATE TRIGGER blog_posts_category_tenant_update
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
END;

CREATE TRIGGER blog_categories_delete_null_post_category
AFTER DELETE ON blog_categories
FOR EACH ROW
BEGIN
    UPDATE blog_posts
    SET category_id = NULL
    WHERE tenant_id = OLD.tenant_id
      AND category_id = OLD.id;
END;
"#,
        )
        .await?;

    // 4. Channel visibility tenant integrity triggers
    connection
        .execute_unprepared(
            r#"
CREATE TRIGGER blog_post_channel_visibility_tenant_insert
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
END;

CREATE TRIGGER blog_post_channel_visibility_tenant_update
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
END;
"#,
        )
        .await?;

    // 5. Post tags tenant integrity triggers
    connection
        .execute_unprepared(
            r#"
CREATE TRIGGER blog_post_tags_tenant_insert
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
END;

CREATE TRIGGER blog_post_tags_tenant_update
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
END;
"#,
        )
        .await?;

    // 6. Preview tokens tenant integrity triggers
    connection
        .execute_unprepared(
            r#"
CREATE TRIGGER blog_preview_tokens_tenant_insert
BEFORE INSERT ON blog_preview_tokens
FOR EACH ROW
WHEN NOT EXISTS (
    SELECT 1
    FROM blog_posts post
    WHERE post.id = NEW.post_id
      AND post.tenant_id = NEW.tenant_id
)
BEGIN
    SELECT RAISE(ABORT, 'blog preview token references post of another tenant');
END;

CREATE TRIGGER blog_preview_tokens_tenant_update
BEFORE UPDATE OF tenant_id, post_id ON blog_preview_tokens
FOR EACH ROW
WHEN NOT EXISTS (
    SELECT 1
    FROM blog_posts post
    WHERE post.id = NEW.post_id
      AND post.tenant_id = NEW.tenant_id
)
BEGIN
    SELECT RAISE(ABORT, 'blog preview token references post of another tenant');
END;
"#,
        )
        .await?;

    // 7. Comment projection deliveries tenant integrity triggers
    connection
        .execute_unprepared(
            r#"
CREATE TRIGGER blog_comment_projection_deliveries_tenant_insert
BEFORE INSERT ON blog_comment_projection_deliveries
FOR EACH ROW
WHEN NOT EXISTS (
    SELECT 1
    FROM blog_posts post
    WHERE post.id = NEW.post_id
      AND post.tenant_id = NEW.tenant_id
)
BEGIN
    SELECT RAISE(ABORT, 'blog comment projection delivery references post of another tenant');
END;

CREATE TRIGGER blog_comment_projection_deliveries_tenant_update
BEFORE UPDATE OF tenant_id, post_id ON blog_comment_projection_deliveries
FOR EACH ROW
WHEN NOT EXISTS (
    SELECT 1
    FROM blog_posts post
    WHERE post.id = NEW.post_id
      AND post.tenant_id = NEW.tenant_id
)
BEGIN
    SELECT RAISE(ABORT, 'blog comment projection delivery references post of another tenant');
END;

CREATE TRIGGER blog_posts_delete_comment_projection_deliveries
AFTER DELETE ON blog_posts
FOR EACH ROW
BEGIN
    DELETE FROM blog_comment_projection_deliveries
    WHERE tenant_id = OLD.tenant_id
      AND post_id = OLD.id;
END;
"#,
        )
        .await?;

    // 8. Comments schedule audit recovery facts immutability triggers (SQLite)
    connection
        .execute_unprepared(
            r#"
CREATE TRIGGER blog_comments_delegation_audit_recovery_immutable_update
BEFORE UPDATE ON blog_comments_tcp_delegation_schedule_audit_recovery_audits
FOR EACH ROW
BEGIN
    SELECT RAISE(ABORT, 'Comments schedule audit recovery facts are append-only');
END;

CREATE TRIGGER blog_comments_delegation_audit_recovery_immutable_delete
BEFORE DELETE ON blog_comments_tcp_delegation_schedule_audit_recovery_audits
FOR EACH ROW
BEGIN
    SELECT RAISE(ABORT, 'Comments schedule audit recovery facts are append-only');
END;
"#,
        )
        .await?;

    Ok(())
}
