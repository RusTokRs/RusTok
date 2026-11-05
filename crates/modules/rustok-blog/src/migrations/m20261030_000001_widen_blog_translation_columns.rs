//! Align `blog_post_translations` column widths with the Blog owner contract.
//!
//! Three columns were created narrower than the boundary accepts:
//! - `title` was `varchar(255)` while the owner validator and the transport
//!   schemas cap a post title at 512 characters (see `MAX_POST_SEO_*` and
//!   `validate_title` in `src/services/post/mod.rs`, mirrored by
//!   `#[schema(max_length = …)]` in `src/dto/post.rs`);
//! - `seo_description` was `varchar(500)` while the contract is 1000;
//! - `locale` was `varchar(16)` while the platform stores normalized,
//!   extension-free BCP-47 identities up to 32 characters
//!   (`rustok_api::MAX_LOCALE_TAG_LEN` — the taxonomy module widened its own
//!   locale columns to the same width in
//!   `m20260721_000006_expand_taxonomy_locale_storage_columns`; Blog was the
//!   remaining 16-wide locale owner).
//!
//! So on PostgreSQL a value that passed owner validation was rejected by
//! storage with an unclassifiable error and surfaced to the caller as a 500
//! instead of a 400. SQLite never enforced the width (TEXT affinity), which is
//! also why the mismatch escaped backend-parity checks.
//!
//! Widening preserves existing data. A later title/description downgrade may
//! refuse when new values exceed the old bounds. Locale storage retains the
//! platform's 32-character contract on rollback. SQLite needs no width change.

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }
        manager
            .get_connection()
            .execute_unprepared(
                r#"
ALTER TABLE blog_post_translations
    ALTER COLUMN title TYPE varchar(512);

ALTER TABLE blog_post_translations
    ALTER COLUMN seo_description TYPE varchar(1000);

ALTER TABLE blog_post_translations
    ALTER COLUMN locale TYPE varchar(32);
"#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Title/description narrowing refuses values above the old bounds.
        // Locale widening is retained even on rollback, per the platform
        // storage contract, so valid longer locale identities remain storable.
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }
        manager
            .get_connection()
            .execute_unprepared(
                r#"
ALTER TABLE blog_post_translations
    ALTER COLUMN title TYPE varchar(255);

ALTER TABLE blog_post_translations
    ALTER COLUMN seo_description TYPE varchar(500);

"#,
            )
            .await?;
        Ok(())
    }
}
