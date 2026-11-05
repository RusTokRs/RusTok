use sea_orm_migration::MigrationTrait;

/// Migration facts shared by schema tooling and release preflight.
///
/// This assessment is not deployment authorization. Its producer must retain
/// the exact plan identity and the scope and limits of its supporting evidence.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "mode", content = "details", deny_unknown_fields)]
pub enum MigrationSafetyAssessment {
    AdditiveSafe,
    MaintenanceRequired {
        reasons: Vec<String>,
        locks_table: bool,
        is_destructive: bool,
    },
    Unknown {
        reasons: Vec<String>,
    },
}

impl MigrationSafetyAssessment {
    pub fn is_additive_safe(&self) -> bool {
        matches!(self, Self::AdditiveSafe)
    }

    /// Diagnostics are a projection; their absence never implies safety.
    pub fn denial_reasons(&self) -> Vec<String> {
        let (reasons, fallback) = match self {
            Self::AdditiveSafe => return Vec::new(),
            Self::MaintenanceRequired { reasons, .. } => {
                (reasons, "Migration requires maintenance")
            }
            Self::Unknown { reasons } => (reasons, "Migration safety evidence is incomplete"),
        };
        let reasons: Vec<_> = reasons
            .iter()
            .map(|reason| reason.trim())
            .filter(|reason| !reason.is_empty())
            .map(str::to_owned)
            .collect();
        if reasons.is_empty() {
            vec![fallback.to_owned()]
        } else {
            reasons
        }
    }
}

pub struct ModuleMigration {
    pub module_slug: &'static str,
    pub migrations: Vec<Box<dyn MigrationTrait>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationDependencyDescriptor {
    pub migration: &'static str,
    pub after: Vec<&'static str>,
}

impl MigrationDependencyDescriptor {
    pub fn new(migration: &'static str, after: Vec<&'static str>) -> Self {
        Self { migration, after }
    }
}

/// Safety classification of a native migration determining its live execution constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MigrationSafetyClass {
    /// Declares additive schema shape. This declaration alone proves neither
    /// lock duration, transaction behavior nor live N/N+1 compatibility;
    /// execution evidence is assessed separately before release admission.
    AdditiveOnly,
    /// Multi-phase migration (expand, backfill, contract).
    ExpandContract,
    /// Requires table/schema lock or data alteration incompatible with version N.
    /// Can only run during a quiesced maintenance window.
    MaintenanceOnly,
    /// Irreversible destructive change that cannot be rolled back without data loss.
    Irreversible,
}

/// Phase constraint for migration execution within a deployment rollout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MigrationPhaseConstraint {
    /// Must execute before candidate N+1 receives live traffic.
    PreActivation,
    /// Must execute only after all nodes have transitioned to N+1 and old N traffic has drained.
    PostActivation,
    /// Can only execute during a quiesced maintenance window.
    MaintenanceWindow,
}

/// Exact safety metadata required to produce a bounded migration phase plan.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MigrationSafetyMetadata {
    pub migration: &'static str,
    pub safety_class: MigrationSafetyClass,
    pub phase_constraint: MigrationPhaseConstraint,
    pub requires_exclusive_lock: bool,
    pub allows_concurrent_writes: bool,
    pub backfill_required: bool,
}

impl MigrationSafetyMetadata {
    pub fn new(
        migration: &'static str,
        safety_class: MigrationSafetyClass,
        phase_constraint: MigrationPhaseConstraint,
    ) -> Self {
        let (requires_exclusive_lock, allows_concurrent_writes) = match safety_class {
            MigrationSafetyClass::AdditiveOnly | MigrationSafetyClass::ExpandContract => {
                (false, true)
            }
            MigrationSafetyClass::MaintenanceOnly | MigrationSafetyClass::Irreversible => {
                (true, false)
            }
        };
        Self {
            migration,
            safety_class,
            phase_constraint,
            requires_exclusive_lock,
            allows_concurrent_writes,
            backfill_required: matches!(safety_class, MigrationSafetyClass::ExpandContract),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MigrationSafetyAssessment;

    #[test]
    fn missing_diagnostics_cannot_make_rejected_assessments_safe() {
        for reasons in [vec![], vec!["  \t".to_owned()]] {
            for assessment in [
                MigrationSafetyAssessment::MaintenanceRequired {
                    reasons: reasons.clone(),
                    locks_table: true,
                    is_destructive: false,
                },
                MigrationSafetyAssessment::Unknown { reasons },
            ] {
                assert!(!assessment.is_additive_safe());
                assert!(!assessment.denial_reasons().is_empty());
                let encoded = serde_json::to_value(&assessment).unwrap();
                let decoded: MigrationSafetyAssessment = serde_json::from_value(encoded).unwrap();
                assert_eq!(assessment, decoded);
            }
        }
    }

    #[test]
    fn incomplete_and_unknown_wire_states_are_rejected() {
        for value in [
            serde_json::json!({}),
            serde_json::json!({"mode": "SafeByDefault"}),
            serde_json::json!({"mode": "MaintenanceRequired", "details": {"reasons": []}}),
            serde_json::json!({"mode": "AdditiveSafe", "unexpected": true}),
            serde_json::json!({"mode": "AdditiveSafe", "details": {"locks_table": true}}),
        ] {
            assert!(serde_json::from_value::<MigrationSafetyAssessment>(value).is_err());
        }
    }
}
