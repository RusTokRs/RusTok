use std::{
    collections::HashSet,
    fmt,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rustok_api::PortError;
use thiserror::Error;

use crate::{
    CommentsTcpDelegationKeyId, CommentsTcpDelegationKeyring, CommentsTcpDelegationKeyringProvider,
    CommentsTcpDelegationSecret, MAX_COMMENTS_TCP_DELEGATION_KEYS,
    MAX_COMMENTS_TCP_DELEGATION_TTL_MS,
};

pub const MAX_COMMENTS_TCP_DELEGATION_PROPAGATION_BUDGET_MS: u64 = 300_000;
pub const MAX_COMMENTS_TCP_DELEGATION_SCHEDULE_CLOCK_SKEW_MS: u64 = 30_000;

#[derive(Clone, Eq, PartialEq)]
pub struct CommentsTcpDelegationScheduledKey {
    key_id: CommentsTcpDelegationKeyId,
    secret: CommentsTcpDelegationSecret,
    activates_at_unix_ms: u64,
    retires_at_unix_ms: Option<u64>,
}

impl CommentsTcpDelegationScheduledKey {
    pub fn new(
        key_id: CommentsTcpDelegationKeyId,
        secret: CommentsTcpDelegationSecret,
        activates_at_unix_ms: u64,
        retires_at_unix_ms: Option<u64>,
    ) -> Result<Self, CommentsTcpDelegationScheduleConfigError> {
        if activates_at_unix_ms == 0 {
            return Err(CommentsTcpDelegationScheduleConfigError::InvalidActivation);
        }
        if retires_at_unix_ms.is_some_and(|retirement| retirement <= activates_at_unix_ms) {
            return Err(CommentsTcpDelegationScheduleConfigError::InvalidRetirement);
        }
        Ok(Self {
            key_id,
            secret,
            activates_at_unix_ms,
            retires_at_unix_ms,
        })
    }

    pub fn activates_at_unix_ms(&self) -> u64 {
        self.activates_at_unix_ms
    }

    pub fn retires_at_unix_ms(&self) -> Option<u64> {
        self.retires_at_unix_ms
    }
}

impl fmt::Debug for CommentsTcpDelegationScheduledKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommentsTcpDelegationScheduledKey")
            .field("key_id", &"[CONFIGURED]")
            .field("secret", &"[REDACTED]")
            .field("activates_at_unix_ms", &self.activates_at_unix_ms)
            .field("retires_at_unix_ms", &self.retires_at_unix_ms)
            .finish()
    }
}

#[derive(Clone)]
pub struct CommentsTcpDelegationSchedule {
    keys: Arc<Vec<CommentsTcpDelegationScheduledKey>>,
    propagation_budget_ms: u64,
    max_ttl_ms: u64,
    clock_skew_ms: u64,
    legacy_unkeyed_key_id: Option<CommentsTcpDelegationKeyId>,
    last_observed_unix_ms: Arc<AtomicU64>,
}

impl CommentsTcpDelegationSchedule {
    pub fn new(
        mut keys: Vec<CommentsTcpDelegationScheduledKey>,
        propagation_budget: Duration,
        max_ttl: Duration,
        clock_skew: Duration,
    ) -> Result<Self, CommentsTcpDelegationScheduleConfigError> {
        if keys.is_empty() || keys.len() > MAX_COMMENTS_TCP_DELEGATION_KEYS {
            return Err(CommentsTcpDelegationScheduleConfigError::InvalidKeyCount);
        }
        let propagation_budget_ms = duration_ms(propagation_budget)
            .ok_or(CommentsTcpDelegationScheduleConfigError::InvalidPropagationBudget)?;
        if propagation_budget_ms == 0
            || propagation_budget_ms > MAX_COMMENTS_TCP_DELEGATION_PROPAGATION_BUDGET_MS
        {
            return Err(CommentsTcpDelegationScheduleConfigError::InvalidPropagationBudget);
        }
        let max_ttl_ms =
            duration_ms(max_ttl).ok_or(CommentsTcpDelegationScheduleConfigError::InvalidTtl)?;
        if max_ttl_ms == 0 || max_ttl_ms > MAX_COMMENTS_TCP_DELEGATION_TTL_MS {
            return Err(CommentsTcpDelegationScheduleConfigError::InvalidTtl);
        }
        let clock_skew_ms = duration_ms(clock_skew)
            .ok_or(CommentsTcpDelegationScheduleConfigError::InvalidClockSkew)?;
        if clock_skew_ms > MAX_COMMENTS_TCP_DELEGATION_SCHEDULE_CLOCK_SKEW_MS {
            return Err(CommentsTcpDelegationScheduleConfigError::InvalidClockSkew);
        }

        keys.sort_by_key(|key| key.activates_at_unix_ms);
        let mut key_ids = HashSet::with_capacity(keys.len());
        for key in &keys {
            if !key_ids.insert(key.key_id.clone()) {
                return Err(CommentsTcpDelegationScheduleConfigError::DuplicateKeyId);
            }
        }
        for window in keys.windows(2) {
            let current = &window[0];
            let successor = &window[1];
            if current.activates_at_unix_ms == successor.activates_at_unix_ms {
                return Err(CommentsTcpDelegationScheduleConfigError::DuplicateActivation);
            }
            let retirement = current
                .retires_at_unix_ms
                .ok_or(CommentsTcpDelegationScheduleConfigError::MissingRetirement)?;
            let required_retirement = successor
                .activates_at_unix_ms
                .checked_add(propagation_budget_ms)
                .and_then(|value| value.checked_add(max_ttl_ms))
                .and_then(|value| value.checked_add(clock_skew_ms))
                .ok_or(CommentsTcpDelegationScheduleConfigError::InsufficientOverlap)?;
            if retirement < required_retirement {
                return Err(CommentsTcpDelegationScheduleConfigError::InsufficientOverlap);
            }
        }
        if keys
            .last()
            .is_some_and(|key| key.retires_at_unix_ms.is_some())
        {
            return Err(CommentsTcpDelegationScheduleConfigError::TerminalKeyMustRemain);
        }

        Ok(Self {
            keys: Arc::new(keys),
            propagation_budget_ms,
            max_ttl_ms,
            clock_skew_ms,
            legacy_unkeyed_key_id: None,
            last_observed_unix_ms: Arc::new(AtomicU64::new(0)),
        })
    }

    pub fn with_legacy_unkeyed_key_id(
        mut self,
        key_id: CommentsTcpDelegationKeyId,
    ) -> Result<Self, CommentsTcpDelegationScheduleConfigError> {
        if !self.keys.iter().any(|key| key.key_id == key_id) {
            return Err(CommentsTcpDelegationScheduleConfigError::LegacyKeyMissing);
        }
        self.legacy_unkeyed_key_id = Some(key_id);
        Ok(self)
    }

    pub fn scheduled_key_count(&self) -> usize {
        self.keys.len()
    }

    pub fn propagation_budget_ms(&self) -> u64 {
        self.propagation_budget_ms
    }

    pub fn max_ttl_ms(&self) -> u64 {
        self.max_ttl_ms
    }

    pub fn clock_skew_ms(&self) -> u64 {
        self.clock_skew_ms
    }

    pub fn accepts_legacy_unkeyed_tokens(&self) -> bool {
        self.legacy_unkeyed_key_id.is_some()
    }

    pub fn current_keyring_at(
        &self,
        now_ms: u64,
    ) -> Result<CommentsTcpDelegationKeyring, PortError> {
        let active = self
            .active_key_at(now_ms)
            .ok_or_else(schedule_unavailable)?;
        let mut verification_keys = Vec::with_capacity(self.keys.len());
        let mut legacy_key_is_retained = false;
        for key in self.keys.iter() {
            let verification_starts_at = key
                .activates_at_unix_ms
                .saturating_sub(self.propagation_budget_ms);
            let before_retirement = key
                .retires_at_unix_ms
                .is_none_or(|retirement| now_ms <= retirement);
            if now_ms >= verification_starts_at && before_retirement {
                legacy_key_is_retained |= self
                    .legacy_unkeyed_key_id
                    .as_ref()
                    .is_some_and(|legacy_key_id| legacy_key_id == &key.key_id);
                verification_keys.push((key.key_id.clone(), key.secret.clone()));
            }
        }
        let mut keyring =
            CommentsTcpDelegationKeyring::new(active.key_id.clone(), verification_keys)
                .map_err(|_| schedule_unavailable())?;
        if legacy_key_is_retained {
            keyring = keyring
                .with_legacy_unkeyed_key_id(
                    self.legacy_unkeyed_key_id
                        .clone()
                        .ok_or_else(schedule_unavailable)?,
                )
                .map_err(|_| schedule_unavailable())?;
        }
        Ok(keyring)
    }

    pub fn validate_replacement_from(
        &self,
        previous: &Self,
        now_ms: u64,
    ) -> Result<(), CommentsTcpDelegationScheduleConfigError> {
        if self.max_ttl_ms != previous.max_ttl_ms || self.clock_skew_ms != previous.clock_skew_ms {
            return Err(CommentsTcpDelegationScheduleConfigError::RuntimePolicyChanged);
        }
        if self.propagation_budget_ms < previous.propagation_budget_ms {
            return Err(CommentsTcpDelegationScheduleConfigError::PropagationReduced);
        }
        let previous_active = previous
            .active_key_at(now_ms)
            .ok_or(CommentsTcpDelegationScheduleConfigError::ScheduleNotActive)?;
        let candidate_active = self
            .active_key_at(now_ms)
            .ok_or(CommentsTcpDelegationScheduleConfigError::ScheduleNotActive)?;
        if previous_active.key_id != candidate_active.key_id {
            return Err(CommentsTcpDelegationScheduleConfigError::ActiveKeyChangedEarly);
        }

        for retained in previous.keys.iter() {
            let still_required = retained
                .retires_at_unix_ms
                .is_none_or(|retirement| retirement >= now_ms);
            if !still_required {
                continue;
            }
            let candidate = self
                .keys
                .iter()
                .find(|candidate| candidate.key_id == retained.key_id)
                .ok_or(CommentsTcpDelegationScheduleConfigError::RetainedKeyMissing)?;
            if candidate.secret != retained.secret
                || candidate.activates_at_unix_ms != retained.activates_at_unix_ms
            {
                return Err(CommentsTcpDelegationScheduleConfigError::RetainedKeyChanged);
            }
            if let (Some(previous_retirement), Some(candidate_retirement)) =
                (retained.retires_at_unix_ms, candidate.retires_at_unix_ms)
                && candidate_retirement < previous_retirement
            {
                return Err(CommentsTcpDelegationScheduleConfigError::RetirementReduced);
            }
        }

        let earliest_new_activation = now_ms
            .checked_add(self.propagation_budget_ms)
            .ok_or(CommentsTcpDelegationScheduleConfigError::NewKeyActivatesTooEarly)?;
        for candidate in self.keys.iter() {
            if previous
                .keys
                .iter()
                .all(|retained| retained.key_id != candidate.key_id)
                && candidate.activates_at_unix_ms < earliest_new_activation
            {
                return Err(CommentsTcpDelegationScheduleConfigError::NewKeyActivatesTooEarly);
            }
        }

        match (
            previous.legacy_unkeyed_key_id.as_ref(),
            self.legacy_unkeyed_key_id.as_ref(),
        ) {
            (None, None) => {}
            (None, Some(_)) => {
                return Err(CommentsTcpDelegationScheduleConfigError::LegacyPolicyChangedEarly);
            }
            (Some(previous_legacy_key_id), Some(candidate_legacy_key_id))
                if previous_legacy_key_id == candidate_legacy_key_id => {}
            (Some(previous_legacy_key_id), None) => {
                let previous_legacy_key = previous
                    .keys
                    .iter()
                    .find(|key| &key.key_id == previous_legacy_key_id)
                    .ok_or(CommentsTcpDelegationScheduleConfigError::LegacyKeyMissing)?;
                let legacy_still_required = previous_legacy_key
                    .retires_at_unix_ms
                    .is_none_or(|retirement| retirement >= now_ms);
                if legacy_still_required {
                    return Err(CommentsTcpDelegationScheduleConfigError::LegacyPolicyChangedEarly);
                }
            }
            (Some(_), Some(_)) => {
                return Err(CommentsTcpDelegationScheduleConfigError::LegacyPolicyChangedEarly);
            }
        }
        Ok(())
    }

    fn active_key_at(&self, now_ms: u64) -> Option<&CommentsTcpDelegationScheduledKey> {
        self.keys
            .iter()
            .rev()
            .find(|key| key.activates_at_unix_ms <= now_ms)
    }

    fn observe_monotonic_time(&self, now_ms: u64) -> Result<(), PortError> {
        let mut observed = self.last_observed_unix_ms.load(Ordering::Acquire);
        loop {
            if now_ms < observed {
                return Err(schedule_unavailable());
            }
            if now_ms == observed {
                return Ok(());
            }
            match self.last_observed_unix_ms.compare_exchange_weak(
                observed,
                now_ms,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(()),
                Err(actual) => observed = actual,
            }
        }
    }
}

impl CommentsTcpDelegationKeyringProvider for CommentsTcpDelegationSchedule {
    fn current_keyring(&self) -> Result<CommentsTcpDelegationKeyring, PortError> {
        let now_ms = current_unix_ms()?;
        self.observe_monotonic_time(now_ms)?;
        self.current_keyring_at(now_ms)
    }
}

impl fmt::Debug for CommentsTcpDelegationSchedule {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommentsTcpDelegationSchedule")
            .field("scheduled_key_count", &self.keys.len())
            .field("propagation_budget_ms", &self.propagation_budget_ms)
            .field("max_ttl_ms", &self.max_ttl_ms)
            .field("clock_skew_ms", &self.clock_skew_ms)
            .field(
                "legacy_unkeyed_tokens",
                &self.legacy_unkeyed_key_id.is_some(),
            )
            .field("key_ids", &"[REDACTED]")
            .field("secrets", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, Eq, Error, PartialEq)]
pub enum CommentsTcpDelegationScheduleConfigError {
    #[error("Comments TCP delegation schedule must contain 1..=8 keys")]
    InvalidKeyCount,
    #[error("Comments TCP delegation schedule key IDs must be unique")]
    DuplicateKeyId,
    #[error("Comments TCP delegation activation timestamps must be unique and greater than zero")]
    DuplicateActivation,
    #[error("Comments TCP delegation activation timestamp must be greater than zero")]
    InvalidActivation,
    #[error("Comments TCP delegation retirement must be later than activation")]
    InvalidRetirement,
    #[error("Comments TCP delegation propagation budget must be within 1..=300000 milliseconds")]
    InvalidPropagationBudget,
    #[error("Comments TCP delegation schedule TTL must be within 1..=30000 milliseconds")]
    InvalidTtl,
    #[error("Comments TCP delegation schedule clock skew must be within 0..=30000 milliseconds")]
    InvalidClockSkew,
    #[error("Every non-terminal delegation key must have a retirement timestamp")]
    MissingRetirement,
    #[error("Delegation key retirement does not cover propagation, TTL, and clock skew")]
    InsufficientOverlap,
    #[error("The terminal delegation key must remain available without a retirement timestamp")]
    TerminalKeyMustRemain,
    #[error("Comments TCP delegation legacy key must exist in the schedule")]
    LegacyKeyMissing,
    #[error(
        "Comments TCP delegation runtime TTL or clock-skew policy cannot change during schedule replacement"
    )]
    RuntimePolicyChanged,
    #[error(
        "Comments TCP delegation propagation budget cannot decrease during schedule replacement"
    )]
    PropagationReduced,
    #[error("Comments TCP delegation schedule has no active signing key")]
    ScheduleNotActive,
    #[error("Comments TCP delegation active signing key cannot change during schedule replacement")]
    ActiveKeyChangedEarly,
    #[error("A retained delegation key cannot be removed before its retirement")]
    RetainedKeyMissing,
    #[error("A retained delegation key secret or activation timestamp cannot change")]
    RetainedKeyChanged,
    #[error("A retained delegation key retirement cannot move earlier")]
    RetirementReduced,
    #[error(
        "A new delegation key must be installed at least one propagation budget before activation"
    )]
    NewKeyActivatesTooEarly,
    #[error(
        "Legacy-unkeyed verification cannot be enabled or changed during replacement and cannot be disabled before retirement"
    )]
    LegacyPolicyChangedEarly,
}

fn duration_ms(duration: Duration) -> Option<u64> {
    u64::try_from(duration.as_millis()).ok()
}

fn current_unix_ms() -> Result<u64, PortError> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| {
        PortError::invariant_violation(
            "comments.tcp_delegation_clock_invalid",
            "Comments TCP delegation clock is not available",
        )
    })?;
    u64::try_from(elapsed.as_millis()).map_err(|_| {
        PortError::invariant_violation(
            "comments.tcp_delegation_clock_invalid",
            "Comments TCP delegation clock is not available",
        )
    })
}

fn schedule_unavailable() -> PortError {
    PortError::unavailable(
        "comments.tcp_delegation_schedule_unavailable",
        "Comments TCP delegation schedule has no safe keyring for this operation",
    )
}
