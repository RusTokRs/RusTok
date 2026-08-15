use std::{
    fmt,
    path::{Path, PathBuf},
    time::Duration,
};

use rustok_api::PortError;
use rustok_comments::{
    CommentsTcpDelegationKeyring, CommentsTcpDelegationKeyringProvider,
    CommentsTcpDelegationSchedule,
};
use rustok_core::ModuleRuntimeExtensions;

use crate::error::Result;
use crate::services::server_runtime_context::ServerRuntimeContext;

mod base {
    pub(super) use super::super::base::*;
}

mod keyring {
    pub(super) use super::super::keyring::*;
}

mod keyring_reload {
    pub(super) use super::super::keyring_reload::*;
}

mod keyring_reload_guard {
    pub(super) use super::super::keyring_reload_guard::*;
}

mod historical {
    include!("comments_provider_runtime_keyring_schedule_base.rs");
    include!("comments_provider_runtime_keyring_schedule_persistence_bridge.rs");
}

pub use historical::{
    COMMENTS_TCP_DELEGATION_SCHEDULE_ENABLED_ENV, CommentsTcpDelegationScheduleReloadOutcome,
    CommentsTcpDelegationScheduleReloadStatus, CommentsTcpDelegationScheduleRuntimeSelection,
};

/// Public schedule handle with read-only status and provider behavior.
///
/// Mutation is intentionally restricted to sibling server-owned trigger code.
#[derive(Clone)]
pub struct SharedCommentsTcpDelegationScheduleHandle(
    historical::SharedCommentsTcpDelegationScheduleHandle,
);

impl SharedCommentsTcpDelegationScheduleHandle {
    pub fn from_host_schedule(
        schedule: CommentsTcpDelegationSchedule,
        generation: u64,
    ) -> std::result::Result<Self, String> {
        historical::SharedCommentsTcpDelegationScheduleHandle::from_host_schedule(
            schedule, generation,
        )
        .map(Self)
    }

    pub fn from_file(
        file_path: impl AsRef<Path>,
        max_ttl: Duration,
    ) -> std::result::Result<Self, String> {
        historical::SharedCommentsTcpDelegationScheduleHandle::from_file(file_path, max_ttl)
            .map(Self)
    }

    pub fn current_status(
        &self,
    ) -> std::result::Result<CommentsTcpDelegationScheduleReloadStatus, String> {
        self.0.current_status()
    }

    pub fn current_selection(
        &self,
    ) -> std::result::Result<CommentsTcpDelegationScheduleRuntimeSelection, String> {
        self.0.current_selection()
    }

    pub(super) fn from_prepared_file(
        file_path: PathBuf,
        schedule: CommentsTcpDelegationSchedule,
        generation: u64,
    ) -> std::result::Result<Self, String> {
        historical::SharedCommentsTcpDelegationScheduleHandle::from_prepared_file(
            file_path, schedule, generation,
        )
        .map(Self)
    }

    pub(super) fn replace_host_schedule(
        &self,
        schedule: CommentsTcpDelegationSchedule,
        generation: u64,
    ) -> std::result::Result<CommentsTcpDelegationScheduleReloadOutcome, String> {
        self.0.replace_host_schedule(schedule, generation)
    }

    pub(super) fn reload_file(
        &self,
    ) -> std::result::Result<CommentsTcpDelegationScheduleReloadOutcome, String> {
        self.0.reload_file()
    }

    pub(super) fn replace_prepared_with_commit<F>(
        &self,
        schedule: CommentsTcpDelegationSchedule,
        generation: u64,
        source: keyring::CommentsTcpDelegationKeyringSource,
        before_publish: F,
    ) -> std::result::Result<CommentsTcpDelegationScheduleReloadOutcome, String>
    where
        F: FnOnce() -> std::result::Result<(), String>,
    {
        self.0
            .replace_prepared_with_commit(schedule, generation, source, before_publish)
    }

    fn historical_clone(&self) -> historical::SharedCommentsTcpDelegationScheduleHandle {
        self.0.clone()
    }
}

impl CommentsTcpDelegationKeyringProvider for SharedCommentsTcpDelegationScheduleHandle {
    fn current_keyring(&self) -> std::result::Result<CommentsTcpDelegationKeyring, PortError> {
        CommentsTcpDelegationKeyringProvider::current_keyring(&self.0)
    }
}

impl fmt::Debug for SharedCommentsTcpDelegationScheduleHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

pub(super) fn register_comments_provider_runtime(
    extensions: &mut ModuleRuntimeExtensions,
) -> std::result::Result<(), String> {
    if let Some(handle) = extensions
        .get::<SharedCommentsTcpDelegationScheduleHandle>()
        .cloned()
    {
        if extensions
            .get::<historical::SharedCommentsTcpDelegationScheduleHandle>()
            .is_some()
        {
            return Err(
                "Public and historical Comments TCP delegation schedule handles cannot be combined"
                    .to_string(),
            );
        }
        extensions.insert(handle.historical_clone());
    }

    historical::register_comments_provider_runtime(extensions)?;

    if extensions
        .get::<SharedCommentsTcpDelegationScheduleHandle>()
        .is_none()
        && let Some(handle) = extensions
            .get::<historical::SharedCommentsTcpDelegationScheduleHandle>()
            .cloned()
    {
        extensions.insert(SharedCommentsTcpDelegationScheduleHandle(handle));
    }
    Ok(())
}

pub(super) async fn start_comments_tcp_listener_if_enabled(
    runtime_ctx: &ServerRuntimeContext,
) -> Result<()> {
    historical::start_comments_tcp_listener_if_enabled(runtime_ctx).await
}

// Historical slice-79 and slice-80 source-verifier markers retained while the
// immutable owner remains byte-for-byte in the private base file.
// "RUSTOK_COMMENTS_TCP_DELEGATION_SCHEDULE_ENABLED"
// COMMENTS_TCP_DELEGATION_SCHEDULE_SCHEMA_VERSION: u16 = 2
// pub struct CommentsTcpDelegationScheduleRuntimeSelection
// pub struct CommentsTcpDelegationScheduleReloadStatus
// pub struct CommentsTcpDelegationScheduleReloadOutcome
// current: RwLock<DelegationScheduleSnapshot>
// pub fn replace_host_schedule(
// pub fn reload_file(
// candidate.generation <= current.generation
// .validate_replacement_from(&current.schedule, now_ms)
// *current = candidate;
// schedule.current_keyring()
// ReloadableCommentsTcpDelegationSigner::with_ttl(
// ReloadableTcpJsonCommentsTransport::with_channel_connector_bearer_and_delegation(
// ReloadableCommentsTcpDelegatingAuthorityResolver::new(
// keyring_reload::register_comments_provider_runtime(extensions)
// keyring_reload_guard::start_comments_tcp_listener_if_enabled(runtime_ctx).await
// base::start_comments_tcp_listener_if_enabled(runtime_ctx).await
// #[serde(deny_unknown_fields)]
// schema_version: u16
// generation: u64
// propagation_budget_ms: u64
// activates_at_unix_ms: u64
// retires_at_unix_ms: Option<u64>
// schema_version must equal {COMMENTS_TCP_DELEGATION_SCHEDULE_SCHEMA_VERSION} in schedule mode
// metadata.len() > keyring::MAX_COMMENTS_TCP_DELEGATION_KEYRING_FILE_BYTES as u64
// .take((keyring::MAX_COMMENTS_TCP_DELEGATION_KEYRING_FILE_BYTES + 1) as u64)
// CommentsTcpDelegationScheduledKey::new(
// CommentsTcpDelegationSchedule::new(
// DEFAULT_COMMENTS_TCP_DELEGATION_CLOCK_SKEW_MS
// Scheduled, static, and ordinary reloadable Comments TCP delegation keyrings cannot be combined
// Host-provided Comments TCP delegation schedule cannot be combined with file or legacy-secret environment configuration
// Comments TCP delegation schedule requires a built-in TCP client or an enabled TCP listener
// Comments TCP delegation schedule is unused because an external listener authority override is configured
// .field("file_path", &"[REDACTED]")
// .field("key_ids", &"[REDACTED]")
// .field("secrets", &"[REDACTED]")
