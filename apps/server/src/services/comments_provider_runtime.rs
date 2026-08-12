mod base {
    include!("comments_provider_runtime_base.rs");
}

mod keyring {
    include!("comments_provider_runtime_keyring.rs");
}

mod keyring_reload {
    include!("comments_provider_runtime_keyring_reload.rs");
}

mod keyring_reload_guard {
    include!("comments_provider_runtime_keyring_reload_guard.rs");
}

mod keyring_schedule {
    include!("comments_provider_runtime_keyring_schedule.rs");
}

mod keyring_schedule_guard {
    include!("comments_provider_runtime_keyring_schedule_guard.rs");
}

mod keyring_schedule_trigger {
    include!("comments_provider_runtime_keyring_schedule_trigger.rs");
}

mod keyring_schedule_persistence {
    include!("comments_provider_runtime_keyring_schedule_persistence.rs");
}

mod keyring_schedule_persistence_postgres {
    include!("comments_provider_runtime_keyring_schedule_persistence_postgres.rs");
}

mod keyring_schedule_persistence_postgres_audit {
    include!("comments_provider_runtime_keyring_schedule_persistence_postgres_audit.rs");
}

mod keyring_schedule_audit_publication {
    include!("comments_provider_runtime_keyring_schedule_audit_publication.rs");
}

mod keyring_schedule_audit_canonical_writer {
    include!("comments_provider_runtime_keyring_schedule_audit_canonical_writer.rs");
}

mod keyring_schedule_audit_handoff_postgres {
    include!("comments_provider_runtime_keyring_schedule_audit_handoff_postgres.rs");
    include!("comments_provider_runtime_keyring_schedule_audit_handoff_retry_ready.rs");
    include!("comments_provider_runtime_keyring_schedule_audit_handoff_postgres_test_support.rs");
}

mod keyring_schedule_audit_handoff_worker {
    include!("comments_provider_runtime_keyring_schedule_audit_handoff_worker.rs");
    include!("comments_provider_runtime_keyring_schedule_audit_handoff_worker_source_retry.rs");
}

mod keyring_schedule_audit_source_retry_postgres {
    include!("comments_provider_runtime_keyring_schedule_audit_source_retry_postgres.rs");
    include!("comments_provider_runtime_keyring_schedule_audit_source_retry_active.rs");
}

mod keyring_schedule_audit_recovery_postgres {
    include!("comments_provider_runtime_keyring_schedule_audit_recovery_postgres.rs");
    include!("comments_provider_runtime_keyring_schedule_audit_recovery_postgres_test_support.rs");
}

mod keyring_schedule_audit_operator {
    include!("comments_provider_runtime_keyring_schedule_audit_operator.rs");
    include!("comments_provider_runtime_keyring_schedule_audit_operator_postgres_evidence.rs");
    include!(
        "comments_provider_runtime_keyring_schedule_audit_restart_ambiguity_postgres_evidence.rs"
    );
}

mod keyring_schedule_persisted_trigger {
    include!("comments_provider_runtime_keyring_schedule_persisted_trigger.rs");
}

mod keyring_schedule_postgres_audited_trigger {
    include!("comments_provider_runtime_keyring_schedule_postgres_audited_trigger.rs");
}

mod keyring_schedule_trigger_guard {
    include!("comments_provider_runtime_keyring_schedule_trigger_guard.rs");
}

pub use base::{
    COMMENTS_PROVIDER_MODE_ENV, COMMENTS_TCP_BEARER_TOKEN_ENV, COMMENTS_TCP_BIND_ENV,
    COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY_ENV, COMMENTS_TCP_DELEGATION_SECRET_ENV,
    COMMENTS_TCP_DELEGATION_TTL_MS_ENV, COMMENTS_TCP_ENDPOINT_ENV,
    COMMENTS_TCP_LISTENER_ENABLED_ENV, COMMENTS_TCP_MAX_CONNECTIONS_ENV,
    COMMENTS_TCP_MAX_FRAME_BYTES_ENV, COMMENTS_TCP_PRE_REQUEST_TIMEOUT_MS_ENV,
    COMMENTS_TCP_SERVICE_ACTOR_ID_ENV, COMMENTS_TCP_SHUTDOWN_GRACE_MS_ENV, CommentsProviderProfile,
    CommentsProviderRuntimeSelection, CommentsTcpListenerConfig, CommentsTcpListenerHandle,
    SharedCommentsTcpAuthorityResolver, SharedCommentsTcpClientChannelConnector,
    SharedCommentsTcpServerChannelAcceptor, SharedCommentsTcpServerProvider,
};
pub use keyring::{
    COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV, CommentsTcpDelegationKeyringRuntimeSelection,
    CommentsTcpDelegationKeyringSource, MAX_COMMENTS_TCP_DELEGATION_KEYRING_FILE_BYTES,
    SharedCommentsTcpDelegationKeyringSnapshot,
};
pub use keyring_reload::{
    COMMENTS_TCP_DELEGATION_RELOAD_ENABLED_ENV, CommentsTcpDelegationKeyringReloadOutcome,
    CommentsTcpDelegationKeyringReloadStatus, SharedCommentsTcpDelegationKeyringReloadHandle,
};
pub use keyring_schedule::{
    COMMENTS_TCP_DELEGATION_SCHEDULE_ENABLED_ENV, CommentsTcpDelegationScheduleReloadOutcome,
    CommentsTcpDelegationScheduleReloadStatus, CommentsTcpDelegationScheduleRuntimeSelection,
    SharedCommentsTcpDelegationScheduleHandle,
};
pub use keyring_schedule_audit_canonical_writer::RustokOutboxCommentsTcpDelegationScheduleAuditCanonicalWriter;
pub use keyring_schedule_audit_handoff_postgres::{
    COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_MAX_CLAIM_SECONDS,
    CommentsTcpDelegationScheduleAuditHandoffClaim, CommentsTcpDelegationScheduleAuditHandoffError,
    PostgresCommentsTcpDelegationScheduleAuditCanonicalHandoff,
};
pub use keyring_schedule_audit_handoff_worker::{
    COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_CLAIM_TTL_SECONDS_ENV,
    COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_CONTROL_PLANE_TENANT_ID_ENV,
    COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_ENABLED_ENV,
    COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_IDLE_POLL_MS_ENV,
    COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_MAX_CLAIMS_PER_CYCLE_ENV,
    COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_RETRY_DELAY_MS_ENV,
    COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_SOURCE_MAX_ATTEMPTS_ENV,
    COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_SOURCE_RETRY_DELAY_SECONDS_ENV,
    CommentsTcpDelegationScheduleAuditHandoffWorkerConfig,
    CommentsTcpDelegationScheduleAuditHandoffWorkerHandle,
    CommentsTcpDelegationScheduleAuditSourceRetryWorkerConfig,
    start_comments_tcp_delegation_schedule_audit_handoff_worker_with_source_retry_if_enabled,
};
// Historical slice-93 source-guard marker retained after operator composition:
// start_comments_tcp_delegation_schedule_audit_handoff_worker_with_source_retry_if_enabled as start_comments_tcp_delegation_schedule_audit_handoff_worker_if_enabled
pub use keyring_schedule_audit_operator::{
    CommentsTcpDelegationScheduleAuditOperatorContext,
    CommentsTcpDelegationScheduleAuditOperatorError,
    CommentsTcpDelegationScheduleAuditOperatorRuntime,
    materialize_comments_tcp_delegation_schedule_audit_operator,
    start_comments_tcp_delegation_schedule_audit_handoff_worker_with_operator_if_enabled as start_comments_tcp_delegation_schedule_audit_handoff_worker_if_enabled,
};
pub use keyring_schedule_audit_publication::{
    COMMENTS_TCP_DELEGATION_SCHEDULE_CANONICAL_AUDIT_EVENT_TYPE,
    COMMENTS_TCP_DELEGATION_SCHEDULE_CANONICAL_AUDIT_SCHEMA_VERSION,
    COMMENTS_TCP_DELEGATION_SCHEDULE_CANONICAL_AUDIT_STATE_KEY,
    CommentsTcpDelegationScheduleAuditCanonicalPublication,
    CommentsTcpDelegationScheduleAuditCanonicalWriteError,
    CommentsTcpDelegationScheduleAuditCanonicalWriter,
    SharedCommentsTcpDelegationScheduleAuditCanonicalWriter,
};
pub use keyring_schedule_audit_recovery_postgres::{
    COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_RECOVERY_ACTION,
    COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_RECOVERY_MAX_REASON_BYTES,
    COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_RECOVERY_TABLE,
    CommentsTcpDelegationScheduleAuditRecoveryError,
    CommentsTcpDelegationScheduleAuditRecoveryInspection,
    CommentsTcpDelegationScheduleAuditRecoveryOutcome,
    CommentsTcpDelegationScheduleAuditRecoveryRequest,
    PostgresCommentsTcpDelegationScheduleAuditRecoveryStore,
};
pub use keyring_schedule_audit_source_retry_postgres::{
    COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_SOURCE_MAX_ATTEMPTS,
    COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_SOURCE_MAX_RETRY_DELAY_SECONDS,
    CommentsTcpDelegationScheduleAuditSourceDeadLetterInspection,
    CommentsTcpDelegationScheduleAuditSourceFailureCode,
    CommentsTcpDelegationScheduleAuditSourceFailureTransition,
    CommentsTcpDelegationScheduleAuditSourceRetryPolicyError,
    PostgresCommentsTcpDelegationScheduleAuditSourceRetryPolicy,
};
pub use keyring_schedule_persisted_trigger::{
    CommentsTcpDelegationPersistedScheduleAuditOutcome,
    CommentsTcpDelegationPersistedScheduleAuditRecord,
    SharedCommentsTcpDelegationPersistedScheduleTrigger,
};
pub use keyring_schedule_persistence::{
    COMMENTS_TCP_DELEGATION_SCHEDULE_PERSISTENCE_SCHEMA_VERSION,
    CommentsTcpDelegationScheduleDigest, CommentsTcpDelegationSchedulePersistenceDocument,
    CommentsTcpDelegationSchedulePersistenceKey, CommentsTcpDelegationSchedulePersistenceRecord,
    CommentsTcpDelegationSchedulePersistenceStartupMode,
    CommentsTcpDelegationSchedulePersistenceStore,
    CommentsTcpDelegationSchedulePersistenceStoreError,
    SharedCommentsTcpDelegationSchedulePersistenceStore,
};
pub use keyring_schedule_persistence_postgres::{
    COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_STATE_KEY,
    COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_TABLE,
    PostgresCommentsTcpDelegationSchedulePersistenceStore,
};
pub use keyring_schedule_persistence_postgres_audit::{
    COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_EVENT_TYPE,
    COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_OUTBOX_TABLE,
    COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_SCHEMA_VERSION,
    PostgresCommentsTcpDelegationScheduleAuditedPersistenceStore,
};
pub use keyring_schedule_postgres_audited_trigger::SharedCommentsTcpDelegationPostgresAuditedScheduleTrigger;
pub use keyring_schedule_trigger::{
    CommentsTcpDelegationScheduleTriggerAuditOutcome,
    CommentsTcpDelegationScheduleTriggerAuditRecord,
    CommentsTcpDelegationScheduleTriggerAuthorizationError,
    CommentsTcpDelegationScheduleTriggerAuthorizationRequest,
    CommentsTcpDelegationScheduleTriggerAuthorizer, CommentsTcpDelegationScheduleTriggerContext,
    CommentsTcpDelegationScheduleTriggerOperation,
    DEFAULT_COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_CAPACITY,
    MAX_COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_CAPACITY,
    SharedCommentsTcpDelegationScheduleTrigger,
    SharedCommentsTcpDelegationScheduleTriggerAuthorizer,
};

use rustok_core::ModuleRuntimeExtensions;

use crate::error::Result;
use crate::services::server_runtime_context::ServerRuntimeContext;

pub fn register_comments_provider_runtime(
    extensions: &mut ModuleRuntimeExtensions,
) -> std::result::Result<(), String> {
    // Historical source-verifier markers retained for static, ordinary reload,
    // scheduled, and authorized trigger composition paths:
    // keyring::register_comments_provider_runtime(extensions)
    // keyring_reload::register_comments_provider_runtime(extensions)
    // keyring_schedule_guard::register_comments_provider_runtime(extensions)
    keyring_schedule_trigger_guard::register_comments_provider_runtime(extensions)
}

pub async fn start_comments_tcp_listener_if_enabled(
    runtime_ctx: &ServerRuntimeContext,
) -> Result<()> {
    // Historical source-verifier markers retained for earlier listener paths:
    // keyring::start_comments_tcp_listener_if_enabled(runtime_ctx).await
    // keyring_reload::start_comments_tcp_listener_if_enabled(runtime_ctx).await
    // keyring_reload_guard::start_comments_tcp_listener_if_enabled(runtime_ctx).await
    // keyring_schedule_guard::start_comments_tcp_listener_if_enabled(runtime_ctx).await
    keyring_schedule_trigger_guard::start_comments_tcp_listener_if_enabled(runtime_ctx).await
}
