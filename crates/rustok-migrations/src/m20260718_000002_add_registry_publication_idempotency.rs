use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Persists immutable registry publication, review, and release-yank commands
/// for exact replay after their durable transitions commit.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statement = match manager.get_database_backend() {
            DbBackend::Postgres => {
                "CREATE TABLE registry_publication_operations (\
                operation_id UUID PRIMARY KEY,\
                request_id TEXT NOT NULL REFERENCES registry_publish_requests(id),\
                idempotency_key UUID NOT NULL,\
                actor_id UUID NOT NULL,\
                trace_id TEXT NOT NULL,\
                correlation_id UUID NOT NULL,\
                actor_principal JSONB NOT NULL,\
                publisher_principal JSONB NOT NULL,\
                allow_owner_rebind BOOLEAN NOT NULL,\
                approval_override JSONB NULL,\
                release_id TEXT NOT NULL REFERENCES registry_module_releases(id),\
                committed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                UNIQUE (request_id, idempotency_key)\
            );\
            CREATE TABLE registry_publish_request_review_operations (\
                operation_id UUID PRIMARY KEY,\
                request_id TEXT NOT NULL REFERENCES registry_publish_requests(id),\
                operation_kind TEXT NOT NULL CHECK (operation_kind IN ('reject', 'request_changes', 'hold', 'resume')),\
                idempotency_key UUID NOT NULL,\
                expected_revision BIGINT NOT NULL,\
                actor_id UUID NOT NULL,\
                trace_id TEXT NOT NULL,\
                correlation_id UUID NOT NULL,\
                actor_principal JSONB NOT NULL,\
                reason TEXT NOT NULL,\
                reason_code TEXT NOT NULL,\
                resulting_status TEXT NOT NULL,\
                resulting_revision BIGINT NOT NULL,\
                committed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                UNIQUE (request_id, idempotency_key)\
            );\
            CREATE TABLE registry_release_yank_operations (\
                operation_id UUID PRIMARY KEY,\
                release_id TEXT NOT NULL REFERENCES registry_module_releases(id),\
                idempotency_key UUID NOT NULL,\
                actor_id UUID NOT NULL,\
                trace_id TEXT NOT NULL,\
                correlation_id UUID NOT NULL,\
                actor_principal JSONB NOT NULL,\
                actor_can_manage_modules BOOLEAN NOT NULL,\
                reason TEXT NOT NULL,\
                reason_code TEXT NOT NULL,\
                resulting_status TEXT NOT NULL CHECK (resulting_status = 'yanked'),\
                committed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                UNIQUE (release_id, idempotency_key)\
            );\
            CREATE TABLE registry_owner_transfer_operations (\
                operation_id UUID PRIMARY KEY, slug TEXT NOT NULL REFERENCES registry_module_owners(slug),\
                idempotency_key UUID NOT NULL, actor_id UUID NOT NULL, trace_id TEXT NOT NULL, correlation_id UUID NOT NULL,\
                previous_owner_principal JSONB NOT NULL, new_owner_principal JSONB NOT NULL, actor_principal JSONB NOT NULL,\
                actor_can_manage_modules BOOLEAN NOT NULL, reason TEXT NOT NULL, reason_code TEXT NOT NULL,\
                committed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (slug, idempotency_key)\
            );\
            CREATE TABLE registry_publish_request_create_operations (\
                operation_id UUID PRIMARY KEY, request_id TEXT NOT NULL REFERENCES registry_publish_requests(id),\
                idempotency_key UUID NOT NULL, command_digest TEXT NOT NULL, actor_id UUID NOT NULL,\
                trace_id TEXT NOT NULL, correlation_id UUID NOT NULL, actor_principal JSONB NOT NULL,\
                actor_can_manage_modules BOOLEAN NOT NULL, committed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                UNIQUE (request_id, idempotency_key)\
            );\
            CREATE TABLE registry_publish_artifact_operations (\
                operation_id UUID PRIMARY KEY, request_id TEXT NOT NULL REFERENCES registry_publish_requests(id),\
                idempotency_key UUID NOT NULL, expected_revision BIGINT NOT NULL, actor_id UUID NOT NULL,\
                trace_id TEXT NOT NULL, correlation_id UUID NOT NULL, actor_principal JSONB NOT NULL,\
                actor_can_manage_modules BOOLEAN NOT NULL, checksum_sha256 TEXT NOT NULL, artifact_size BIGINT NOT NULL,\
                content_type TEXT NOT NULL, artifact_storage_key TEXT NOT NULL, previous_storage_key TEXT NULL,\
                reuploaded_after_changes_requested BOOLEAN NOT NULL,\
                committed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (request_id, idempotency_key)\
            );\
            CREATE TABLE registry_author_signature_evidence_operations (\
                operation_id UUID PRIMARY KEY, request_id TEXT NOT NULL REFERENCES registry_publish_requests(id),\
                idempotency_key UUID NOT NULL, expected_revision BIGINT NOT NULL, actor_id UUID NOT NULL,\
                trace_id TEXT NOT NULL, correlation_id UUID NOT NULL, actor_principal JSONB NOT NULL,\
                subject_digest_sha256 TEXT NOT NULL CHECK (length(subject_digest_sha256) = 64),\
                evidence_reference TEXT NOT NULL,\
                signature_digest_sha256 TEXT NOT NULL CHECK (length(signature_digest_sha256) = 64),\
                signer_identity TEXT NOT NULL, policy_revision TEXT NOT NULL,\
                evidence_id TEXT NOT NULL REFERENCES registry_publication_evidence(id),\
                resulting_revision BIGINT NOT NULL, recorded BOOLEAN NOT NULL,\
                committed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (request_id, idempotency_key)\
            );\
            CREATE TABLE registry_validation_job_enqueue_operations (\
                operation_id UUID PRIMARY KEY, request_id TEXT NOT NULL REFERENCES registry_publish_requests(id),\
                idempotency_key UUID NOT NULL, expected_revision BIGINT NOT NULL, actor_id UUID NOT NULL,\
                trace_id TEXT NOT NULL, correlation_id UUID NOT NULL, actor_principal JSONB NOT NULL,\
                allow_rejected_retry BOOLEAN NOT NULL, request_status TEXT NOT NULL, queued BOOLEAN NOT NULL,\
                validation_job_id TEXT NULL, committed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                UNIQUE (request_id, idempotency_key)\
            );\
            CREATE TABLE registry_validation_stage_report_operations (\
                operation_id UUID PRIMARY KEY, request_id TEXT NOT NULL REFERENCES registry_publish_requests(id),\
                idempotency_key UUID NOT NULL, expected_revision BIGINT NOT NULL, actor_id UUID NOT NULL,\
                trace_id TEXT NOT NULL, correlation_id UUID NOT NULL, actor_principal JSONB NOT NULL,\
                stage_key TEXT NOT NULL, status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'passed', 'failed', 'blocked')),\
                reason_code TEXT NULL, requeue BOOLEAN NOT NULL, stage_id TEXT NOT NULL,\
                resulting_request_revision BIGINT NOT NULL, committed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                UNIQUE (request_id, idempotency_key)\
            )"
            }
            DbBackend::Sqlite => {
                "CREATE TABLE registry_publication_operations (\
                operation_id TEXT PRIMARY KEY NOT NULL,\
                request_id TEXT NOT NULL REFERENCES registry_publish_requests(id),\
                idempotency_key TEXT NOT NULL,\
                actor_id TEXT NOT NULL,\
                trace_id TEXT NOT NULL,\
                correlation_id TEXT NOT NULL,\
                actor_principal JSON NOT NULL,\
                publisher_principal JSON NOT NULL,\
                allow_owner_rebind INTEGER NOT NULL CHECK (allow_owner_rebind IN (0, 1)),\
                approval_override JSON NULL,\
                release_id TEXT NOT NULL REFERENCES registry_module_releases(id),\
                committed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                UNIQUE (request_id, idempotency_key)\
            );\
            CREATE TABLE registry_publish_request_review_operations (\
                operation_id TEXT PRIMARY KEY NOT NULL,\
                request_id TEXT NOT NULL REFERENCES registry_publish_requests(id),\
                operation_kind TEXT NOT NULL CHECK (operation_kind IN ('reject', 'request_changes', 'hold', 'resume')),\
                idempotency_key TEXT NOT NULL,\
                expected_revision INTEGER NOT NULL,\
                actor_id TEXT NOT NULL,\
                trace_id TEXT NOT NULL,\
                correlation_id TEXT NOT NULL,\
                actor_principal JSON NOT NULL,\
                reason TEXT NOT NULL,\
                reason_code TEXT NOT NULL,\
                resulting_status TEXT NOT NULL,\
                resulting_revision INTEGER NOT NULL,\
                committed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                UNIQUE (request_id, idempotency_key)\
            );\
            CREATE TABLE registry_release_yank_operations (\
                operation_id TEXT PRIMARY KEY NOT NULL,\
                release_id TEXT NOT NULL REFERENCES registry_module_releases(id),\
                idempotency_key TEXT NOT NULL,\
                actor_id TEXT NOT NULL,\
                trace_id TEXT NOT NULL,\
                correlation_id TEXT NOT NULL,\
                actor_principal JSON NOT NULL,\
                actor_can_manage_modules INTEGER NOT NULL CHECK (actor_can_manage_modules IN (0, 1)),\
                reason TEXT NOT NULL,\
                reason_code TEXT NOT NULL,\
                resulting_status TEXT NOT NULL CHECK (resulting_status = 'yanked'),\
                committed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                UNIQUE (release_id, idempotency_key)\
            );\
            CREATE TABLE registry_owner_transfer_operations (\
                operation_id TEXT PRIMARY KEY NOT NULL, slug TEXT NOT NULL REFERENCES registry_module_owners(slug),\
                idempotency_key TEXT NOT NULL, actor_id TEXT NOT NULL, trace_id TEXT NOT NULL, correlation_id TEXT NOT NULL,\
                previous_owner_principal JSON NOT NULL, new_owner_principal JSON NOT NULL, actor_principal JSON NOT NULL,\
                actor_can_manage_modules INTEGER NOT NULL CHECK (actor_can_manage_modules IN (0, 1)),\
                reason TEXT NOT NULL, reason_code TEXT NOT NULL, committed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                UNIQUE (slug, idempotency_key)\
            );\
            CREATE TABLE registry_publish_request_create_operations (\
                operation_id TEXT PRIMARY KEY NOT NULL, request_id TEXT NOT NULL REFERENCES registry_publish_requests(id),\
                idempotency_key TEXT NOT NULL, command_digest TEXT NOT NULL, actor_id TEXT NOT NULL,\
                trace_id TEXT NOT NULL, correlation_id TEXT NOT NULL, actor_principal JSON NOT NULL,\
                actor_can_manage_modules INTEGER NOT NULL CHECK (actor_can_manage_modules IN (0, 1)),\
                committed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (request_id, idempotency_key)\
            );\
            CREATE TABLE registry_publish_artifact_operations (\
                operation_id TEXT PRIMARY KEY NOT NULL, request_id TEXT NOT NULL REFERENCES registry_publish_requests(id),\
                idempotency_key TEXT NOT NULL, expected_revision INTEGER NOT NULL, actor_id TEXT NOT NULL,\
                trace_id TEXT NOT NULL, correlation_id TEXT NOT NULL, actor_principal JSON NOT NULL,\
                actor_can_manage_modules INTEGER NOT NULL CHECK (actor_can_manage_modules IN (0, 1)),\
                checksum_sha256 TEXT NOT NULL, artifact_size INTEGER NOT NULL, content_type TEXT NOT NULL,\
                artifact_storage_key TEXT NOT NULL, previous_storage_key TEXT NULL,\
                reuploaded_after_changes_requested INTEGER NOT NULL CHECK (reuploaded_after_changes_requested IN (0, 1)),\
                committed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (request_id, idempotency_key)\
            );\
            CREATE TABLE registry_author_signature_evidence_operations (\
                operation_id TEXT PRIMARY KEY NOT NULL, request_id TEXT NOT NULL REFERENCES registry_publish_requests(id),\
                idempotency_key TEXT NOT NULL, expected_revision INTEGER NOT NULL, actor_id TEXT NOT NULL,\
                trace_id TEXT NOT NULL, correlation_id TEXT NOT NULL, actor_principal JSON NOT NULL,\
                subject_digest_sha256 TEXT NOT NULL CHECK (length(subject_digest_sha256) = 64),\
                evidence_reference TEXT NOT NULL,\
                signature_digest_sha256 TEXT NOT NULL CHECK (length(signature_digest_sha256) = 64),\
                signer_identity TEXT NOT NULL, policy_revision TEXT NOT NULL,\
                evidence_id TEXT NOT NULL REFERENCES registry_publication_evidence(id),\
                resulting_revision INTEGER NOT NULL, recorded INTEGER NOT NULL CHECK (recorded IN (0, 1)),\
                committed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (request_id, idempotency_key)\
            );\
            CREATE TABLE registry_validation_job_enqueue_operations (\
                operation_id TEXT PRIMARY KEY NOT NULL, request_id TEXT NOT NULL REFERENCES registry_publish_requests(id),\
                idempotency_key TEXT NOT NULL, expected_revision INTEGER NOT NULL, actor_id TEXT NOT NULL,\
                trace_id TEXT NOT NULL, correlation_id TEXT NOT NULL, actor_principal JSON NOT NULL,\
                allow_rejected_retry INTEGER NOT NULL CHECK (allow_rejected_retry IN (0, 1)),\
                request_status TEXT NOT NULL, queued INTEGER NOT NULL CHECK (queued IN (0, 1)),\
                validation_job_id TEXT NULL, committed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                UNIQUE (request_id, idempotency_key)\
            );\
            CREATE TABLE registry_validation_stage_report_operations (\
                operation_id TEXT PRIMARY KEY NOT NULL, request_id TEXT NOT NULL REFERENCES registry_publish_requests(id),\
                idempotency_key TEXT NOT NULL, expected_revision INTEGER NOT NULL, actor_id TEXT NOT NULL,\
                trace_id TEXT NOT NULL, correlation_id TEXT NOT NULL, actor_principal JSON NOT NULL,\
                stage_key TEXT NOT NULL, status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'passed', 'failed', 'blocked')),\
                reason_code TEXT NULL, requeue INTEGER NOT NULL CHECK (requeue IN (0, 1)), stage_id TEXT NOT NULL,\
                resulting_request_revision INTEGER NOT NULL, committed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                UNIQUE (request_id, idempotency_key)\
            )"
            }
            backend => {
                return Err(DbErr::Migration(format!(
                    "registry publication idempotency migration does not support database backend {backend:?}"
                )));
            }
        };
        for statement in statement
            .split(';')
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(
                    manager.get_database_backend(),
                    statement.to_string(),
                ))
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE registry_validation_stage_report_operations")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE registry_validation_job_enqueue_operations")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE registry_publish_artifact_operations")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE registry_author_signature_evidence_operations")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE registry_publish_request_create_operations")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE registry_owner_transfer_operations")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE registry_release_yank_operations")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE registry_publish_request_review_operations")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE registry_publication_operations")
            .await
            .map(|_| ())
    }
}
