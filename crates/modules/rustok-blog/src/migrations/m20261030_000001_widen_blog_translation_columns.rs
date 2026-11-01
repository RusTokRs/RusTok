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
//! Widening is metadata-only on PostgreSQL and lossless in both directions for
//! existing data: PostgreSQL could never have stored more than the old bound.
//! SQLite needs no change and intentionally gets none.

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
        // Narrowing refuses to run while values above the old bound exist;
        // that failure is the correct signal that the downgrade would lose data.
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

ALTER TABLE blog_post_translations
    ALTER COLUMN locale TYPE varchar(16);
"#,
            )
            .await?;
        Ok(())
    }
}
