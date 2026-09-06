use thiserror::Error;

use crate::dlq_duplicate_inspection::DlqDuplicateSummary;

/// Operator-selected count thresholds for a transport-neutral physical DLQ
/// duplicate alert evaluation.
///
/// The policy contains no broker coordinates, identifiers, payload facts,
/// credentials, delivery state, or destructive action. Callers must choose all
/// thresholds explicitly; this module does not define a production default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DlqDuplicateAlertPolicy {
    warning_duplicate_messages: u64,
    critical_duplicate_messages: u64,
    warning_duplicate_groups: u64,
    critical_duplicate_groups: u64,
    warning_max_copies_per_message_id: u64,
    critical_max_copies_per_message_id: u64,
}

impl DlqDuplicateAlertPolicy {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        warning_duplicate_messages: u64,
        critical_duplicate_messages: u64,
        warning_duplicate_groups: u64,
        critical_duplicate_groups: u64,
        warning_max_copies_per_message_id: u64,
        critical_max_copies_per_message_id: u64,
    ) -> Result<Self, DlqDuplicateAlertPolicyError> {
        if warning_duplicate_messages == 0
            || warning_duplicate_groups == 0
            || warning_max_copies_per_message_id < 2
            || critical_duplicate_messages < warning_duplicate_messages
            || critical_duplicate_groups < warning_duplicate_groups
            || critical_max_copies_per_message_id < warning_max_copies_per_message_id
        {
            return Err(DlqDuplicateAlertPolicyError::InvalidThresholds);
        }

        Ok(Self {
            warning_duplicate_messages,
            critical_duplicate_messages,
            warning_duplicate_groups,
            critical_duplicate_groups,
            warning_max_copies_per_message_id,
            critical_max_copies_per_message_id,
        })
    }

    pub const fn evaluate(&self, summary: &DlqDuplicateSummary) -> DlqDuplicateAlertEvaluation {
        let identity_conflict = summary.has_identity_conflicts();
        let critical_duplicate_messages =
            summary.duplicate_messages() >= self.critical_duplicate_messages;
        let critical_duplicate_groups =
            summary.duplicate_groups() >= self.critical_duplicate_groups;
        let critical_max_copies =
            summary.max_copies_per_message_id() >= self.critical_max_copies_per_message_id;

        if identity_conflict
            || critical_duplicate_messages
            || critical_duplicate_groups
            || critical_max_copies
        {
            return DlqDuplicateAlertEvaluation {
                level: DlqDuplicateAlertLevel::Critical,
                physical_duplicates: summary.has_physical_duplicates(),
                identity_conflict,
                duplicate_messages_threshold_reached: critical_duplicate_messages,
                duplicate_groups_threshold_reached: critical_duplicate_groups,
                max_copies_threshold_reached: critical_max_copies,
            };
        }

        let warning_duplicate_messages =
            summary.duplicate_messages() >= self.warning_duplicate_messages;
        let warning_duplicate_groups = summary.duplicate_groups() >= self.warning_duplicate_groups;
        let warning_max_copies =
            summary.max_copies_per_message_id() >= self.warning_max_copies_per_message_id;

        if warning_duplicate_messages || warning_duplicate_groups || warning_max_copies {
            return DlqDuplicateAlertEvaluation {
                level: DlqDuplicateAlertLevel::Warning,
                physical_duplicates: summary.has_physical_duplicates(),
                identity_conflict: false,
                duplicate_messages_threshold_reached: warning_duplicate_messages,
                duplicate_groups_threshold_reached: warning_duplicate_groups,
                max_copies_threshold_reached: warning_max_copies,
            };
        }

        if summary.has_physical_duplicates() {
            return DlqDuplicateAlertEvaluation {
                level: DlqDuplicateAlertLevel::Notice,
                physical_duplicates: true,
                identity_conflict: false,
                duplicate_messages_threshold_reached: false,
                duplicate_groups_threshold_reached: false,
                max_copies_threshold_reached: false,
            };
        }

        DlqDuplicateAlertEvaluation {
            level: DlqDuplicateAlertLevel::Clear,
            physical_duplicates: false,
            identity_conflict: false,
            duplicate_messages_threshold_reached: false,
            duplicate_groups_threshold_reached: false,
            max_copies_threshold_reached: false,
        }
    }
}

/// Count-only alert level. Delivery channel, paging, suppression, and escalation
/// timing remain caller-owned policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DlqDuplicateAlertLevel {
    Clear,
    Notice,
    Warning,
    Critical,
}

impl DlqDuplicateAlertLevel {
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::Clear => "iggy.dlq_duplicate.alert.clear",
            Self::Notice => "iggy.dlq_duplicate.alert.notice",
            Self::Warning => "iggy.dlq_duplicate.alert.warning",
            Self::Critical => "iggy.dlq_duplicate.alert.critical",
        }
    }
}

/// Identifier-free result of evaluating one count-only duplicate summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DlqDuplicateAlertEvaluation {
    level: DlqDuplicateAlertLevel,
    physical_duplicates: bool,
    identity_conflict: bool,
    duplicate_messages_threshold_reached: bool,
    duplicate_groups_threshold_reached: bool,
    max_copies_threshold_reached: bool,
}

impl DlqDuplicateAlertEvaluation {
    pub const fn level(&self) -> DlqDuplicateAlertLevel {
        self.level
    }

    pub const fn has_physical_duplicates(&self) -> bool {
        self.physical_duplicates
    }

    pub const fn has_identity_conflict(&self) -> bool {
        self.identity_conflict
    }

    pub const fn duplicate_messages_threshold_reached(&self) -> bool {
        self.duplicate_messages_threshold_reached
    }

    pub const fn duplicate_groups_threshold_reached(&self) -> bool {
        self.duplicate_groups_threshold_reached
    }

    pub const fn max_copies_threshold_reached(&self) -> bool {
        self.max_copies_threshold_reached
    }

    /// Conflicting exact bytes for one deterministic message ID always require
    /// separate manual investigation, independent of numeric thresholds.
    pub const fn requires_manual_review(&self) -> bool {
        self.identity_conflict
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum DlqDuplicateAlertPolicyError {
    #[error("physical DLQ duplicate alert thresholds are invalid")]
    InvalidThresholds,
}

impl DlqDuplicateAlertPolicyError {
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::InvalidThresholds => "iggy.dlq_duplicate.alert_policy_invalid",
        }
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::{DlqDuplicateObservation, summarize_dlq_duplicates};

    use super::*;

    fn policy() -> DlqDuplicateAlertPolicy {
        DlqDuplicateAlertPolicy::new(2, 4, 2, 3, 3, 5).unwrap()
    }

    fn summary(entries: &[(u128, &[u8])]) -> DlqDuplicateSummary {
        summarize_dlq_duplicates(entries.iter().map(|(id, payload)| {
            DlqDuplicateObservation::from_payload(Uuid::from_u128(*id), payload).unwrap()
        }))
        .unwrap()
    }

    #[test]
    fn invalid_threshold_ordering_fails_closed() {
        for candidate in [
            DlqDuplicateAlertPolicy::new(0, 1, 1, 1, 2, 2),
            DlqDuplicateAlertPolicy::new(2, 1, 1, 1, 2, 2),
            DlqDuplicateAlertPolicy::new(1, 1, 0, 1, 2, 2),
            DlqDuplicateAlertPolicy::new(1, 1, 2, 1, 2, 2),
            DlqDuplicateAlertPolicy::new(1, 1, 1, 1, 1, 2),
            DlqDuplicateAlertPolicy::new(1, 1, 1, 1, 3, 2),
        ] {
            let error = candidate.unwrap_err();
            assert_eq!(error, DlqDuplicateAlertPolicyError::InvalidThresholds);
            assert_eq!(
                error.stable_code(),
                "iggy.dlq_duplicate.alert_policy_invalid"
            );
        }
    }

    #[test]
    fn clear_and_notice_remain_below_operator_thresholds() {
        let clear = policy().evaluate(&summary(&[(1, &[1]), (2, &[2])]));
        assert_eq!(clear.level(), DlqDuplicateAlertLevel::Clear);
        assert!(!clear.has_physical_duplicates());

        let notice = policy().evaluate(&summary(&[(1, &[1]), (1, &[1]), (2, &[2])]));
        assert_eq!(notice.level(), DlqDuplicateAlertLevel::Notice);
        assert!(notice.has_physical_duplicates());
        assert!(!notice.duplicate_messages_threshold_reached());
        assert!(!notice.requires_manual_review());
    }

    #[test]
    fn warning_reports_only_reached_warning_dimensions() {
        let warning = policy().evaluate(&summary(&[
            (1, &[1]),
            (1, &[1]),
            (2, &[2]),
            (2, &[2]),
            (3, &[3]),
        ]));

        assert_eq!(warning.level(), DlqDuplicateAlertLevel::Warning);
        assert!(warning.duplicate_messages_threshold_reached());
        assert!(warning.duplicate_groups_threshold_reached());
        assert!(!warning.max_copies_threshold_reached());
        assert!(!warning.has_identity_conflict());
    }

    #[test]
    fn critical_numeric_threshold_takes_precedence() {
        let critical = policy().evaluate(&summary(&[
            (1, &[1]),
            (1, &[1]),
            (1, &[1]),
            (1, &[1]),
            (1, &[1]),
        ]));

        assert_eq!(critical.level(), DlqDuplicateAlertLevel::Critical);
        assert!(critical.duplicate_messages_threshold_reached());
        assert!(critical.max_copies_threshold_reached());
        assert!(!critical.duplicate_groups_threshold_reached());
        assert!(!critical.requires_manual_review());
    }

    #[test]
    fn identity_conflict_is_always_critical_and_manual() {
        let critical = DlqDuplicateAlertPolicy::new(10, 20, 10, 20, 10, 20)
            .unwrap()
            .evaluate(&summary(&[(7, &[1]), (7, &[2])]));

        assert_eq!(critical.level(), DlqDuplicateAlertLevel::Critical);
        assert!(critical.has_physical_duplicates());
        assert!(critical.has_identity_conflict());
        assert!(critical.requires_manual_review());
        assert!(!critical.duplicate_messages_threshold_reached());
        assert_eq!(
            critical.level().stable_code(),
            "iggy.dlq_duplicate.alert.critical"
        );
    }
}
