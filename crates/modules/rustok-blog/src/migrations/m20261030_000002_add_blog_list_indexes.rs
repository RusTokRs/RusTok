//! Keyset indexes that the Blog read paths are documented and written to use.
//!
//! Before this migration `blog_posts` had tenant-scoped indexes for slug,
//! (status, published_at), pinning and scheduling only. The admin list's
//! default `created_at DESC` sort and its `updated_at`/`published_at` sorts
//! therefore degraded to a scan plus sort on every page, and the module
//! README's claim that each sort column has its own index was not true. The
//! public list ordering (`order_public_post_list` in
//! `src/services/post/queries.rs`) also referenced an
//! `idx_blog_posts_public_keyset` index that did not exist; this migration
//! creates an index under exactly that name so the code comment and the README
//! become true instead of needing to be corrected.
//!
//! Index shapes:
//! - The public keyset index is partial over the public corpus
//!   (`status = 'published' AND published_at IS NOT NULL`, the same predicate
//!   every public read enforces) and orders the remaining keys exactly like the
//!   query: `is_pinned DESC, pinned_at DESC NULLS LAST, published_at DESC,
//!   id DESC` on PostgreSQL; SQLite has no per-column NULL placement in index
//!   definitions, so it gets the same columns without `NULLS LAST` (the query's
//!   explicit ordering still guarantees correctness).
//! - `created_at`/`updated_at` are `NOT NULL`, so one ascending composite per
//!   column serves both keyset directions: PostgreSQL scans it backwards for
//!   `DESC` with the matching `id` tiebreaker.
//! - `published_at` is nullable and the admin keyset keeps NULLs last in both
//!   directions, so PostgreSQL gets one index per direction with explicit
//!   NULL placement; SQLite gets one plain composite.
//!
//! The migration also drops `idx_blog_preview_tokens_token`: the
//! `blog_preview_tokens.token` column already carries a `UNIQUE` constraint
//! whose implicit index serves the digest lookup, so the extra non-unique
//! index only added write cost.
//!
//! Finally it adds `(tenant_id, processed_at)` on
//! `blog_comment_projection_deliveries`, which the projection-retention worker
//! (`blog_comment_projection_retention`) scans per tenant; without it each
//! retention iteration would seq-scan the per-tenant ledger.

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let is_sqlite = manager.get_database_backend() == DatabaseBackend::Sqlite;
        let index_sql = if is_sqlite {
            r#"
CREATE INDEX IF NOT EXISTS idx_blog_posts_public_keyset
    ON blog_posts (tenant_id, is_pinned DESC, pinned_at DESC, published_at DESC, id DESC)
    WHERE status = 'published' AND published_at IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_blog_posts_tenant_created_id
    ON blog_posts (tenant_id, created_at, id);

CREATE INDEX IF NOT EXISTS idx_blog_posts_tenant_updated_id
    ON blog_posts (tenant_id, updated_at, id);

CREATE INDEX IF NOT EXISTS idx_blog_posts_tenant_published_id
    ON blog_posts (tenant_id, published_at, id);

CREATE INDEX IF NOT EXISTS idx_blog_comment_projection_deliveries_tenant_processed
    ON blog_comment_projection_deliveries (tenant_id, processed_at);

DROP INDEX IF EXISTS idx_blog_preview_tokens_token;
"#
        } else {
            r#"
CREATE INDEX IF NOT EXISTS idx_blog_posts_public_keyset
    ON blog_posts (tenant_id, is_pinned DESC, pinned_at DESC NULLS LAST, published_at DESC, id DESC)
    WHERE status = 'published' AND published_at IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_blog_posts_tenant_created_id
    ON blog_posts (tenant_id, created_at, id);

CREATE INDEX IF NOT EXISTS idx_blog_posts_tenant_updated_id
    ON blog_posts (tenant_id, updated_at, id);

CREATE INDEX IF NOT EXISTS idx_blog_posts_tenant_published_desc
    ON blog_posts (tenant_id, published_at DESC NULLS LAST, id DESC);

CREATE INDEX IF NOT EXISTS idx_blog_posts_tenant_published_asc
    ON blog_posts (tenant_id, published_at ASC NULLS LAST, id ASC);

CREATE INDEX IF NOT EXISTS idx_blog_comment_projection_deliveries_tenant_processed
    ON blog_comment_projection_deliveries (tenant_id, processed_at);

DROP INDEX IF EXISTS idx_blog_preview_tokens_token;
"#
        };
        manager
            .get_connection()
            .execute_unprepared(index_sql)
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let is_sqlite = manager.get_database_backend() == DatabaseBackend::Sqlite;
        let drop_sql = if is_sqlite {
            r#"
DROP INDEX IF EXISTS idx_blog_posts_tenant_published_id;
DROP INDEX IF EXISTS idx_blog_posts_tenant_updated_id;
DROP INDEX IF EXISTS idx_blog_posts_tenant_created_id;
DROP INDEX IF EXISTS idx_blog_posts_public_keyset;
DROP INDEX IF EXISTS idx_blog_comment_projection_deliveries_tenant_processed;

CREATE INDEX IF NOT EXISTS idx_blog_preview_tokens_token
    ON blog_preview_tokens (token);
"#
        } else {
            r#"
DROP INDEX IF EXISTS idx_blog_posts_tenant_published_asc;
DROP INDEX IF EXISTS idx_blog_posts_tenant_published_desc;
DROP INDEX IF EXISTS idx_blog_posts_tenant_updated_id;
DROP INDEX IF EXISTS idx_blog_posts_tenant_created_id;
DROP INDEX IF EXISTS idx_blog_posts_public_keyset;
DROP INDEX IF EXISTS idx_blog_comment_projection_deliveries_tenant_processed;

CREATE INDEX IF NOT EXISTS idx_blog_preview_tokens_token
    ON blog_preview_tokens (token);
"#
        };
        manager
            .get_connection()
            .execute_unprepared(drop_sql)
            .await?;
        Ok(())
    }
}
