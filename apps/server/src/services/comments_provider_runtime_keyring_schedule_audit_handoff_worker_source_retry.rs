use super::keyring_schedule_audit_source_retry_postgres::{
    COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_SOURCE_MAX_ATTEMPTS,
    COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_SOURCE_MAX_RETRY_DELAY_SECONDS,
    CommentsTcpDelegationScheduleAuditSourceFailureTransition,
    CommentsTcpDelegationScheduleAuditSourceRetryPolicyError,
    PostgresCommentsTcpDelegationScheduleAuditSourceRetryPolicy,
};

pub const COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_SOURCE_MAX_ATTEMPTS_ENV: &str =
    "RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_SOURCE_MAX_ATTEMPTS";
pub const COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_SOURCE_RETRY_DELAY_SECONDS_ENV: &str =
    "RUSTOK_COMMENTS_TCP_DELEGATION_AUDIT_HANDOFF_SOURCE_RETRY_DELAY_SECONDS";

const DEFAULT_SOURCE_MAX_ATTEMPTS: u64 = 8;
const DEFAULT_SOURCE_RETRY_DELAY_SECONDS: u64 = 30;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommentsTcpDelegationScheduleAuditSourceRetryWorkerConfig {
    handoff: CommentsTcpDelegationScheduleAuditHandoffWorkerConfig,
    source_max_attempts: u32,
    source_retry_delay: Duration,
}

impl CommentsTcpDelegationScheduleAuditSourceRetryWorkerConfig {
    pub fn from_environment() -> std::result::Result<Option<Self>, String> {
        let Some(handoff) =
            CommentsTcpDelegationScheduleAuditHandoffWorkerConfig::from_environment()?
        else {
            return Ok(None);
        };
        let source_max_attempts = parse_optional_bounded_u64(
            COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_SOURCE_MAX_ATTEMPTS_ENV,
            read_optional_environment(
                COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_SOURCE_MAX_ATTEMPTS_ENV,
            )?
            .as_deref(),
            DEFAULT_SOURCE_MAX_ATTEMPTS,
            u64::from(COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_SOURCE_MAX_ATTEMPTS),
        )?;
        let source_max_attempts = u32::try_from(source_max_attempts).map_err(|_| {
            format!(
                "{COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_SOURCE_MAX_ATTEMPTS_ENV} is out of range"
            )
        })?;
        let source_retry_delay_seconds = parse_optional_bounded_u64(
            COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_SOURCE_RETRY_DELAY_SECONDS_ENV,
            read_optional_environment(
                COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_HANDOFF_SOURCE_RETRY_DELAY_SECONDS_ENV,
            )?
            .as_deref(),
            DEFAULT_SOURCE_RETRY_DELAY_SECONDS,
            COMMENTS_TCP_DELEGATION_SCHEDULE_AUDIT_SOURCE_MAX_RETRY_DELAY_SECONDS,
        )?;

        Ok(Some(Self {
            handoff,
            source_max_attempts,
            source_retry_delay: Duration::from_secs(source_retry_delay_seconds),
        }))
    }

    pub fn handoff(&self) -> CommentsTcpDelegationScheduleAuditHandoffWorkerConfig {
        self.handoff
    }

    pub fn source_max_attempts(&self) -> u32 {
        self.source_max_attempts
    }

    pub fn source_retry_delay(&self) -> Duration {
        self.source_retry_delay
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SourceRetryHandoffWorkerCycleOutcome {
    calls: usize,
    claimed: usize,
    published: usize,
    conflicts: usize,
    unavailable: usize,
    retries_scheduled: usize,
    dead_lettered: usize,
    stale_claims: usize,
    swept_dead_letters: usize,
    policy_invalid_state: usize,
    policy_unavailable: usize,
    reached_empty: bool,
}

impl SourceRetryHandoffWorkerCycleOutcome {
    fn had_error(self) -> bool {
        self.conflicts > 0
            || self.unavailable > 0
            || self.stale_claims > 0
            || self.policy_invalid_state > 0
            || self.policy_unavailable > 0
    }

    fn made_progress(self) -> bool {
        self.published > 0
            || self.retries_scheduled > 0
            || self.dead_lettered > 0
            || self.swept_dead_letters > 0
    }

    fn next_delay(
        self,
        config: CommentsTcpDelegationScheduleAuditSourceRetryWorkerConfig,
    ) -> Duration {
        if self.had_error() {
            config.handoff.retry_delay
        } else if !self.reached_empty && self.calls >= config.handoff.max_claims_per_cycle {
            ACTIVE_CYCLE_DELAY
        } else {
            config.handoff.idle_poll
        }
    }

    fn count_handoff_error(&mut self, error: CommentsTcpDelegationScheduleAuditHandoffError) {
        match error {
            CommentsTcpDelegationScheduleAuditHandoffError::Conflict => self.conflicts += 1,
            CommentsTcpDelegationScheduleAuditHandoffError::Unavailable => self.unavailable += 1,
        }
    }

    fn count_policy_error(
        &mut self,
        error: CommentsTcpDelegationScheduleAuditSourceRetryPolicyError,
    ) {
        match error {
            CommentsTcpDelegationScheduleAuditSourceRetryPolicyError::InvalidStoredState => {
                self.policy_invalid_state += 1
            }
            CommentsTcpDelegationScheduleAuditSourceRetryPolicyError::Unavailable => {
                self.policy_unavailable += 1
            }
        }
    }
}

/// Starts the canonical Blog source handoff worker with the durable source retry
/// and exhaustion policy composed into the same task and shutdown lane.
pub fn start_comments_tcp_delegation_schedule_audit_handoff_worker_with_source_retry_if_enabled(
    runtime_ctx: &ServerRuntimeContext,
) -> Result<()> {
    let Some(config) =
        CommentsTcpDelegationScheduleAuditSourceRetryWorkerConfig::from_environment()
            .map_err(Error::BadRequest)?
    else {
        return Ok(());
    };

    if !runtime_ctx.settings().runtime.runs_background_workers() {
        return Err(Error::BadRequest(
            "Comments schedule audit handoff worker requires a background-worker host mode"
                .to_string(),
        ));
    }

    if !runtime_ctx.shared_insert_if_absent(
        CommentsTcpDelegationScheduleAuditHandoffWorkerLifecycleReservation,
    ) {
        return Ok(());
    }

    let result = start_source_retry_handoff_worker(runtime_ctx, config);
    if result.is_err() {
        let _ = runtime_ctx
            .shared_take::<CommentsTcpDelegationScheduleAuditHandoffWorkerLifecycleReservation>();
    }
    result
}

fn start_source_retry_handoff_worker(
    runtime_ctx: &ServerRuntimeContext,
    config: CommentsTcpDelegationScheduleAuditSourceRetryWorkerConfig,
) -> Result<()> {
    let writer: SharedCommentsTcpDelegationScheduleAuditCanonicalWriter =
        Arc::new(RustokOutboxCommentsTcpDelegationScheduleAuditCanonicalWriter);
    let database = runtime_ctx.db_clone();
    let handoff = PostgresCommentsTcpDelegationScheduleAuditCanonicalHandoff::new(
        database.clone(),
        config.handoff.control_plane_tenant_id,
        writer,
        config.handoff.claim_ttl,
    )
    .map_err(Error::BadRequest)?;
    let retry_policy = PostgresCommentsTcpDelegationScheduleAuditSourceRetryPolicy::new(
        database,
        config.source_max_attempts,
        config.source_retry_delay,
    )
    .map_err(Error::BadRequest)?;

    let stop_handle = ensure_stop_handle(runtime_ctx);
    let stop_rx = stop_handle.subscribe();
    let instance_id = HANDOFF_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    let task = tokio::spawn(run_source_retry_handoff_worker(
        handoff,
        retry_policy,
        config,
        stop_rx,
        instance_id,
    ));
    runtime_ctx.shared_insert(CommentsTcpDelegationScheduleAuditHandoffWorkerHandle(
        Arc::new(CommentsTcpDelegationScheduleAuditHandoffWorkerRuntime { instance_id, task }),
    ));

    tracing::info!(
        instance_id,
        claim_ttl_seconds = config.handoff.claim_ttl.as_secs(),
        idle_poll_ms = config.handoff.idle_poll.as_millis(),
        loop_retry_delay_ms = config.handoff.retry_delay.as_millis(),
        max_claims_per_cycle = config.handoff.max_claims_per_cycle,
        source_max_attempts = config.source_max_attempts,
        source_retry_delay_seconds = config.source_retry_delay.as_secs(),
        "Comments schedule audit source-retry handoff worker started"
    );
    Ok(())
}

async fn run_source_retry_handoff_worker(
    handoff: PostgresCommentsTcpDelegationScheduleAuditCanonicalHandoff,
    retry_policy: PostgresCommentsTcpDelegationScheduleAuditSourceRetryPolicy,
    config: CommentsTcpDelegationScheduleAuditSourceRetryWorkerConfig,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
    instance_id: u64,
) {
    loop {
        if *stop_rx.borrow() {
            tracing::info!(
                instance_id,
                "Comments schedule audit source-retry handoff worker received shutdown signal"
            );
            return;
        }

        let outcome = run_source_retry_handoff_cycle(
            &handoff,
            &retry_policy,
            config.source_max_attempts,
            config.handoff.max_claims_per_cycle,
        )
        .await;
        if outcome.made_progress() {
            tracing::info!(
                instance_id,
                calls = outcome.calls,
                claimed = outcome.claimed,
                published = outcome.published,
                conflicts = outcome.conflicts,
                unavailable = outcome.unavailable,
                retries_scheduled = outcome.retries_scheduled,
                dead_lettered = outcome.dead_lettered,
                stale_claims = outcome.stale_claims,
                swept_dead_letters = outcome.swept_dead_letters,
                policy_invalid_state = outcome.policy_invalid_state,
                policy_unavailable = outcome.policy_unavailable,
                "Comments schedule audit source-retry handoff worker completed a bounded cycle"
            );
        } else if outcome.had_error() {
            tracing::warn!(
                instance_id,
                calls = outcome.calls,
                claimed = outcome.claimed,
                conflicts = outcome.conflicts,
                unavailable = outcome.unavailable,
                stale_claims = outcome.stale_claims,
                policy_invalid_state = outcome.policy_invalid_state,
                policy_unavailable = outcome.policy_unavailable,
                "Comments schedule audit source-retry handoff worker cycle failed closed"
            );
        }

        let delay = outcome.next_delay(config);
        tokio::select! {
            _ = tokio::time::sleep(delay) => {}
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    tracing::info!(
                        instance_id,
                        "Comments schedule audit source-retry handoff worker received shutdown signal"
                    );
                    return;
                }
            }
        }
    }
}

async fn run_source_retry_handoff_cycle(
    handoff: &PostgresCommentsTcpDelegationScheduleAuditCanonicalHandoff,
    retry_policy: &PostgresCommentsTcpDelegationScheduleAuditSourceRetryPolicy,
    source_max_attempts: u32,
    max_claims_per_cycle: usize,
) -> SourceRetryHandoffWorkerCycleOutcome {
    let mut outcome = SourceRetryHandoffWorkerCycleOutcome::default();

    match retry_policy.dead_letter_next_expired_exhausted().await {
        Ok(Some(_)) => outcome.swept_dead_letters += 1,
        Ok(None) => {}
        Err(error) => {
            outcome.count_policy_error(error);
            if error == CommentsTcpDelegationScheduleAuditSourceRetryPolicyError::Unavailable {
                return outcome;
            }
        }
    }

    while outcome.calls < max_claims_per_cycle {
        outcome.calls += 1;
        let claim = match handoff.claim_next_retry_ready(source_max_attempts).await {
            Ok(Some(claim)) => {
                outcome.claimed += 1;
                claim
            }
            Ok(None) => {
                outcome.reached_empty = true;
                break;
            }
            Err(error) => {
                outcome.count_handoff_error(error);
                break;
            }
        };

        match handoff.publish_claimed(claim).await {
            Ok(_) => outcome.published += 1,
            Err(error) => {
                outcome.count_handoff_error(error);
                match retry_policy.record_active_failure(claim, error).await {
                    Ok(
                        CommentsTcpDelegationScheduleAuditSourceFailureTransition::RetryScheduled {
                            ..
                        },
                    ) => outcome.retries_scheduled += 1,
                    Ok(
                        CommentsTcpDelegationScheduleAuditSourceFailureTransition::DeadLettered {
                            ..
                        },
                    ) => outcome.dead_lettered += 1,
                    Ok(CommentsTcpDelegationScheduleAuditSourceFailureTransition::StaleClaim) => {
                        outcome.stale_claims += 1;
                        break;
                    }
                    Err(policy_error) => {
                        outcome.count_policy_error(policy_error);
                        break;
                    }
                }
            }
        }
    }

    outcome
}

#[cfg(test)]
mod source_retry_worker_tests {
    use super::*;

    fn config() -> CommentsTcpDelegationScheduleAuditSourceRetryWorkerConfig {
        CommentsTcpDelegationScheduleAuditSourceRetryWorkerConfig {
            handoff: CommentsTcpDelegationScheduleAuditHandoffWorkerConfig {
                control_plane_tenant_id: Uuid::new_v4(),
                claim_ttl: Duration::from_secs(60),
                idle_poll: Duration::from_secs(1),
                retry_delay: Duration::from_secs(2),
                max_claims_per_cycle: 32,
            },
            source_max_attempts: 8,
            source_retry_delay: Duration::from_secs(30),
        }
    }

    #[test]
    fn source_policy_bounds_are_strict() {
        assert!(parse_optional_bounded_u64("attempts", Some("0"), 8, 100).is_err());
        assert!(parse_optional_bounded_u64("attempts", Some("101"), 8, 100).is_err());
        assert!(parse_optional_bounded_u64("delay", Some("86401"), 30, 86_400).is_err());
    }

    #[test]
    fn source_retry_cycle_uses_loop_retry_delay_for_closed_errors() {
        let config = config();
        assert_eq!(
            SourceRetryHandoffWorkerCycleOutcome {
                conflicts: 1,
                ..Default::default()
            }
            .next_delay(config),
            Duration::from_secs(2)
        );
        assert_eq!(
            SourceRetryHandoffWorkerCycleOutcome {
                calls: 1,
                reached_empty: true,
                ..Default::default()
            }
            .next_delay(config),
            Duration::from_secs(1)
        );
        assert_eq!(
            SourceRetryHandoffWorkerCycleOutcome {
                calls: 32,
                published: 32,
                ..Default::default()
            }
            .next_delay(config),
            ACTIVE_CYCLE_DELAY
        );
    }
}
