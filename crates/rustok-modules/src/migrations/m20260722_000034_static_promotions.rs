use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Creates platform-scoped trusted static-promotion review and immutable
/// distribution build-intent aggregates. Neither aggregate edits the running
/// server Cargo graph or installs a native payload at runtime.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_static_promotions (\
                    promotion_id UUID PRIMARY KEY,\
                    release_id TEXT NOT NULL REFERENCES registry_module_releases(id) ON DELETE RESTRICT,\
                    publish_request_id TEXT NOT NULL REFERENCES registry_publish_requests(id) ON DELETE RESTRICT,\
                    module_slug TEXT NOT NULL CHECK (length(trim(module_slug)) BETWEEN 1 AND 128),\
                    module_version TEXT NOT NULL CHECK (length(trim(module_version)) BETWEEN 1 AND 128),\
                    cargo_package TEXT NOT NULL CHECK (length(trim(cargo_package)) BETWEEN 1 AND 128),\
                    entry_type TEXT NOT NULL CHECK (length(trim(entry_type)) BETWEEN 1 AND 256),\
                    source_reference TEXT NOT NULL CHECK (length(trim(source_reference)) BETWEEN 1 AND 512),\
                    source_digest TEXT NOT NULL CHECK (source_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    dependency_lock_digest TEXT NOT NULL CHECK (dependency_lock_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    status TEXT NOT NULL CHECK (status IN ('requested', 'approved')),\
                    revision BIGINT NOT NULL CHECK (revision > 0),\
                    requested_by UUID NOT NULL,\
                    requested_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    approved_by UUID NULL,\
                    approved_at TIMESTAMPTZ NULL,\
                    CHECK ((status = 'requested' AND approved_by IS NULL AND approved_at IS NULL) OR\
                           (status = 'approved' AND approved_by IS NOT NULL AND approved_at IS NOT NULL)),\
                    UNIQUE (release_id)\
                )",
                "CREATE INDEX module_static_promotions_status_idx ON module_static_promotions (status, requested_at, promotion_id)",
                "CREATE TABLE module_static_promotion_reviews (\
                    review_id UUID PRIMARY KEY,\
                    promotion_id UUID NOT NULL UNIQUE REFERENCES module_static_promotions(promotion_id) ON DELETE RESTRICT,\
                    ownership_evidence_reference TEXT NOT NULL CHECK (length(trim(ownership_evidence_reference)) BETWEEN 1 AND 512),\
                    ownership_evidence_digest TEXT NOT NULL CHECK (ownership_evidence_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    dependency_audit_reference TEXT NOT NULL CHECK (length(trim(dependency_audit_reference)) BETWEEN 1 AND 512),\
                    dependency_audit_digest TEXT NOT NULL CHECK (dependency_audit_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    test_evidence_reference TEXT NOT NULL CHECK (length(trim(test_evidence_reference)) BETWEEN 1 AND 512),\
                    test_evidence_digest TEXT NOT NULL CHECK (test_evidence_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    static_review_reference TEXT NOT NULL CHECK (length(trim(static_review_reference)) BETWEEN 1 AND 512),\
                    static_review_digest TEXT NOT NULL CHECK (static_review_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    approval_policy_revision TEXT NOT NULL CHECK (length(trim(approval_policy_revision)) BETWEEN 1 AND 128),\
                    review_digest TEXT NOT NULL CHECK (review_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    reviewed_by UUID NOT NULL,\
                    reviewed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP\
                )",
                "CREATE TABLE module_static_promotion_operations (\
                    idempotency_key UUID PRIMARY KEY,\
                    operation_kind TEXT NOT NULL CHECK (operation_kind IN ('request', 'approve')),\
                    request_digest TEXT NOT NULL CHECK (request_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    actor_id UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0 AND length(trace_id) <= 512),\
                    correlation_id UUID NOT NULL,\
                    promotion_id UUID NULL REFERENCES module_static_promotions(promotion_id) ON DELETE RESTRICT,\
                    result_revision BIGINT NULL CHECK (result_revision IS NULL OR result_revision > 0),\
                    result_status TEXT NULL CHECK (result_status IS NULL OR result_status IN ('requested', 'approved')),\
                    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    completed_at TIMESTAMPTZ NULL,\
                    CHECK ((promotion_id IS NULL AND result_revision IS NULL AND result_status IS NULL AND completed_at IS NULL) OR\
                           (promotion_id IS NOT NULL AND result_revision IS NOT NULL AND result_status IS NOT NULL AND completed_at IS NOT NULL))\
                )",
                "CREATE TABLE module_static_distribution_builds (\
                    distribution_build_id UUID PRIMARY KEY,\
                    predecessor_build_id UUID NULL REFERENCES module_static_distribution_builds(distribution_build_id) ON DELETE RESTRICT,\
                    composition_revision BIGINT NOT NULL UNIQUE CHECK (composition_revision > 0),\
                    composition_digest TEXT NOT NULL CHECK (composition_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    platform_source_reference TEXT NOT NULL CHECK (length(trim(platform_source_reference)) BETWEEN 1 AND 512),\
                    platform_source_digest TEXT NOT NULL CHECK (platform_source_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    toolchain_digest TEXT NOT NULL CHECK (toolchain_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    build_target TEXT NOT NULL CHECK (length(trim(build_target)) BETWEEN 1 AND 128),\
                    preparation_source TEXT NOT NULL DEFAULT 'built' CHECK (preparation_source IN ('built', 'signed_bootstrap')),\
                    bootstrap_receipt_digest TEXT NULL CHECK (bootstrap_receipt_digest IS NULL OR bootstrap_receipt_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'succeeded', 'failed', 'cancelled')),\
                    requested_by UUID NOT NULL,\
                    requested_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),\
                    active_claim_id UUID NULL,\
                    claimed_by TEXT NULL,\
                    lease_expires_at TIMESTAMPTZ NULL,\
                    last_heartbeat_at TIMESTAMPTZ NULL,\
                    bundle_reference TEXT NULL,\
                    bundle_root_digest TEXT NULL CHECK (bundle_root_digest IS NULL OR bundle_root_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    role_set_digest TEXT NULL CHECK (role_set_digest IS NULL OR role_set_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    role_artifacts_json TEXT NULL,\
                    sbom_reference TEXT NULL,\
                    sbom_digest TEXT NULL CHECK (sbom_digest IS NULL OR sbom_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    provenance_reference TEXT NULL,\
                    provenance_digest TEXT NULL CHECK (provenance_digest IS NULL OR provenance_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    signature_reference TEXT NULL,\
                    signature_digest TEXT NULL CHECK (signature_digest IS NULL OR signature_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    test_evidence_reference TEXT NULL,\
                    test_evidence_digest TEXT NULL CHECK (test_evidence_digest IS NULL OR test_evidence_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    failure_code TEXT NULL,\
                    failure_detail TEXT NULL,\
                    completion_digest TEXT NULL CHECK (completion_digest IS NULL OR completion_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    started_at TIMESTAMPTZ NULL,\
                    completed_at TIMESTAMPTZ NULL,\
                    CHECK (\
                        (status = 'queued' AND active_claim_id IS NULL AND claimed_by IS NULL AND lease_expires_at IS NULL AND bundle_reference IS NULL AND bundle_root_digest IS NULL AND role_set_digest IS NULL AND role_artifacts_json IS NULL AND sbom_reference IS NULL AND sbom_digest IS NULL AND provenance_reference IS NULL AND provenance_digest IS NULL AND signature_reference IS NULL AND signature_digest IS NULL AND test_evidence_reference IS NULL AND test_evidence_digest IS NULL AND failure_code IS NULL AND failure_detail IS NULL AND completion_digest IS NULL AND completed_at IS NULL)\
                        OR (status = 'running' AND active_claim_id IS NOT NULL AND claimed_by IS NOT NULL AND lease_expires_at IS NOT NULL AND started_at IS NOT NULL AND bundle_reference IS NULL AND bundle_root_digest IS NULL AND role_set_digest IS NULL AND role_artifacts_json IS NULL AND sbom_reference IS NULL AND sbom_digest IS NULL AND provenance_reference IS NULL AND provenance_digest IS NULL AND signature_reference IS NULL AND signature_digest IS NULL AND test_evidence_reference IS NULL AND test_evidence_digest IS NULL AND failure_code IS NULL AND failure_detail IS NULL AND completion_digest IS NULL AND completed_at IS NULL)\
                        OR (status = 'succeeded' AND lease_expires_at IS NULL AND bundle_reference IS NOT NULL AND bundle_root_digest IS NOT NULL AND role_set_digest IS NOT NULL AND role_artifacts_json IS NOT NULL AND sbom_reference IS NOT NULL AND sbom_digest IS NOT NULL AND provenance_reference IS NOT NULL AND provenance_digest IS NOT NULL AND signature_reference IS NOT NULL AND signature_digest IS NOT NULL AND test_evidence_reference IS NOT NULL AND test_evidence_digest IS NOT NULL AND failure_code IS NULL AND failure_detail IS NULL AND completion_digest IS NOT NULL AND completed_at IS NOT NULL AND ((preparation_source = 'built' AND active_claim_id IS NOT NULL AND claimed_by IS NOT NULL AND attempt_count > 0 AND bootstrap_receipt_digest IS NULL) OR (preparation_source = 'signed_bootstrap' AND active_claim_id IS NULL AND claimed_by IS NULL AND attempt_count = 0 AND bootstrap_receipt_digest IS NOT NULL)))\
                        OR (status IN ('failed', 'cancelled') AND active_claim_id IS NOT NULL AND claimed_by IS NOT NULL AND lease_expires_at IS NULL AND bundle_reference IS NULL AND bundle_root_digest IS NULL AND role_set_digest IS NULL AND role_artifacts_json IS NULL AND sbom_reference IS NULL AND sbom_digest IS NULL AND provenance_reference IS NULL AND provenance_digest IS NULL AND signature_reference IS NULL AND signature_digest IS NULL AND test_evidence_reference IS NULL AND test_evidence_digest IS NULL AND failure_code IS NOT NULL AND failure_detail IS NOT NULL AND completion_digest IS NOT NULL AND completed_at IS NOT NULL)\
                    )\
                )",
                "CREATE INDEX module_static_distribution_builds_queue_idx ON module_static_distribution_builds (status, requested_at, distribution_build_id)",
                "CREATE TABLE module_static_distribution_attempts (\
                    claim_id UUID PRIMARY KEY,\
                    distribution_build_id UUID NOT NULL REFERENCES module_static_distribution_builds(distribution_build_id) ON DELETE RESTRICT,\
                    attempt_number INTEGER NOT NULL CHECK (attempt_number > 0),\
                    runner_id TEXT NOT NULL CHECK (length(trim(runner_id)) BETWEEN 1 AND 128),\
                    status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed', 'cancelled', 'lease_expired')),\
                    lease_expires_at TIMESTAMPTZ NOT NULL,\
                    last_heartbeat_at TIMESTAMPTZ NOT NULL,\
                    bundle_reference TEXT NULL,\
                    bundle_root_digest TEXT NULL CHECK (bundle_root_digest IS NULL OR bundle_root_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    role_set_digest TEXT NULL CHECK (role_set_digest IS NULL OR role_set_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    role_artifacts_json TEXT NULL,\
                    sbom_reference TEXT NULL,\
                    sbom_digest TEXT NULL CHECK (sbom_digest IS NULL OR sbom_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    provenance_reference TEXT NULL,\
                    provenance_digest TEXT NULL CHECK (provenance_digest IS NULL OR provenance_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    signature_reference TEXT NULL,\
                    signature_digest TEXT NULL CHECK (signature_digest IS NULL OR signature_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    test_evidence_reference TEXT NULL,\
                    test_evidence_digest TEXT NULL CHECK (test_evidence_digest IS NULL OR test_evidence_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    failure_code TEXT NULL,\
                    failure_detail TEXT NULL,\
                    completion_digest TEXT NULL CHECK (completion_digest IS NULL OR completion_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    started_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    completed_at TIMESTAMPTZ NULL,\
                    UNIQUE (distribution_build_id, attempt_number),\
                    CHECK (\
                        (status = 'running' AND bundle_reference IS NULL AND bundle_root_digest IS NULL AND role_set_digest IS NULL AND role_artifacts_json IS NULL AND sbom_reference IS NULL AND sbom_digest IS NULL AND provenance_reference IS NULL AND provenance_digest IS NULL AND signature_reference IS NULL AND signature_digest IS NULL AND test_evidence_reference IS NULL AND test_evidence_digest IS NULL AND failure_code IS NULL AND failure_detail IS NULL AND completion_digest IS NULL AND completed_at IS NULL)\
                        OR (status = 'lease_expired' AND bundle_reference IS NULL AND bundle_root_digest IS NULL AND role_set_digest IS NULL AND role_artifacts_json IS NULL AND sbom_reference IS NULL AND sbom_digest IS NULL AND provenance_reference IS NULL AND provenance_digest IS NULL AND signature_reference IS NULL AND signature_digest IS NULL AND test_evidence_reference IS NULL AND test_evidence_digest IS NULL AND failure_code IS NULL AND failure_detail IS NULL AND completion_digest IS NULL AND completed_at IS NOT NULL)\
                        OR (status = 'succeeded' AND bundle_reference IS NOT NULL AND bundle_root_digest IS NOT NULL AND role_set_digest IS NOT NULL AND role_artifacts_json IS NOT NULL AND sbom_reference IS NOT NULL AND sbom_digest IS NOT NULL AND provenance_reference IS NOT NULL AND provenance_digest IS NOT NULL AND signature_reference IS NOT NULL AND signature_digest IS NOT NULL AND test_evidence_reference IS NOT NULL AND test_evidence_digest IS NOT NULL AND failure_code IS NULL AND failure_detail IS NULL AND completion_digest IS NOT NULL AND completed_at IS NOT NULL)\
                        OR (status IN ('failed', 'cancelled') AND bundle_reference IS NULL AND bundle_root_digest IS NULL AND role_set_digest IS NULL AND role_artifacts_json IS NULL AND sbom_reference IS NULL AND sbom_digest IS NULL AND provenance_reference IS NULL AND provenance_digest IS NULL AND signature_reference IS NULL AND signature_digest IS NULL AND test_evidence_reference IS NULL AND test_evidence_digest IS NULL AND failure_code IS NOT NULL AND failure_detail IS NOT NULL AND completion_digest IS NOT NULL AND completed_at IS NOT NULL)\
                    )\
                )",
                "CREATE TABLE module_static_distribution_state (\
                    state_id TEXT PRIMARY KEY CHECK (state_id = 'current'),\
                    revision BIGINT NOT NULL CHECK (revision >= 0),\
                    current_build_id UUID NULL REFERENCES module_static_distribution_builds(distribution_build_id) ON DELETE RESTRICT,\
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP\
                )",
                "INSERT INTO module_static_distribution_state (state_id, revision, current_build_id)\
                 VALUES ('current', 0, NULL)",
                "CREATE TABLE module_static_distribution_items (\
                    distribution_build_id UUID NOT NULL REFERENCES module_static_distribution_builds(distribution_build_id) ON DELETE RESTRICT,\
                    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),\
                    promotion_id UUID NOT NULL,\
                    promotion_revision BIGINT NOT NULL CHECK (promotion_revision > 0),\
                    release_id TEXT NOT NULL,\
                    module_slug TEXT NOT NULL CHECK (length(trim(module_slug)) BETWEEN 1 AND 128),\
                    module_version TEXT NOT NULL CHECK (length(trim(module_version)) BETWEEN 1 AND 128),\
                    cargo_package TEXT NOT NULL CHECK (length(trim(cargo_package)) BETWEEN 1 AND 128),\
                    entry_type TEXT NOT NULL CHECK (length(trim(entry_type)) BETWEEN 1 AND 256),\
                    source_reference TEXT NOT NULL CHECK (length(trim(source_reference)) BETWEEN 1 AND 512),\
                    source_digest TEXT NOT NULL CHECK (source_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    dependency_lock_digest TEXT NOT NULL CHECK (dependency_lock_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    executor_mode TEXT NOT NULL CHECK (executor_mode = 'static_native'),\
                    PRIMARY KEY (distribution_build_id, ordinal),\
                    UNIQUE (distribution_build_id, promotion_id),\
                    UNIQUE (distribution_build_id, module_slug)\
                )",
                "CREATE TABLE module_static_distribution_operations (\
                    idempotency_key UUID PRIMARY KEY,\
                    request_digest TEXT NOT NULL CHECK (request_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    actor_id UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    correlation_id UUID NOT NULL,\
                    distribution_build_id UUID NULL REFERENCES module_static_distribution_builds(distribution_build_id) ON DELETE RESTRICT,\
                    composition_revision BIGINT NULL CHECK (composition_revision IS NULL OR composition_revision > 0),\
                    composition_digest TEXT NULL CHECK (composition_digest IS NULL OR composition_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    completed_at TIMESTAMPTZ NULL,\
                    CHECK ((distribution_build_id IS NULL AND composition_revision IS NULL AND composition_digest IS NULL AND completed_at IS NULL) OR\
                           (distribution_build_id IS NOT NULL AND composition_revision IS NOT NULL AND composition_digest IS NOT NULL AND completed_at IS NOT NULL))\
                )",
                "CREATE TABLE module_static_distribution_releases (\
                    distribution_release_id UUID PRIMARY KEY,\
                    distribution_build_id UUID NOT NULL UNIQUE REFERENCES module_static_distribution_builds(distribution_build_id) ON DELETE RESTRICT,\
                    predecessor_release_id UUID NULL REFERENCES module_static_distribution_releases(distribution_release_id) ON DELETE RESTRICT,\
                    release_revision BIGINT NOT NULL UNIQUE CHECK (release_revision > 0),\
                    composition_revision BIGINT NOT NULL CHECK (composition_revision > 0),\
                    composition_digest TEXT NOT NULL CHECK (composition_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    bundle_reference TEXT NOT NULL CHECK (length(trim(bundle_reference)) BETWEEN 1 AND 512),\
                    bundle_root_digest TEXT NOT NULL CHECK (bundle_root_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    role_set_digest TEXT NOT NULL CHECK (role_set_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    role_artifacts_json TEXT NOT NULL,\
                    sbom_reference TEXT NOT NULL CHECK (length(trim(sbom_reference)) BETWEEN 1 AND 512),\
                    sbom_digest TEXT NOT NULL CHECK (sbom_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    provenance_reference TEXT NOT NULL CHECK (length(trim(provenance_reference)) BETWEEN 1 AND 512),\
                    provenance_digest TEXT NOT NULL CHECK (provenance_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    signature_reference TEXT NOT NULL CHECK (length(trim(signature_reference)) BETWEEN 1 AND 512),\
                    signature_digest TEXT NOT NULL CHECK (signature_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    test_evidence_reference TEXT NOT NULL CHECK (length(trim(test_evidence_reference)) BETWEEN 1 AND 512),\
                    test_evidence_digest TEXT NOT NULL CHECK (test_evidence_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    status TEXT NOT NULL CHECK (status IN ('admitted', 'active', 'superseded', 'revoked')),\
                    admitted_by UUID NOT NULL,\
                    admitted_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    activated_at TIMESTAMPTZ NULL,\
                    superseded_at TIMESTAMPTZ NULL,\
                    revoked_by UUID NULL,\
                    revoked_at TIMESTAMPTZ NULL,\
                    revocation_reason TEXT NULL CHECK (revocation_reason IS NULL OR length(trim(revocation_reason)) BETWEEN 1 AND 1024),\
                    revocation_policy_revision TEXT NULL CHECK (revocation_policy_revision IS NULL OR length(trim(revocation_policy_revision)) BETWEEN 1 AND 128),\
                    CHECK ((status = 'admitted' AND activated_at IS NULL AND superseded_at IS NULL AND revoked_by IS NULL AND revoked_at IS NULL AND revocation_reason IS NULL AND revocation_policy_revision IS NULL) OR\
                           (status = 'active' AND activated_at IS NOT NULL AND superseded_at IS NULL AND revoked_by IS NULL AND revoked_at IS NULL AND revocation_reason IS NULL AND revocation_policy_revision IS NULL) OR\
                           (status = 'superseded' AND activated_at IS NOT NULL AND superseded_at IS NOT NULL AND revoked_by IS NULL AND revoked_at IS NULL AND revocation_reason IS NULL AND revocation_policy_revision IS NULL) OR\
                           (status = 'revoked' AND revoked_by IS NOT NULL AND revoked_at IS NOT NULL AND revocation_reason IS NOT NULL AND revocation_policy_revision IS NOT NULL))\
                )",
                "CREATE UNIQUE INDEX module_static_distribution_releases_active_idx ON module_static_distribution_releases (status) WHERE status = 'active'",
                "CREATE TABLE module_static_distribution_release_state (\
                    state_id TEXT PRIMARY KEY CHECK (state_id = 'current'),\
                    revision BIGINT NOT NULL CHECK (revision >= 0),\
                    active_release_id UUID NULL REFERENCES module_static_distribution_releases(distribution_release_id) ON DELETE RESTRICT,\
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP\
                )",
                "INSERT INTO module_static_distribution_release_state (state_id, revision, active_release_id) VALUES ('current', 0, NULL)",
                "CREATE TABLE module_static_distribution_release_admissions (\
                    admission_id UUID PRIMARY KEY,\
                    distribution_release_id UUID NOT NULL UNIQUE REFERENCES module_static_distribution_releases(distribution_release_id) ON DELETE RESTRICT,\
                    verifier_identity TEXT NOT NULL CHECK (length(trim(verifier_identity)) BETWEEN 1 AND 256),\
                    policy_revision TEXT NOT NULL CHECK (length(trim(policy_revision)) BETWEEN 1 AND 128),\
                    evidence_reference TEXT NOT NULL CHECK (length(trim(evidence_reference)) BETWEEN 1 AND 512),\
                    evidence_digest TEXT NOT NULL CHECK (evidence_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    signature_verified BOOLEAN NOT NULL CHECK (signature_verified),\
                    provenance_verified BOOLEAN NOT NULL CHECK (provenance_verified),\
                    sbom_verified BOOLEAN NOT NULL CHECK (sbom_verified),\
                    test_evidence_verified BOOLEAN NOT NULL CHECK (test_evidence_verified),\
                    dependency_policy_verified BOOLEAN NOT NULL CHECK (dependency_policy_verified),\
                    verified_at TIMESTAMPTZ NOT NULL\
                )",
                "CREATE TABLE module_static_distribution_admission_operations (\
                    idempotency_key UUID PRIMARY KEY,\
                    request_digest TEXT NOT NULL CHECK (request_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    actor_id UUID NOT NULL,\
                    distribution_release_id UUID NULL REFERENCES module_static_distribution_releases(distribution_release_id) ON DELETE RESTRICT,\
                    release_revision BIGINT NULL CHECK (release_revision IS NULL OR release_revision > 0),\
                    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    completed_at TIMESTAMPTZ NULL,\
                    CHECK ((distribution_release_id IS NULL AND release_revision IS NULL AND completed_at IS NULL) OR\
                           (distribution_release_id IS NOT NULL AND release_revision IS NOT NULL AND completed_at IS NOT NULL))\
                )",
                "CREATE TABLE module_static_distribution_revocation_operations (\
                    idempotency_key UUID PRIMARY KEY,\
                    request_digest TEXT NOT NULL CHECK (request_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    actor_id UUID NOT NULL,\
                    distribution_release_id UUID NULL REFERENCES module_static_distribution_releases(distribution_release_id) ON DELETE RESTRICT,\
                    release_state_revision BIGINT NULL CHECK (release_state_revision IS NULL OR release_state_revision > 0),\
                    was_active BOOLEAN NULL,\
                    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    completed_at TIMESTAMPTZ NULL,\
                    CHECK ((distribution_release_id IS NULL AND release_state_revision IS NULL AND was_active IS NULL AND completed_at IS NULL) OR\
                           (distribution_release_id IS NOT NULL AND release_state_revision IS NOT NULL AND was_active IS NOT NULL AND completed_at IS NOT NULL))\
                )",
                "CREATE TABLE module_static_distribution_release_idempotency_keys (\
                    idempotency_key UUID PRIMARY KEY,\
                    operation_kind TEXT NOT NULL CHECK (operation_kind IN ('bootstrap_import', 'admit', 'revoke')),\
                    request_digest TEXT NOT NULL CHECK (request_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    actor_id UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0 AND length(trace_id) <= 512),\
                    correlation_id UUID NOT NULL,\
                    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP\
                )",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_static_promotions (\
                    promotion_id TEXT PRIMARY KEY,\
                    release_id TEXT NOT NULL REFERENCES registry_module_releases(id) ON DELETE RESTRICT,\
                    publish_request_id TEXT NOT NULL REFERENCES registry_publish_requests(id) ON DELETE RESTRICT,\
                    module_slug TEXT NOT NULL CHECK (length(trim(module_slug)) BETWEEN 1 AND 128),\
                    module_version TEXT NOT NULL CHECK (length(trim(module_version)) BETWEEN 1 AND 128),\
                    cargo_package TEXT NOT NULL CHECK (length(trim(cargo_package)) BETWEEN 1 AND 128),\
                    entry_type TEXT NOT NULL CHECK (length(trim(entry_type)) BETWEEN 1 AND 256),\
                    source_reference TEXT NOT NULL CHECK (length(trim(source_reference)) BETWEEN 1 AND 512),\
                    source_digest TEXT NOT NULL CHECK (length(source_digest) = 71 AND substr(source_digest, 1, 7) = 'sha256:' AND substr(source_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    dependency_lock_digest TEXT NOT NULL CHECK (length(dependency_lock_digest) = 71 AND substr(dependency_lock_digest, 1, 7) = 'sha256:' AND substr(dependency_lock_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    status TEXT NOT NULL CHECK (status IN ('requested', 'approved')),\
                    revision INTEGER NOT NULL CHECK (revision > 0),\
                    requested_by TEXT NOT NULL,\
                    requested_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    approved_by TEXT NULL,\
                    approved_at TEXT NULL,\
                    CHECK ((status = 'requested' AND approved_by IS NULL AND approved_at IS NULL) OR\
                           (status = 'approved' AND approved_by IS NOT NULL AND approved_at IS NOT NULL)),\
                    UNIQUE (release_id)\
                )",
                "CREATE INDEX module_static_promotions_status_idx ON module_static_promotions (status, requested_at, promotion_id)",
                "CREATE TABLE module_static_promotion_reviews (\
                    review_id TEXT PRIMARY KEY,\
                    promotion_id TEXT NOT NULL UNIQUE REFERENCES module_static_promotions(promotion_id) ON DELETE RESTRICT,\
                    ownership_evidence_reference TEXT NOT NULL CHECK (length(trim(ownership_evidence_reference)) BETWEEN 1 AND 512),\
                    ownership_evidence_digest TEXT NOT NULL CHECK (length(ownership_evidence_digest) = 71 AND substr(ownership_evidence_digest, 1, 7) = 'sha256:' AND substr(ownership_evidence_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    dependency_audit_reference TEXT NOT NULL CHECK (length(trim(dependency_audit_reference)) BETWEEN 1 AND 512),\
                    dependency_audit_digest TEXT NOT NULL CHECK (length(dependency_audit_digest) = 71 AND substr(dependency_audit_digest, 1, 7) = 'sha256:' AND substr(dependency_audit_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    test_evidence_reference TEXT NOT NULL CHECK (length(trim(test_evidence_reference)) BETWEEN 1 AND 512),\
                    test_evidence_digest TEXT NOT NULL CHECK (length(test_evidence_digest) = 71 AND substr(test_evidence_digest, 1, 7) = 'sha256:' AND substr(test_evidence_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    static_review_reference TEXT NOT NULL CHECK (length(trim(static_review_reference)) BETWEEN 1 AND 512),\
                    static_review_digest TEXT NOT NULL CHECK (length(static_review_digest) = 71 AND substr(static_review_digest, 1, 7) = 'sha256:' AND substr(static_review_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    approval_policy_revision TEXT NOT NULL CHECK (length(trim(approval_policy_revision)) BETWEEN 1 AND 128),\
                    review_digest TEXT NOT NULL CHECK (length(review_digest) = 71 AND substr(review_digest, 1, 7) = 'sha256:' AND substr(review_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    reviewed_by TEXT NOT NULL,\
                    reviewed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP\
                )",
                "CREATE TABLE module_static_promotion_operations (\
                    idempotency_key TEXT PRIMARY KEY,\
                    operation_kind TEXT NOT NULL CHECK (operation_kind IN ('request', 'approve')),\
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71 AND substr(request_digest, 1, 7) = 'sha256:' AND substr(request_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    actor_id TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0 AND length(trace_id) <= 512),\
                    correlation_id TEXT NOT NULL,\
                    promotion_id TEXT NULL REFERENCES module_static_promotions(promotion_id) ON DELETE RESTRICT,\
                    result_revision INTEGER NULL CHECK (result_revision IS NULL OR result_revision > 0),\
                    result_status TEXT NULL CHECK (result_status IS NULL OR result_status IN ('requested', 'approved')),\
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    completed_at TEXT NULL,\
                    CHECK ((promotion_id IS NULL AND result_revision IS NULL AND result_status IS NULL AND completed_at IS NULL) OR\
                           (promotion_id IS NOT NULL AND result_revision IS NOT NULL AND result_status IS NOT NULL AND completed_at IS NOT NULL))\
                )",
                "CREATE TABLE module_static_distribution_builds (\
                    distribution_build_id TEXT PRIMARY KEY,\
                    predecessor_build_id TEXT NULL REFERENCES module_static_distribution_builds(distribution_build_id) ON DELETE RESTRICT,\
                    composition_revision INTEGER NOT NULL UNIQUE CHECK (composition_revision > 0),\
                    composition_digest TEXT NOT NULL CHECK (length(composition_digest) = 71 AND substr(composition_digest, 1, 7) = 'sha256:' AND substr(composition_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    platform_source_reference TEXT NOT NULL CHECK (length(trim(platform_source_reference)) BETWEEN 1 AND 512),\
                    platform_source_digest TEXT NOT NULL CHECK (length(platform_source_digest) = 71 AND substr(platform_source_digest, 1, 7) = 'sha256:' AND substr(platform_source_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    toolchain_digest TEXT NOT NULL CHECK (length(toolchain_digest) = 71 AND substr(toolchain_digest, 1, 7) = 'sha256:' AND substr(toolchain_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    build_target TEXT NOT NULL CHECK (length(trim(build_target)) BETWEEN 1 AND 128),\
                    preparation_source TEXT NOT NULL DEFAULT 'built' CHECK (preparation_source IN ('built', 'signed_bootstrap')),\
                    bootstrap_receipt_digest TEXT NULL CHECK (bootstrap_receipt_digest IS NULL OR (length(bootstrap_receipt_digest) = 71 AND substr(bootstrap_receipt_digest, 1, 7) = 'sha256:' AND substr(bootstrap_receipt_digest, 8) NOT GLOB '*[^0-9a-f]*')),\
                    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'succeeded', 'failed', 'cancelled')),\
                    requested_by TEXT NOT NULL,\
                    requested_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),\
                    active_claim_id TEXT NULL,\
                    claimed_by TEXT NULL,\
                    lease_expires_at TEXT NULL,\
                    last_heartbeat_at TEXT NULL,\
                    bundle_reference TEXT NULL,\
                    bundle_root_digest TEXT NULL CHECK (bundle_root_digest IS NULL OR (length(bundle_root_digest) = 71 AND substr(bundle_root_digest, 1, 7) = 'sha256:' AND substr(bundle_root_digest, 8) NOT GLOB '*[^0-9a-f]*')),\
                    role_set_digest TEXT NULL CHECK (role_set_digest IS NULL OR (length(role_set_digest) = 71 AND substr(role_set_digest, 1, 7) = 'sha256:' AND substr(role_set_digest, 8) NOT GLOB '*[^0-9a-f]*')),\
                    role_artifacts_json TEXT NULL,\
                    sbom_reference TEXT NULL,\
                    sbom_digest TEXT NULL CHECK (sbom_digest IS NULL OR (length(sbom_digest) = 71 AND substr(sbom_digest, 1, 7) = 'sha256:' AND substr(sbom_digest, 8) NOT GLOB '*[^0-9a-f]*')),\
                    provenance_reference TEXT NULL,\
                    provenance_digest TEXT NULL CHECK (provenance_digest IS NULL OR (length(provenance_digest) = 71 AND substr(provenance_digest, 1, 7) = 'sha256:' AND substr(provenance_digest, 8) NOT GLOB '*[^0-9a-f]*')),\
                    signature_reference TEXT NULL,\
                    signature_digest TEXT NULL CHECK (signature_digest IS NULL OR (length(signature_digest) = 71 AND substr(signature_digest, 1, 7) = 'sha256:' AND substr(signature_digest, 8) NOT GLOB '*[^0-9a-f]*')),\
                    test_evidence_reference TEXT NULL,\
                    test_evidence_digest TEXT NULL CHECK (test_evidence_digest IS NULL OR (length(test_evidence_digest) = 71 AND substr(test_evidence_digest, 1, 7) = 'sha256:' AND substr(test_evidence_digest, 8) NOT GLOB '*[^0-9a-f]*')),\
                    failure_code TEXT NULL,\
                    failure_detail TEXT NULL,\
                    completion_digest TEXT NULL CHECK (completion_digest IS NULL OR (length(completion_digest) = 71 AND substr(completion_digest, 1, 7) = 'sha256:' AND substr(completion_digest, 8) NOT GLOB '*[^0-9a-f]*')),\
                    started_at TEXT NULL,\
                    completed_at TEXT NULL,\
                    CHECK (\
                        (status = 'queued' AND active_claim_id IS NULL AND claimed_by IS NULL AND lease_expires_at IS NULL AND bundle_reference IS NULL AND bundle_root_digest IS NULL AND role_set_digest IS NULL AND role_artifacts_json IS NULL AND sbom_reference IS NULL AND sbom_digest IS NULL AND provenance_reference IS NULL AND provenance_digest IS NULL AND signature_reference IS NULL AND signature_digest IS NULL AND test_evidence_reference IS NULL AND test_evidence_digest IS NULL AND failure_code IS NULL AND failure_detail IS NULL AND completion_digest IS NULL AND completed_at IS NULL)\
                        OR (status = 'running' AND active_claim_id IS NOT NULL AND claimed_by IS NOT NULL AND lease_expires_at IS NOT NULL AND started_at IS NOT NULL AND bundle_reference IS NULL AND bundle_root_digest IS NULL AND role_set_digest IS NULL AND role_artifacts_json IS NULL AND sbom_reference IS NULL AND sbom_digest IS NULL AND provenance_reference IS NULL AND provenance_digest IS NULL AND signature_reference IS NULL AND signature_digest IS NULL AND test_evidence_reference IS NULL AND test_evidence_digest IS NULL AND failure_code IS NULL AND failure_detail IS NULL AND completion_digest IS NULL AND completed_at IS NULL)\
                        OR (status = 'succeeded' AND lease_expires_at IS NULL AND bundle_reference IS NOT NULL AND bundle_root_digest IS NOT NULL AND role_set_digest IS NOT NULL AND role_artifacts_json IS NOT NULL AND sbom_reference IS NOT NULL AND sbom_digest IS NOT NULL AND provenance_reference IS NOT NULL AND provenance_digest IS NOT NULL AND signature_reference IS NOT NULL AND signature_digest IS NOT NULL AND test_evidence_reference IS NOT NULL AND test_evidence_digest IS NOT NULL AND failure_code IS NULL AND failure_detail IS NULL AND completion_digest IS NOT NULL AND completed_at IS NOT NULL AND ((preparation_source = 'built' AND active_claim_id IS NOT NULL AND claimed_by IS NOT NULL AND attempt_count > 0 AND bootstrap_receipt_digest IS NULL) OR (preparation_source = 'signed_bootstrap' AND active_claim_id IS NULL AND claimed_by IS NULL AND attempt_count = 0 AND bootstrap_receipt_digest IS NOT NULL)))\
                        OR (status IN ('failed', 'cancelled') AND active_claim_id IS NOT NULL AND claimed_by IS NOT NULL AND lease_expires_at IS NULL AND bundle_reference IS NULL AND bundle_root_digest IS NULL AND role_set_digest IS NULL AND role_artifacts_json IS NULL AND sbom_reference IS NULL AND sbom_digest IS NULL AND provenance_reference IS NULL AND provenance_digest IS NULL AND signature_reference IS NULL AND signature_digest IS NULL AND test_evidence_reference IS NULL AND test_evidence_digest IS NULL AND failure_code IS NOT NULL AND failure_detail IS NOT NULL AND completion_digest IS NOT NULL AND completed_at IS NOT NULL)\
                    )\
                )",
                "CREATE INDEX module_static_distribution_builds_queue_idx ON module_static_distribution_builds (status, requested_at, distribution_build_id)",
                "CREATE TABLE module_static_distribution_attempts (\
                    claim_id TEXT PRIMARY KEY,\
                    distribution_build_id TEXT NOT NULL REFERENCES module_static_distribution_builds(distribution_build_id) ON DELETE RESTRICT,\
                    attempt_number INTEGER NOT NULL CHECK (attempt_number > 0),\
                    runner_id TEXT NOT NULL CHECK (length(trim(runner_id)) BETWEEN 1 AND 128),\
                    status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed', 'cancelled', 'lease_expired')),\
                    lease_expires_at TEXT NOT NULL,\
                    last_heartbeat_at TEXT NOT NULL,\
                    bundle_reference TEXT NULL,\
                    bundle_root_digest TEXT NULL CHECK (bundle_root_digest IS NULL OR (length(bundle_root_digest) = 71 AND substr(bundle_root_digest, 1, 7) = 'sha256:' AND substr(bundle_root_digest, 8) NOT GLOB '*[^0-9a-f]*')),\
                    role_set_digest TEXT NULL CHECK (role_set_digest IS NULL OR (length(role_set_digest) = 71 AND substr(role_set_digest, 1, 7) = 'sha256:' AND substr(role_set_digest, 8) NOT GLOB '*[^0-9a-f]*')),\
                    role_artifacts_json TEXT NULL,\
                    sbom_reference TEXT NULL,\
                    sbom_digest TEXT NULL CHECK (sbom_digest IS NULL OR (length(sbom_digest) = 71 AND substr(sbom_digest, 1, 7) = 'sha256:' AND substr(sbom_digest, 8) NOT GLOB '*[^0-9a-f]*')),\
                    provenance_reference TEXT NULL,\
                    provenance_digest TEXT NULL CHECK (provenance_digest IS NULL OR (length(provenance_digest) = 71 AND substr(provenance_digest, 1, 7) = 'sha256:' AND substr(provenance_digest, 8) NOT GLOB '*[^0-9a-f]*')),\
                    signature_reference TEXT NULL,\
                    signature_digest TEXT NULL CHECK (signature_digest IS NULL OR (length(signature_digest) = 71 AND substr(signature_digest, 1, 7) = 'sha256:' AND substr(signature_digest, 8) NOT GLOB '*[^0-9a-f]*')),\
                    test_evidence_reference TEXT NULL,\
                    test_evidence_digest TEXT NULL CHECK (test_evidence_digest IS NULL OR (length(test_evidence_digest) = 71 AND substr(test_evidence_digest, 1, 7) = 'sha256:' AND substr(test_evidence_digest, 8) NOT GLOB '*[^0-9a-f]*')),\
                    failure_code TEXT NULL,\
                    failure_detail TEXT NULL,\
                    completion_digest TEXT NULL CHECK (completion_digest IS NULL OR (length(completion_digest) = 71 AND substr(completion_digest, 1, 7) = 'sha256:' AND substr(completion_digest, 8) NOT GLOB '*[^0-9a-f]*')),\
                    started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    completed_at TEXT NULL,\
                    UNIQUE (distribution_build_id, attempt_number),\
                    CHECK (\
                        (status = 'running' AND bundle_reference IS NULL AND bundle_root_digest IS NULL AND role_set_digest IS NULL AND role_artifacts_json IS NULL AND sbom_reference IS NULL AND sbom_digest IS NULL AND provenance_reference IS NULL AND provenance_digest IS NULL AND signature_reference IS NULL AND signature_digest IS NULL AND test_evidence_reference IS NULL AND test_evidence_digest IS NULL AND failure_code IS NULL AND failure_detail IS NULL AND completion_digest IS NULL AND completed_at IS NULL)\
                        OR (status = 'lease_expired' AND bundle_reference IS NULL AND bundle_root_digest IS NULL AND role_set_digest IS NULL AND role_artifacts_json IS NULL AND sbom_reference IS NULL AND sbom_digest IS NULL AND provenance_reference IS NULL AND provenance_digest IS NULL AND signature_reference IS NULL AND signature_digest IS NULL AND test_evidence_reference IS NULL AND test_evidence_digest IS NULL AND failure_code IS NULL AND failure_detail IS NULL AND completion_digest IS NULL AND completed_at IS NOT NULL)\
                        OR (status = 'succeeded' AND bundle_reference IS NOT NULL AND bundle_root_digest IS NOT NULL AND role_set_digest IS NOT NULL AND role_artifacts_json IS NOT NULL AND sbom_reference IS NOT NULL AND sbom_digest IS NOT NULL AND provenance_reference IS NOT NULL AND provenance_digest IS NOT NULL AND signature_reference IS NOT NULL AND signature_digest IS NOT NULL AND test_evidence_reference IS NOT NULL AND test_evidence_digest IS NOT NULL AND failure_code IS NULL AND failure_detail IS NULL AND completion_digest IS NOT NULL AND completed_at IS NOT NULL)\
                        OR (status IN ('failed', 'cancelled') AND bundle_reference IS NULL AND bundle_root_digest IS NULL AND role_set_digest IS NULL AND role_artifacts_json IS NULL AND sbom_reference IS NULL AND sbom_digest IS NULL AND provenance_reference IS NULL AND provenance_digest IS NULL AND signature_reference IS NULL AND signature_digest IS NULL AND test_evidence_reference IS NULL AND test_evidence_digest IS NULL AND failure_code IS NOT NULL AND failure_detail IS NOT NULL AND completion_digest IS NOT NULL AND completed_at IS NOT NULL)\
                    )\
                )",
                "CREATE TABLE module_static_distribution_state (\
                    state_id TEXT PRIMARY KEY CHECK (state_id = 'current'),\
                    revision INTEGER NOT NULL CHECK (revision >= 0),\
                    current_build_id TEXT NULL REFERENCES module_static_distribution_builds(distribution_build_id) ON DELETE RESTRICT,\
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP\
                )",
                "INSERT INTO module_static_distribution_state (state_id, revision, current_build_id)\
                 VALUES ('current', 0, NULL)",
                "CREATE TABLE module_static_distribution_items (\
                    distribution_build_id TEXT NOT NULL REFERENCES module_static_distribution_builds(distribution_build_id) ON DELETE RESTRICT,\
                    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),\
                    promotion_id TEXT NOT NULL,\
                    promotion_revision INTEGER NOT NULL CHECK (promotion_revision > 0),\
                    release_id TEXT NOT NULL,\
                    module_slug TEXT NOT NULL CHECK (length(trim(module_slug)) BETWEEN 1 AND 128),\
                    module_version TEXT NOT NULL CHECK (length(trim(module_version)) BETWEEN 1 AND 128),\
                    cargo_package TEXT NOT NULL CHECK (length(trim(cargo_package)) BETWEEN 1 AND 128),\
                    entry_type TEXT NOT NULL CHECK (length(trim(entry_type)) BETWEEN 1 AND 256),\
                    source_reference TEXT NOT NULL CHECK (length(trim(source_reference)) BETWEEN 1 AND 512),\
                    source_digest TEXT NOT NULL CHECK (length(source_digest) = 71 AND substr(source_digest, 1, 7) = 'sha256:' AND substr(source_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    dependency_lock_digest TEXT NOT NULL CHECK (length(dependency_lock_digest) = 71 AND substr(dependency_lock_digest, 1, 7) = 'sha256:' AND substr(dependency_lock_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    executor_mode TEXT NOT NULL CHECK (executor_mode = 'static_native'),\
                    PRIMARY KEY (distribution_build_id, ordinal),\
                    UNIQUE (distribution_build_id, promotion_id),\
                    UNIQUE (distribution_build_id, module_slug)\
                )",
                "CREATE TABLE module_static_distribution_operations (\
                    idempotency_key TEXT PRIMARY KEY,\
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71 AND substr(request_digest, 1, 7) = 'sha256:' AND substr(request_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    actor_id TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    correlation_id TEXT NOT NULL,\
                    distribution_build_id TEXT NULL REFERENCES module_static_distribution_builds(distribution_build_id) ON DELETE RESTRICT,\
                    composition_revision INTEGER NULL CHECK (composition_revision IS NULL OR composition_revision > 0),\
                    composition_digest TEXT NULL CHECK (composition_digest IS NULL OR (length(composition_digest) = 71 AND substr(composition_digest, 1, 7) = 'sha256:' AND substr(composition_digest, 8) NOT GLOB '*[^0-9a-f]*')),\
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    completed_at TEXT NULL,\
                    CHECK ((distribution_build_id IS NULL AND composition_revision IS NULL AND composition_digest IS NULL AND completed_at IS NULL) OR\
                           (distribution_build_id IS NOT NULL AND composition_revision IS NOT NULL AND composition_digest IS NOT NULL AND completed_at IS NOT NULL))\
                )",
                "CREATE TABLE module_static_distribution_releases (\
                    distribution_release_id TEXT PRIMARY KEY,\
                    distribution_build_id TEXT NOT NULL UNIQUE REFERENCES module_static_distribution_builds(distribution_build_id) ON DELETE RESTRICT,\
                    predecessor_release_id TEXT NULL REFERENCES module_static_distribution_releases(distribution_release_id) ON DELETE RESTRICT,\
                    release_revision INTEGER NOT NULL UNIQUE CHECK (release_revision > 0),\
                    composition_revision INTEGER NOT NULL CHECK (composition_revision > 0),\
                    composition_digest TEXT NOT NULL CHECK (length(composition_digest) = 71 AND substr(composition_digest, 1, 7) = 'sha256:' AND substr(composition_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    bundle_reference TEXT NOT NULL CHECK (length(trim(bundle_reference)) BETWEEN 1 AND 512),\
                    bundle_root_digest TEXT NOT NULL CHECK (length(bundle_root_digest) = 71 AND substr(bundle_root_digest, 1, 7) = 'sha256:' AND substr(bundle_root_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    role_set_digest TEXT NOT NULL CHECK (length(role_set_digest) = 71 AND substr(role_set_digest, 1, 7) = 'sha256:' AND substr(role_set_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    role_artifacts_json TEXT NOT NULL,\
                    sbom_reference TEXT NOT NULL CHECK (length(trim(sbom_reference)) BETWEEN 1 AND 512),\
                    sbom_digest TEXT NOT NULL CHECK (length(sbom_digest) = 71 AND substr(sbom_digest, 1, 7) = 'sha256:' AND substr(sbom_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    provenance_reference TEXT NOT NULL CHECK (length(trim(provenance_reference)) BETWEEN 1 AND 512),\
                    provenance_digest TEXT NOT NULL CHECK (length(provenance_digest) = 71 AND substr(provenance_digest, 1, 7) = 'sha256:' AND substr(provenance_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    signature_reference TEXT NOT NULL CHECK (length(trim(signature_reference)) BETWEEN 1 AND 512),\
                    signature_digest TEXT NOT NULL CHECK (length(signature_digest) = 71 AND substr(signature_digest, 1, 7) = 'sha256:' AND substr(signature_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    test_evidence_reference TEXT NOT NULL CHECK (length(trim(test_evidence_reference)) BETWEEN 1 AND 512),\
                    test_evidence_digest TEXT NOT NULL CHECK (length(test_evidence_digest) = 71 AND substr(test_evidence_digest, 1, 7) = 'sha256:' AND substr(test_evidence_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    status TEXT NOT NULL CHECK (status IN ('admitted', 'active', 'superseded', 'revoked')),\
                    admitted_by TEXT NOT NULL,\
                    admitted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    activated_at TEXT NULL,\
                    superseded_at TEXT NULL,\
                    revoked_by TEXT NULL,\
                    revoked_at TEXT NULL,\
                    revocation_reason TEXT NULL CHECK (revocation_reason IS NULL OR length(trim(revocation_reason)) BETWEEN 1 AND 1024),\
                    revocation_policy_revision TEXT NULL CHECK (revocation_policy_revision IS NULL OR length(trim(revocation_policy_revision)) BETWEEN 1 AND 128),\
                    CHECK ((status = 'admitted' AND activated_at IS NULL AND superseded_at IS NULL AND revoked_by IS NULL AND revoked_at IS NULL AND revocation_reason IS NULL AND revocation_policy_revision IS NULL) OR\
                           (status = 'active' AND activated_at IS NOT NULL AND superseded_at IS NULL AND revoked_by IS NULL AND revoked_at IS NULL AND revocation_reason IS NULL AND revocation_policy_revision IS NULL) OR\
                           (status = 'superseded' AND activated_at IS NOT NULL AND superseded_at IS NOT NULL AND revoked_by IS NULL AND revoked_at IS NULL AND revocation_reason IS NULL AND revocation_policy_revision IS NULL) OR\
                           (status = 'revoked' AND revoked_by IS NOT NULL AND revoked_at IS NOT NULL AND revocation_reason IS NOT NULL AND revocation_policy_revision IS NOT NULL))\
                )",
                "CREATE UNIQUE INDEX module_static_distribution_releases_active_idx ON module_static_distribution_releases (status) WHERE status = 'active'",
                "CREATE TABLE module_static_distribution_release_state (\
                    state_id TEXT PRIMARY KEY CHECK (state_id = 'current'),\
                    revision INTEGER NOT NULL CHECK (revision >= 0),\
                    active_release_id TEXT NULL REFERENCES module_static_distribution_releases(distribution_release_id) ON DELETE RESTRICT,\
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP\
                )",
                "INSERT INTO module_static_distribution_release_state (state_id, revision, active_release_id) VALUES ('current', 0, NULL)",
                "CREATE TABLE module_static_distribution_release_admissions (\
                    admission_id TEXT PRIMARY KEY,\
                    distribution_release_id TEXT NOT NULL UNIQUE REFERENCES module_static_distribution_releases(distribution_release_id) ON DELETE RESTRICT,\
                    verifier_identity TEXT NOT NULL CHECK (length(trim(verifier_identity)) BETWEEN 1 AND 256),\
                    policy_revision TEXT NOT NULL CHECK (length(trim(policy_revision)) BETWEEN 1 AND 128),\
                    evidence_reference TEXT NOT NULL CHECK (length(trim(evidence_reference)) BETWEEN 1 AND 512),\
                    evidence_digest TEXT NOT NULL CHECK (length(evidence_digest) = 71 AND substr(evidence_digest, 1, 7) = 'sha256:' AND substr(evidence_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    signature_verified INTEGER NOT NULL CHECK (signature_verified = 1),\
                    provenance_verified INTEGER NOT NULL CHECK (provenance_verified = 1),\
                    sbom_verified INTEGER NOT NULL CHECK (sbom_verified = 1),\
                    test_evidence_verified INTEGER NOT NULL CHECK (test_evidence_verified = 1),\
                    dependency_policy_verified INTEGER NOT NULL CHECK (dependency_policy_verified = 1),\
                    verified_at TEXT NOT NULL\
                )",
                "CREATE TABLE module_static_distribution_admission_operations (\
                    idempotency_key TEXT PRIMARY KEY,\
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71 AND substr(request_digest, 1, 7) = 'sha256:' AND substr(request_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    actor_id TEXT NOT NULL,\
                    distribution_release_id TEXT NULL REFERENCES module_static_distribution_releases(distribution_release_id) ON DELETE RESTRICT,\
                    release_revision INTEGER NULL CHECK (release_revision IS NULL OR release_revision > 0),\
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    completed_at TEXT NULL,\
                    CHECK ((distribution_release_id IS NULL AND release_revision IS NULL AND completed_at IS NULL) OR\
                           (distribution_release_id IS NOT NULL AND release_revision IS NOT NULL AND completed_at IS NOT NULL))\
                )",
                "CREATE TABLE module_static_distribution_revocation_operations (\
                    idempotency_key TEXT PRIMARY KEY,\
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71 AND substr(request_digest, 1, 7) = 'sha256:' AND substr(request_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    actor_id TEXT NOT NULL,\
                    distribution_release_id TEXT NULL REFERENCES module_static_distribution_releases(distribution_release_id) ON DELETE RESTRICT,\
                    release_state_revision INTEGER NULL CHECK (release_state_revision IS NULL OR release_state_revision > 0),\
                    was_active INTEGER NULL CHECK (was_active IS NULL OR was_active IN (0, 1)),\
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                    completed_at TEXT NULL,\
                    CHECK ((distribution_release_id IS NULL AND release_state_revision IS NULL AND was_active IS NULL AND completed_at IS NULL) OR\
                           (distribution_release_id IS NOT NULL AND release_state_revision IS NOT NULL AND was_active IS NOT NULL AND completed_at IS NOT NULL))\
                )",
                "CREATE TABLE module_static_distribution_release_idempotency_keys (\
                    idempotency_key TEXT PRIMARY KEY,\
                    operation_kind TEXT NOT NULL CHECK (operation_kind IN ('bootstrap_import', 'admit', 'revoke')),\
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71 AND substr(request_digest, 1, 7) = 'sha256:' AND substr(request_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    actor_id TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0 AND length(trace_id) <= 512),\
                    correlation_id TEXT NOT NULL,\
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP\
                )",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "static promotion migration does not support database backend {backend:?}"
                )));
            }
        };
        for statement in statements {
            manager
                .get_connection()
                .execute(Statement::from_string(
                    manager.get_database_backend(),
                    (*statement).to_string(),
                ))
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in [
            "module_static_distribution_revocation_operations",
            "module_static_distribution_admission_operations",
            "module_static_distribution_release_idempotency_keys",
            "module_static_distribution_release_admissions",
            "module_static_distribution_release_state",
            "module_static_distribution_releases",
            "module_static_distribution_operations",
            "module_static_distribution_items",
            "module_static_distribution_state",
            "module_static_distribution_attempts",
            "module_static_distribution_builds",
            "module_static_promotion_operations",
            "module_static_promotion_reviews",
            "module_static_promotions",
        ] {
            manager
                .get_connection()
                .execute_unprepared(&format!("DROP TABLE {table}"))
                .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

    use super::*;

    #[tokio::test]
    async fn sqlite_schema_separates_admission_from_serving_activation() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("sqlite database");
        db.execute_unprepared("PRAGMA foreign_keys = ON")
            .await
            .expect("foreign keys");
        Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect("static promotion migration");

        let release_columns = column_names(&db, "module_static_distribution_releases").await;
        assert!(release_columns.contains("admitted_by"));
        assert!(release_columns.contains("admitted_at"));
        assert!(release_columns.contains("activated_at"));
        assert!(!release_columns.contains("activated_by"));

        let tables = db
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT name FROM sqlite_master WHERE type = 'table'".to_string(),
            ))
            .await
            .expect("table catalog")
            .into_iter()
            .map(|row| row.try_get::<String>("", "name").expect("table name"))
            .collect::<HashSet<_>>();
        assert!(tables.contains("module_static_distribution_admission_operations"));
        assert!(!tables.contains("module_static_distribution_release_operations"));
        assert!(!tables.contains("module_static_distribution_rollback_requests"));
        assert!(!tables.contains("module_static_distribution_rollback_operations"));

        let idempotency_columns =
            column_names(&db, "module_static_distribution_release_idempotency_keys").await;
        assert!(idempotency_columns.contains("trace_id"));
        assert!(idempotency_columns.contains("correlation_id"));
    }

    async fn column_names(db: &sea_orm::DatabaseConnection, table: &str) -> HashSet<String> {
        db.query_all(Statement::from_string(
            DbBackend::Sqlite,
            format!("PRAGMA table_info({table})"),
        ))
        .await
        .expect("table info")
        .into_iter()
        .map(|row| row.try_get("", "name").expect("column name"))
        .collect()
    }
}
