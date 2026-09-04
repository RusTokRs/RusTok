use sea_orm_migration::MigrationTrait;

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
    /// Additive-only changes (create table, add nullable column, concurrent index).
    /// Safe for live N/N+1 concurrent serving without exclusive table locks.
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
            MigrationSafetyClass::AdditiveOnly | MigrationSafetyClass::ExpandContract => (false, true),
            MigrationSafetyClass::MaintenanceOnly | MigrationSafetyClass::Irreversible => (true, false),
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

