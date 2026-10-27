use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use sea_orm_migration::prelude::*;

/// Column-level integrity of `blog_posts` (AGENTS.md §7.1): the status
/// vocabulary, the positive business revision, the non-negative derived
/// comment counter, pin-state coherence and the draft-only schedule are
/// physical column validity, so PostgreSQL enforces them with CHECK
/// constraints and SQLite (the unit-test backend, which cannot add CHECK
/// constraints to an existing table) enforces the same predicates with
/// insert/update triggers, following the Blog Category settings contract
/// migration (`m20260924_000029`).
const STATUS_CONSTRAINT: &str = "ck_blog_posts_status_vocabulary";
const VERSION_CONSTRAINT: &str = "ck_blog_posts_version_positive";
const COMMENT_COUNT_CONSTRAINT: &str = "ck_blog_posts_comment_count_non_negative";
const PIN_CONSTRAINT: &str = "ck_blog_posts_pin_state_coherent";
const PUBLISHED_AT_CONSTRAINT: &str = "ck_blog_posts_published_requires_published_at";
const SCHEDULE_CONSTRAINT: &str = "ck_blog_posts_schedule_is_draft_only";
const INSERT_TRIGGER: &str = "blog_posts_column_integrity_insert";
const UPDATE_TRIGGER: &str = "blog_posts_column_integrity_update";

/// The same predicate for SQLite triggers, where the row under write is
/// addressed through `NEW`.
const SQLITE_INVALID_ROW_PREDICATE: &str = "
    NEW.status NOT IN ('draft', 'published', 'archived')
    OR NEW.version <= 0
    OR NEW.comment_count < 0
    OR (NEW.is_pinned AND NEW.pinned_at IS NULL)
    OR (NOT NEW.is_pinned AND NEW.pinned_at IS NOT NULL)
    OR (NEW.status = 'published' AND NEW.published_at IS NULL)
    OR (NEW.status <> 'draft' AND NEW.scheduled_at IS NOT NULL)
";

/// One predicate, expressed once, shared by the pre-flight validation query,
/// the PostgreSQL CHECK constraints and the SQLite triggers. `is_pinned`
/// renders as a boolean in PostgreSQL and as 0/1 in SQLite; the comparison
/// against the `pinned_at` presence is valid in both.
const INVALID_ROW_PREDICATE: &str = "
    status NOT IN ('draft', 'published', 'archived')
    OR version <= 0
    OR comment_count < 0
    OR (is_pinned AND pinned_at IS NULL)
    OR (NOT is_pinned AND pinned_at IS NOT NULL)
    OR (status = 'published' AND published_at IS NULL)
    OR (status <> 'draft' AND scheduled_at IS NOT NULL)
";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        validate_existing_posts(manager).await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "Blog post column integrity migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "Blog post column integrity migration does not support {backend:?}"
            ))),
        }
    }
}

async fn validate_existing_posts(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    let row = connection
        .query_one_raw(Statement::from_string(
            connection.get_database_backend(),
            format!(
                "SELECT COUNT(*) AS invalid_count FROM blog_posts WHERE {INVALID_ROW_PREDICATE}"
            ),
        ))
        .await?
        .ok_or_else(|| {
            DbErr::Custom("failed to validate Blog post column integrity".to_string())
        })?;
    let invalid_count: i64 = row.try_get("", "invalid_count")?;

    if invalid_count != 0 {
        return Err(DbErr::Migration(format!(
            "Blog post column integrity migration blocked: {invalid_count} invalid row(s) exist; \
             statuses must be draft/published/archived, version must be positive, comment_count \
             must be non-negative, pin state must be coherent, published posts must carry \
             published_at, and only drafts may carry scheduled_at"
        )));
    }

    Ok(())
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for (constraint, predicate) in [
        (
            STATUS_CONSTRAINT,
            "status IN ('draft', 'published', 'archived')",
        ),
        (VERSION_CONSTRAINT, "version > 0"),
        (COMMENT_COUNT_CONSTRAINT, "comment_count >= 0"),
        (
            PIN_CONSTRAINT,
            "(is_pinned AND pinned_at IS NOT NULL) OR (NOT is_pinned AND pinned_at IS NULL)",
        ),
        (
            PUBLISHED_AT_CONSTRAINT,
            "status <> 'published' OR published_at IS NOT NULL",
        ),
        (
            SCHEDULE_CONSTRAINT,
            "status = 'draft' OR scheduled_at IS NULL",
        ),
    ] {
        connection
            .execute_unprepared(&format!(
                "ALTER TABLE blog_posts ADD CONSTRAINT {constraint} CHECK ({predicate});"
            ))
            .await?;
    }

    Ok(())
}

async fn down_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for constraint in [
        SCHEDULE_CONSTRAINT,
        PUBLISHED_AT_CONSTRAINT,
        PIN_CONSTRAINT,
        COMMENT_COUNT_CONSTRAINT,
        VERSION_CONSTRAINT,
        STATUS_CONSTRAINT,
    ] {
        connection
            .execute_unprepared(&format!(
                "ALTER TABLE blog_posts DROP CONSTRAINT IF EXISTS {constraint}"
            ))
            .await?;
    }

    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for (trigger, timing) in [
        (INSERT_TRIGGER, "BEFORE INSERT"),
        (UPDATE_TRIGGER, "BEFORE UPDATE"),
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

    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    connection
        .execute_unprepared(&format!("DROP TRIGGER IF EXISTS {UPDATE_TRIGGER}"))
        .await?;
    connection
        .execute_unprepared(&format!("DROP TRIGGER IF EXISTS {INSERT_TRIGGER}"))
        .await?;

    Ok(())
}
