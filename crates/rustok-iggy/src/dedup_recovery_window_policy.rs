use std::time::Duration;

use thiserror::Error;

/// Reviewed Iggy message-ID deduplication settings used for an offline recovery-window
/// assessment.
///
/// The configuration is supplied by an operator or retained evidence. This type does not
/// connect to Iggy or read active server configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IggyDeduplicationConfiguration {
    Disabled,
    Enabled { max_entries: u64, expiry: Duration },
}

impl IggyDeduplicationConfiguration {
    pub const fn disabled() -> Self {
        Self::Disabled
    }

    pub fn enabled(
        max_entries: u64,
        expiry: Duration,
    ) -> Result<Self, IggyDedupRecoveryWindowPolicyError> {
        if max_entries == 0 || expiry.is_zero() {
            return Err(IggyDedupRecoveryWindowPolicyError::InvalidConfiguration);
        }

        Ok(Self::Enabled {
            max_entries,
            expiry,
        })
    }
}

/// Caller-supplied upper bounds for the complete publish-to-recovery interval.
///
/// The required expiry is the checked sum of the publication lease, process restart,
/// transport reconnect, and operator recovery bounds. The required capacity is the maximum
/// number of distinct deterministic message IDs that may share one physical partition during
/// that interval. No production default is provided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IggyDedupRecoveryWindowPolicy {
    publication_lease: Duration,
    process_restart: Duration,
    transport_reconnect: Duration,
    operator_recovery: Duration,
    required_max_entries_per_partition: u64,
    required_expiry: Duration,
}

impl IggyDedupRecoveryWindowPolicy {
    pub fn new(
        publication_lease: Duration,
        process_restart: Duration,
        transport_reconnect: Duration,
        operator_recovery: Duration,
        required_max_entries_per_partition: u64,
    ) -> Result<Self, IggyDedupRecoveryWindowPolicyError> {
        if publication_lease.is_zero() || required_max_entries_per_partition == 0 {
            return Err(IggyDedupRecoveryWindowPolicyError::InvalidPolicy);
        }

        let required_expiry = publication_lease
            .checked_add(process_restart)
            .and_then(|value| value.checked_add(transport_reconnect))
            .and_then(|value| value.checked_add(operator_recovery))
            .ok_or(IggyDedupRecoveryWindowPolicyError::RecoveryHorizonOverflow)?;

        Ok(Self {
            publication_lease,
            process_restart,
            transport_reconnect,
            operator_recovery,
            required_max_entries_per_partition,
            required_expiry,
        })
    }

    pub const fn publication_lease(&self) -> Duration {
        self.publication_lease
    }

    pub const fn process_restart(&self) -> Duration {
        self.process_restart
    }

    pub const fn transport_reconnect(&self) -> Duration {
        self.transport_reconnect
    }

    pub const fn operator_recovery(&self) -> Duration {
        self.operator_recovery
    }

    pub const fn required_max_entries_per_partition(&self) -> u64 {
        self.required_max_entries_per_partition
    }

    pub const fn required_expiry(&self) -> Duration {
        self.required_expiry
    }

    pub fn assess(
        &self,
        configuration: IggyDeduplicationConfiguration,
    ) -> IggyDedupRecoveryWindowAssessment {
        let IggyDeduplicationConfiguration::Enabled {
            max_entries,
            expiry,
        } = configuration
        else {
            return IggyDedupRecoveryWindowAssessment {
                status: IggyDedupRecoveryWindowStatus::Disabled,
                required_expiry: self.required_expiry,
                configured_expiry: None,
                required_max_entries_per_partition: self.required_max_entries_per_partition,
                configured_max_entries: None,
            };
        };

        let expiry_sufficient = expiry >= self.required_expiry;
        let capacity_sufficient = max_entries >= self.required_max_entries_per_partition;
        let status = match (expiry_sufficient, capacity_sufficient) {
            (true, true) => IggyDedupRecoveryWindowStatus::Sufficient,
            (false, true) => IggyDedupRecoveryWindowStatus::InsufficientExpiry,
            (true, false) => IggyDedupRecoveryWindowStatus::InsufficientCapacity,
            (false, false) => IggyDedupRecoveryWindowStatus::InsufficientExpiryAndCapacity,
        };

        IggyDedupRecoveryWindowAssessment {
            status,
            required_expiry: self.required_expiry,
            configured_expiry: Some(expiry),
            required_max_entries_per_partition: self.required_max_entries_per_partition,
            configured_max_entries: Some(max_entries),
        }
    }
}

/// Identifier-free result of comparing one reviewed deduplication configuration with one
/// caller-supplied recovery horizon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IggyDedupRecoveryWindowAssessment {
    status: IggyDedupRecoveryWindowStatus,
    required_expiry: Duration,
    configured_expiry: Option<Duration>,
    required_max_entries_per_partition: u64,
    configured_max_entries: Option<u64>,
}

impl IggyDedupRecoveryWindowAssessment {
    pub const fn status(&self) -> IggyDedupRecoveryWindowStatus {
        self.status
    }

    pub const fn required_expiry(&self) -> Duration {
        self.required_expiry
    }

    pub const fn configured_expiry(&self) -> Option<Duration> {
        self.configured_expiry
    }

    pub const fn required_max_entries_per_partition(&self) -> u64 {
        self.required_max_entries_per_partition
    }

    pub const fn configured_max_entries(&self) -> Option<u64> {
        self.configured_max_entries
    }

    pub const fn is_sufficient(&self) -> bool {
        matches!(self.status, IggyDedupRecoveryWindowStatus::Sufficient)
    }

    pub const fn requires_operator_action(&self) -> bool {
        !self.is_sufficient()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IggyDedupRecoveryWindowStatus {
    Disabled,
    InsufficientExpiry,
    InsufficientCapacity,
    InsufficientExpiryAndCapacity,
    Sufficient,
}

impl IggyDedupRecoveryWindowStatus {
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Disabled => "iggy.dedup_recovery.disabled",
            Self::InsufficientExpiry => "iggy.dedup_recovery.insufficient_expiry",
            Self::InsufficientCapacity => "iggy.dedup_recovery.insufficient_capacity",
            Self::InsufficientExpiryAndCapacity => {
                "iggy.dedup_recovery.insufficient_expiry_and_capacity"
            }
            Self::Sufficient => "iggy.dedup_recovery.sufficient",
        }
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum IggyDedupRecoveryWindowPolicyError {
    #[error("Iggy deduplication recovery-window policy is invalid")]
    InvalidPolicy,
    #[error("reviewed Iggy deduplication configuration is invalid")]
    InvalidConfiguration,
    #[error("Iggy deduplication recovery horizon overflows Duration")]
    RecoveryHorizonOverflow,
}

impl IggyDedupRecoveryWindowPolicyError {
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::InvalidPolicy => "iggy.dedup_recovery.policy_invalid",
            Self::InvalidConfiguration => "iggy.dedup_recovery.configuration_invalid",
            Self::RecoveryHorizonOverflow => "iggy.dedup_recovery.horizon_overflow",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> IggyDedupRecoveryWindowPolicy {
        IggyDedupRecoveryWindowPolicy::new(
            Duration::from_secs(30),
            Duration::from_secs(20),
            Duration::from_secs(10),
            Duration::from_secs(60),
            500,
        )
        .unwrap()
    }

    #[test]
    fn invalid_policy_and_configuration_fail_closed() {
        assert_eq!(
            IggyDedupRecoveryWindowPolicy::new(
                Duration::ZERO,
                Duration::ZERO,
                Duration::ZERO,
                Duration::ZERO,
                1,
            )
            .unwrap_err(),
            IggyDedupRecoveryWindowPolicyError::InvalidPolicy
        );
        assert_eq!(
            IggyDedupRecoveryWindowPolicy::new(
                Duration::from_secs(1),
                Duration::ZERO,
                Duration::ZERO,
                Duration::ZERO,
                0,
            )
            .unwrap_err(),
            IggyDedupRecoveryWindowPolicyError::InvalidPolicy
        );
        assert_eq!(
            IggyDeduplicationConfiguration::enabled(0, Duration::from_secs(1)).unwrap_err(),
            IggyDedupRecoveryWindowPolicyError::InvalidConfiguration
        );
        assert_eq!(
            IggyDeduplicationConfiguration::enabled(1, Duration::ZERO).unwrap_err(),
            IggyDedupRecoveryWindowPolicyError::InvalidConfiguration
        );
    }

    #[test]
    fn recovery_horizon_overflow_fails_closed() {
        let error = IggyDedupRecoveryWindowPolicy::new(
            Duration::MAX,
            Duration::from_secs(1),
            Duration::ZERO,
            Duration::ZERO,
            1,
        )
        .unwrap_err();

        assert_eq!(
            error,
            IggyDedupRecoveryWindowPolicyError::RecoveryHorizonOverflow
        );
        assert_eq!(error.stable_code(), "iggy.dedup_recovery.horizon_overflow");
    }

    #[test]
    fn disabled_configuration_never_claims_sufficiency() {
        let assessment = policy().assess(IggyDeduplicationConfiguration::disabled());

        assert_eq!(assessment.status(), IggyDedupRecoveryWindowStatus::Disabled);
        assert!(!assessment.is_sufficient());
        assert!(assessment.requires_operator_action());
        assert_eq!(assessment.required_expiry(), Duration::from_secs(120));
        assert_eq!(assessment.configured_expiry(), None);
        assert_eq!(assessment.configured_max_entries(), None);
    }

    #[test]
    fn expiry_and_capacity_deficits_are_distinguished() {
        let policy = policy();
        let expiry = policy.assess(
            IggyDeduplicationConfiguration::enabled(500, Duration::from_secs(119)).unwrap(),
        );
        let capacity = policy.assess(
            IggyDeduplicationConfiguration::enabled(499, Duration::from_secs(120)).unwrap(),
        );
        let both = policy.assess(
            IggyDeduplicationConfiguration::enabled(499, Duration::from_secs(119)).unwrap(),
        );

        assert_eq!(
            expiry.status(),
            IggyDedupRecoveryWindowStatus::InsufficientExpiry
        );
        assert_eq!(
            capacity.status(),
            IggyDedupRecoveryWindowStatus::InsufficientCapacity
        );
        assert_eq!(
            both.status(),
            IggyDedupRecoveryWindowStatus::InsufficientExpiryAndCapacity
        );
        assert_eq!(
            both.status().stable_code(),
            "iggy.dedup_recovery.insufficient_expiry_and_capacity"
        );
    }

    #[test]
    fn exact_boundary_is_sufficient_without_stronger_guarantees() {
        let policy = policy();
        let assessment = policy.assess(
            IggyDeduplicationConfiguration::enabled(500, Duration::from_secs(120)).unwrap(),
        );

        assert_eq!(
            assessment.status(),
            IggyDedupRecoveryWindowStatus::Sufficient
        );
        assert!(assessment.is_sufficient());
        assert!(!assessment.requires_operator_action());
        assert_eq!(assessment.required_max_entries_per_partition(), 500);
        assert_eq!(assessment.configured_max_entries(), Some(500));
        assert_eq!(
            assessment.configured_expiry(),
            Some(Duration::from_secs(120))
        );
        assert_eq!(
            assessment.status().stable_code(),
            "iggy.dedup_recovery.sufficient"
        );
    }
}
