//! Shared desired-state and observed-state primitives for module-owned node
//! reconciliation. Concrete rollout owners retain their own artifact and
//! topology identities while using this one lifecycle vocabulary.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Durable head pointers for a revisioned reconciliation owner.
///
/// A concrete owner persists these pointers in its own aggregate table. The
/// identifiers deliberately carry no artifact, topology, or node semantics so
/// the same contract can represent static distributions and future sandbox
/// assignments without a second desired/observed model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleDesiredObservedState {
    pub revision: u64,
    pub desired_id: Option<Uuid>,
    pub observed_id: Option<Uuid>,
}

impl ModuleDesiredObservedState {
    /// Returns the converged identity only when an explicit desired target and
    /// the latest observed target are identical.
    pub fn converged_id(&self) -> Option<Uuid> {
        self.desired_id
            .filter(|desired_id| Some(*desired_id) == self.observed_id)
    }
}

/// Common lifecycle phase reported by a node for one owner-assigned target.
///
/// `Active` is deliberately not a standard forward transition: the owning
/// reconciler admits it only after its aggregate convergence policy is met.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleReconciliationPhase {
    Pending,
    Prepared,
    Healthy,
    Active,
    Failed,
}

impl ModuleReconciliationPhase {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Prepared => "prepared",
            Self::Healthy => "healthy",
            Self::Active => "active",
            Self::Failed => "failed",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "prepared" => Some(Self::Prepared),
            "healthy" => Some(Self::Healthy),
            "active" => Some(Self::Active),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }

    /// Checks the phase/evidence shape before an owner-specific validator
    /// evaluates the actual evidence references and failure details.
    pub(crate) const fn permits_report_payload(
        self,
        has_health_evidence: bool,
        has_failure: bool,
    ) -> bool {
        match self {
            Self::Pending => false,
            Self::Prepared => !has_health_evidence && !has_failure,
            Self::Healthy | Self::Active => has_health_evidence && !has_failure,
            Self::Failed => !has_health_evidence && has_failure,
        }
    }

    /// Baseline per-assignment transitions that do not require an aggregate
    /// decision. Owners may add explicit, state-gated transitions such as
    /// `healthy -> active` or a recovery retry from `failed`.
    pub(crate) const fn allows_standard_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Pending, Self::Prepared | Self::Failed)
                | (Self::Prepared, Self::Healthy | Self::Failed)
                | (Self::Healthy, Self::Failed)
                | (Self::Active, Self::Failed)
        )
    }
}

/// Immutable health evidence reported for one observed assignment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleReconciliationEvidence {
    pub reference: String,
    pub digest: String,
}

/// Structured failure reported for one observed assignment or aggregate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleReconciliationFailure {
    pub code: String,
    pub detail: String,
}

#[cfg(test)]
mod tests {
    use super::{ModuleDesiredObservedState, ModuleReconciliationPhase};
    use uuid::Uuid;

    #[test]
    fn state_converges_only_on_an_explicit_matching_target() {
        let target = Uuid::new_v4();
        assert_eq!(
            ModuleDesiredObservedState {
                revision: 4,
                desired_id: Some(target),
                observed_id: Some(target),
            }
            .converged_id(),
            Some(target)
        );
        assert_eq!(
            ModuleDesiredObservedState {
                revision: 5,
                desired_id: None,
                observed_id: None,
            }
            .converged_id(),
            None
        );
    }

    #[test]
    fn baseline_phase_contract_requires_owner_gated_activation() {
        assert!(
            ModuleReconciliationPhase::Pending
                .allows_standard_transition_to(ModuleReconciliationPhase::Prepared)
        );
        assert!(
            ModuleReconciliationPhase::Prepared
                .allows_standard_transition_to(ModuleReconciliationPhase::Healthy)
        );
        assert!(
            !ModuleReconciliationPhase::Healthy
                .allows_standard_transition_to(ModuleReconciliationPhase::Active)
        );
        assert!(ModuleReconciliationPhase::Healthy.permits_report_payload(true, false));
        assert!(ModuleReconciliationPhase::Failed.permits_report_payload(false, true));
        assert!(!ModuleReconciliationPhase::Pending.permits_report_payload(false, false));
        assert_eq!(
            ModuleReconciliationPhase::parse("active"),
            Some(ModuleReconciliationPhase::Active)
        );
        assert_eq!(ModuleReconciliationPhase::parse("unknown"), None);
    }
}
