use std::{
    env,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::services::app_lifecycle::StopHandle;
use crate::services::server_runtime_context::ServerRuntimeContext;

use super::{
    keyring_schedule_audit_canonical_writer::RustokOutboxCommentsTcpDelegationScheduleAuditCanonicalWriter,
    keyring_schedule_audit_handoff_postgres::{
        COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_MAX_CLAIM_SECONDS,
        CommentsTcpDelegationScheduleAuditHandoffError,
        PostgresCommentsTcpDelegationScheduleAuditCanonicalHandoff,
    },
    keyring_schedule_audit_publication::SharedCommentsTcpDelegationScheduleAuditCanonicalWriter,
};

pub const COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_ENABLED_ENV: &str =
    "RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_ENABLED";
pub const COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_CONTROL_PLANE_TENANT_ID_ENV: &str =
    "RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_CONTROL_PLANE_TENANT_ID";
pub const COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_CLAIM_TTL_SECONDS_ENV: &str =
    "RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_CLAIM_TTL_SECONDS";
pub const COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_IDLE_POLL_MS_ENV: &str =
    "RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_IDLE_POLL_MS";
pub const COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_RETRY_DELAY_MS_ENV: &str =
    "RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_RETRY_DELAY_MS";
pub const COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_MAX_CLAIMS_PER_CYCLE_ENV: &str =
    "RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_MAX_CLAIMS_PER_CYCLE";

const DEFAULT_CLAIM_TTL_SECONDS: u64 = 60;
const DEFAULT_IDLE_POLL_MS: u64 = 1_000;
const DEFAULT_RETRY_DELAY_MS: u64 = 1_000;
const DEFAULT_MAX_CLAIMS_PER_CYCLE: usize = 32;
const MAX_IDLE_POLL_MS: u64 = 60_000;
const MAX_RETRY_DELAY_MS: u64 = 60_000;
const MAX_CLAIMS_PER_CYCLE: usize = 256;
const ACTIVE_CYCLE_DELAY: Duration = Duration::from_millis(1);

static HANDOFF_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct HandoffWorkerCycleOutcome {
    pub(crate) calls: usize,
    pub(crate) claimed: usize,
    pub(crate) published: usize,
    pub(crate) conflicts: usize,
    pub(crate) unavailable: usize,
    pub(crate) reached_empty: bool,
}

#[cfg(test)]
impl HandoffWorkerCycleOutcome {
    pub(crate) fn had_error(self) -> bool {
        self.conflicts > 0 || self.unavailable > 0
    }

    pub(crate) fn next_delay(
        self,
        config: CommentsTcpDelegationScheduleAuditHandoffWorkerConfig,
    ) -> Duration {
        if self.had_error() {
            config.retry_delay
        } else if !self.reached_empty && self.calls >= config.max_claims_per_cycle {
            ACTIVE_CYCLE_DELAY
        } else {
            config.idle_poll
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommentsTcpDelegationScheduleAuditHandoffWorkerConfig {
    control_plane_tenant_id: Uuid,
    claim_ttl: Duration,
    idle_poll: Duration,
    retry_delay: Duration,
    max_claims_per_cycle: usize,
}

impl CommentsTcpDelegationScheduleAuditHandoffWorkerConfig {
    pub fn from_environment() -> std::result::Result<Option<Self>, String> {
        let enabled = match read_optional_environment(
            COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_ENABLED_ENV,
        )? {
            Some(value) => parse_bool_value(
                COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_ENABLED_ENV,
                value.as_str(),
            )?,
            None => false,
        };
        if !enabled {
            return Ok(None);
        }

        let control_plane_tenant_id = parse_required_canonical_uuid(
            COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_CONTROL_PLANE_TENANT_ID_ENV,
            read_optional_environment(
                COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_CONTROL_PLANE_TENANT_ID_ENV,
            )?
            .as_deref(),
        )?;
        let claim_ttl_seconds = parse_optional_bounded_u64(
            COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_CLAIM_TTL_SECONDS_ENV,
            read_optional_environment(
                COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_CLAIM_TTL_SECONDS_ENV,
            )?
            .as_deref(),
            DEFAULT_CLAIM_TTL_SECONDS,
            COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_MAX_CLAIM_SECONDS,
        )?;
        let idle_poll_ms = parse_optional_bounded_u64(
            COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_IDLE_POLL_MS_ENV,
            read_optional_environment(
                COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_IDLE_POLL_MS_ENV,
            )?
            .as_deref(),
            DEFAULT_IDLE_POLL_MS,
            MAX_IDLE_POLL_MS,
        )?;
        let retry_delay_ms = parse_optional_bounded_u64(
            COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_RETRY_DELAY_MS_ENV,
            read_optional_environment(
                COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_RETRY_DELAY_MS_ENV,
            )?
            .as_deref(),
            DEFAULT_RETRY_DELAY_MS,
            MAX_RETRY_DELAY_MS,
        )?;
        let max_claims_per_cycle = parse_optional_bounded_usize(
            COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_MAX_CLAIMS_PER_CYCLE_ENV,
            read_optional_environment(
                COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_MAX_CLAIMS_PER_CYCLE_ENV,
            )?
            .as_deref(),
            DEFAULT_MAX_CLAIMS_PER_CYCLE,
            MAX_CLAIMS_PER_CYCLE,
        )?;

        Ok(Some(Self {
            control_plane_tenant_id,
            claim_ttl: Duration::from_secs(claim_ttl_seconds),
            idle_poll: Duration::from_millis(idle_poll_ms),
            retry_delay: Duration::from_millis(retry_delay_ms),
            max_claims_per_cycle,
        }))
    }

    pub fn control_plane_tenant_id(&self) -> Uuid {
        self.control_plane_tenant_id
    }

    pub fn claim_ttl(&self) -> Duration {
        self.claim_ttl
    }

    pub fn idle_poll(&self) -> Duration {
        self.idle_poll
    }

    pub fn retry_delay(&self) -> Duration {
        self.retry_delay
    }

    pub fn max_claims_per_cycle(&self) -> usize {
        self.max_claims_per_cycle
    }
}

struct CommentsTcpDelegationScheduleAuditHandoffWorkerRuntime {
    instance_id: u64,
    task: JoinHandle<()>,
}

#[derive(Clone)]
pub struct CommentsTcpDelegationScheduleAuditHandoffWorkerHandle(
    Arc<CommentsTcpDelegationScheduleAuditHandoffWorkerRuntime>,
);

impl CommentsTcpDelegationScheduleAuditHandoffWorkerHandle {
    pub fn instance_id(&self) -> u64 {
        self.0.instance_id
    }

    pub fn is_finished(&self) -> bool {
        self.0.task.is_finished()
    }
}

struct CommentsTcpDelegationScheduleAuditHandoffWorkerLifecycleReservation;

fn ensure_stop_handle(runtime_ctx: &ServerRuntimeContext) -> StopHandle {
    if let Some(handle) = runtime_ctx.shared_get::<StopHandle>() {
        return handle;
    }

    let (candidate, _receiver) = StopHandle::new();
    let _ = runtime_ctx.shared_insert_if_absent(candidate.clone());
    runtime_ctx
        .shared_get::<StopHandle>()
        .expect("StopHandle must exist after Comments schedule audit worker initialization")
}

fn read_optional_environment(name: &str) -> std::result::Result<Option<String>, String> {
    match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(format!("{name} must contain valid UTF-8")),
    }
}

fn parse_bool_value(name: &str, value: &str) -> std::result::Result<bool, String> {
    if value.trim() != value {
        return Err(format!("{name} must not contain surrounding whitespace"));
    }
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("{name} must be exactly true or false")),
    }
}

fn parse_required_canonical_uuid(
    name: &str,
    value: Option<&str>,
) -> std::result::Result<Uuid, String> {
    let value = value.ok_or_else(|| format!("{name} is required when handoff is enabled"))?;
    if value.trim() != value {
        return Err(format!("{name} must not contain surrounding whitespace"));
    }
    let parsed =
        Uuid::parse_str(value).map_err(|_| format!("{name} must be a canonical non-nil UUID"))?;
    if parsed.is_nil() || parsed.to_string() != value {
        return Err(format!("{name} must be a canonical non-nil UUID"));
    }
    Ok(parsed)
}

fn parse_optional_bounded_u64(
    name: &str,
    value: Option<&str>,
    default: u64,
    max: u64,
) -> std::result::Result<u64, String> {
    let Some(value) = value else {
        return Ok(default);
    };
    if value.trim() != value {
        return Err(format!("{name} must not contain surrounding whitespace"));
    }
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("{name} must be an integer in 1..={max}"))?;
    if parsed == 0 || parsed > max {
        return Err(format!("{name} must be an integer in 1..={max}"));
    }
    Ok(parsed)
}

fn parse_optional_bounded_usize(
    name: &str,
    value: Option<&str>,
    default: usize,
    max: usize,
) -> std::result::Result<usize, String> {
    let Some(value) = value else {
        return Ok(default);
    };
    if value.trim() != value {
        return Err(format!("{name} must not contain surrounding whitespace"));
    }
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("{name} must be an integer in 1..={max}"))?;
    if parsed == 0 || parsed > max {
        return Err(format!("{name} must be an integer in 1..={max}"));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_config_parsers_reject_ambiguous_values() {
        assert_eq!(parse_bool_value("enabled", "true"), Ok(true));
        assert!(parse_bool_value("enabled", " true").is_err());
        assert!(parse_bool_value("enabled", "TRUE").is_err());
        assert!(parse_optional_bounded_u64("ttl", Some("0"), 60, 300).is_err());
        assert!(parse_optional_bounded_u64("ttl", Some("301"), 60, 300).is_err());
        assert!(parse_optional_bounded_usize("batch", Some("257"), 32, 256).is_err());
    }

    #[test]
    fn control_plane_tenant_requires_canonical_non_nil_uuid() {
        let tenant_id = Uuid::new_v4();
        assert_eq!(
            parse_required_canonical_uuid("tenant", Some(&tenant_id.to_string())),
            Ok(tenant_id)
        );
        assert!(parse_required_canonical_uuid("tenant", None).is_err());
        assert!(
            parse_required_canonical_uuid("tenant", Some("00000000-0000-0000-0000-000000000000"))
                .is_err()
        );
        assert!(
            parse_required_canonical_uuid("tenant", Some(" 00000000-0000-0000-0000-000000000001"))
                .is_err()
        );
    }

    #[test]
    fn cycle_delay_is_bounded_by_outcome_class() {
        let config = CommentsTcpDelegationScheduleAuditHandoffWorkerConfig {
            control_plane_tenant_id: Uuid::new_v4(),
            claim_ttl: Duration::from_secs(60),
            idle_poll: Duration::from_secs(1),
            retry_delay: Duration::from_secs(2),
            max_claims_per_cycle: 32,
        };
        assert_eq!(
            HandoffWorkerCycleOutcome {
                calls: 1,
                reached_empty: true,
                ..Default::default()
            }
            .next_delay(config),
            Duration::from_secs(1)
        );
        assert_eq!(
            HandoffWorkerCycleOutcome {
                calls: 1,
                conflicts: 1,
                ..Default::default()
            }
            .next_delay(config),
            Duration::from_secs(2)
        );
        assert_eq!(
            HandoffWorkerCycleOutcome {
                calls: 32,
                published: 32,
                ..Default::default()
            }
            .next_delay(config),
            ACTIVE_CYCLE_DELAY
        );
    }
}
