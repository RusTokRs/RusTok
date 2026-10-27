use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

/// Adds durable two-phase collection state for published artifact CAS blobs
/// and for tenant artifact-data object bytes.
///
/// A blob is first marked as unreferenced and must remain untouched through
/// its grace period. Collection then requires a second transaction that
/// rechecks the live admission reference and records a `deleting` claim
/// before any external storage effect. A separate publication-reservation
/// row is the pending reference for bytes that have crossed the CAS publish
/// boundary but whose admission transaction has not committed yet.
/// Artifact-data candidates already had
/// the durable intent row; this migration gives that row the same claim and
/// revision fence so its external delete no longer runs inside the database
/// transaction that owns the namespace lock.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let statements: &[&str] = match backend {
            SupportedBackend::Postgres => &[
                "CREATE TABLE module_artifact_blob_gc_candidates (\
                    digest TEXT PRIMARY KEY,\
                    first_seen_unreferenced_at TIMESTAMPTZ NOT NULL,\
                    grace_ends_at TIMESTAMPTZ NOT NULL,\
                    state TEXT NOT NULL CHECK (state IN ('marked', 'deleting')),\
                    revision BIGINT NOT NULL CHECK (revision > 0),\
                    delete_started_at TIMESTAMPTZ NULL,\
                    created_at TIMESTAMPTZ NOT NULL,\
                    updated_at TIMESTAMPTZ NOT NULL,\
                    CHECK (grace_ends_at >= first_seen_unreferenced_at),\
                    CHECK ((state = 'marked' AND delete_started_at IS NULL) OR \
                           (state = 'deleting' AND delete_started_at IS NOT NULL))\
                )",
                "CREATE INDEX module_artifact_blob_gc_candidates_state_idx \
                 ON module_artifact_blob_gc_candidates (state, grace_ends_at, delete_started_at, digest)",
                "CREATE TABLE module_artifact_blob_publication_reservations (\
                    stage_id UUID PRIMARY KEY,\
                    digest TEXT NOT NULL,\
                    reserved_at TIMESTAMPTZ NOT NULL,\
                    expires_at TIMESTAMPTZ NOT NULL,\
                    CHECK (expires_at >= reserved_at)\
                )",
                "CREATE INDEX module_artifact_blob_publication_reservations_digest_idx \
                 ON module_artifact_blob_publication_reservations (digest, expires_at, stage_id)",
                "ALTER TABLE module_artifact_data_object_gc_candidates \
                 ADD COLUMN state TEXT NOT NULL DEFAULT 'queued' CHECK (state IN ('queued', 'deleting'))",
                "ALTER TABLE module_artifact_data_object_gc_candidates \
                 ADD COLUMN revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0)",
                "ALTER TABLE module_artifact_data_object_gc_candidates \
                 ADD COLUMN delete_started_at TIMESTAMPTZ NULL",
                "ALTER TABLE module_artifact_data_object_gc_candidates \
                 ADD CONSTRAINT module_artifact_data_object_gc_candidates_claim_check \
                 CHECK ((state = 'queued' AND delete_started_at IS NULL) OR \
                        (state = 'deleting' AND delete_started_at IS NOT NULL))",
                "CREATE INDEX module_artifact_data_object_gc_candidates_claim_idx \
                 ON module_artifact_data_object_gc_candidates (tenant_id, state, queued_at, candidate_id)",
            ],
            SupportedBackend::Sqlite => &[
                "CREATE TABLE module_artifact_blob_gc_candidates (\
                    digest TEXT PRIMARY KEY,\
                    first_seen_unreferenced_at TEXT NOT NULL,\
                    grace_ends_at TEXT NOT NULL,\
                    state TEXT NOT NULL CHECK (state IN ('marked', 'deleting')),\
                    revision INTEGER NOT NULL CHECK (revision > 0),\
                    delete_started_at TEXT NULL,\
                    created_at TEXT NOT NULL,\
                    updated_at TEXT NOT NULL,\
                    CHECK (grace_ends_at >= first_seen_unreferenced_at),\
                    CHECK ((state = 'marked' AND delete_started_at IS NULL) OR \
                           (state = 'deleting' AND delete_started_at IS NOT NULL))\
                )",
                "CREATE INDEX module_artifact_blob_gc_candidates_state_idx \
                 ON module_artifact_blob_gc_candidates (state, grace_ends_at, delete_started_at, digest)",
                "CREATE TABLE module_artifact_blob_publication_reservations (\
                    stage_id TEXT PRIMARY KEY,\
                    digest TEXT NOT NULL,\
                    reserved_at TEXT NOT NULL,\
                    expires_at TEXT NOT NULL,\
                    CHECK (expires_at >= reserved_at)\
                )",
                "CREATE INDEX module_artifact_blob_publication_reservations_digest_idx \
                 ON module_artifact_blob_publication_reservations (digest, expires_at, stage_id)",
                "ALTER TABLE module_artifact_data_object_gc_candidates \
                 ADD COLUMN state TEXT NOT NULL DEFAULT 'queued' CHECK (state IN ('queued', 'deleting'))",
                "ALTER TABLE module_artifact_data_object_gc_candidates \
                 ADD COLUMN revision INTEGER NOT NULL DEFAULT 1 CHECK (revision > 0)",
                "ALTER TABLE module_artifact_data_object_gc_candidates \
                 ADD COLUMN delete_started_at TEXT NULL",
                "CREATE INDEX module_artifact_data_object_gc_candidates_claim_idx \
                 ON module_artifact_data_object_gc_candidates (tenant_id, state, queued_at, candidate_id)",
            ],
        };
        for statement in statements {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(
                    backend.db_backend(),
                    (*statement).to_string(),
                ))
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let statements: &[&str] = match backend {
            SupportedBackend::Postgres => &[
                "DROP INDEX module_artifact_data_object_gc_candidates_claim_idx",
                "ALTER TABLE module_artifact_data_object_gc_candidates \
                 DROP CONSTRAINT module_artifact_data_object_gc_candidates_claim_check",
                "ALTER TABLE module_artifact_data_object_gc_candidates DROP COLUMN delete_started_at",
                "ALTER TABLE module_artifact_data_object_gc_candidates DROP COLUMN revision",
                "ALTER TABLE module_artifact_data_object_gc_candidates DROP COLUMN state",
                "DROP INDEX module_artifact_blob_publication_reservations_digest_idx",
                "DROP TABLE module_artifact_blob_publication_reservations",
                "DROP TABLE module_artifact_blob_gc_candidates",
            ],
            SupportedBackend::Sqlite => &[
                "DROP INDEX module_artifact_data_object_gc_candidates_claim_idx",
                "ALTER TABLE module_artifact_data_object_gc_candidates DROP COLUMN delete_started_at",
                "ALTER TABLE module_artifact_data_object_gc_candidates DROP COLUMN revision",
                "ALTER TABLE module_artifact_data_object_gc_candidates DROP COLUMN state",
                "DROP INDEX module_artifact_blob_publication_reservations_digest_idx",
                "DROP TABLE module_artifact_blob_publication_reservations",
                "DROP TABLE module_artifact_blob_gc_candidates",
            ],
        };
        for statement in statements {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(
                    backend.db_backend(),
                    (*statement).to_string(),
                ))
                .await?;
        }
        Ok(())
    }
}
