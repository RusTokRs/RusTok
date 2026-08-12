use sha2::{Digest, Sha256};
use thiserror::Error;

const BASIS_POINTS: u32 = 10_000;
const MAX_PARTITION_MODULUS: u16 = 128;
const EVIDENCE_ID_HEX_BYTES: usize = 64;
const SHADOW_PLAN_VERSION: &str = "tenant_hash_shadow_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionStrategy {
    TenantHash { modulus: u16 },
}

impl PartitionStrategy {
    pub fn tenant_hash(modulus: u16) -> Result<Self, PartitionAdmissionError> {
        validate_modulus(modulus)?;
        Ok(Self::TenantHash { modulus })
    }

    pub const fn modulus(self) -> u16 {
        match self {
            Self::TenantHash { modulus } => modulus,
        }
    }

    const fn tag(self) -> &'static str {
        match self {
            Self::TenantHash { .. } => "tenant_hash",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionAdmissionPolicy {
    minimum_total_rows: u64,
    minimum_total_bytes: u64,
    minimum_distinct_tenants: u64,
    required_tenant_predicate_coverage_bps: u32,
    maximum_query_p95_regression_bps: u32,
    maximum_mutation_p95_regression_bps: u32,
    maximum_wal_amplification_bps: u32,
    maximum_partition_size_to_mean_bps: u32,
    maximum_cutover_lock_ms: u64,
}

impl PartitionAdmissionPolicy {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        minimum_total_rows: u64,
        minimum_total_bytes: u64,
        minimum_distinct_tenants: u64,
        required_tenant_predicate_coverage_bps: u32,
        maximum_query_p95_regression_bps: u32,
        maximum_mutation_p95_regression_bps: u32,
        maximum_wal_amplification_bps: u32,
        maximum_partition_size_to_mean_bps: u32,
        maximum_cutover_lock_ms: u64,
    ) -> Result<Self, PartitionAdmissionError> {
        if minimum_total_rows == 0 {
            return Err(PartitionAdmissionError::InvalidPolicy(
                "minimum_total_rows must be positive",
            ));
        }
        if minimum_total_bytes == 0 {
            return Err(PartitionAdmissionError::InvalidPolicy(
                "minimum_total_bytes must be positive",
            ));
        }
        if minimum_distinct_tenants < 2 {
            return Err(PartitionAdmissionError::InvalidPolicy(
                "minimum_distinct_tenants must be at least two",
            ));
        }
        if required_tenant_predicate_coverage_bps != BASIS_POINTS {
            return Err(PartitionAdmissionError::InvalidPolicy(
                "partition admission requires 10000 basis points of tenant predicate coverage",
            ));
        }
        if maximum_wal_amplification_bps < BASIS_POINTS {
            return Err(PartitionAdmissionError::InvalidPolicy(
                "maximum WAL amplification must allow the 10000 basis-point parity value",
            ));
        }
        if maximum_partition_size_to_mean_bps < BASIS_POINTS {
            return Err(PartitionAdmissionError::InvalidPolicy(
                "maximum partition skew must allow the 10000 basis-point parity value",
            ));
        }
        if maximum_cutover_lock_ms == 0 {
            return Err(PartitionAdmissionError::InvalidPolicy(
                "maximum_cutover_lock_ms must be positive",
            ));
        }
        Ok(Self {
            minimum_total_rows,
            minimum_total_bytes,
            minimum_distinct_tenants,
            required_tenant_predicate_coverage_bps,
            maximum_query_p95_regression_bps,
            maximum_mutation_p95_regression_bps,
            maximum_wal_amplification_bps,
            maximum_partition_size_to_mean_bps,
            maximum_cutover_lock_ms,
        })
    }

    pub const fn minimum_total_rows(&self) -> u64 {
        self.minimum_total_rows
    }

    pub const fn minimum_total_bytes(&self) -> u64 {
        self.minimum_total_bytes
    }

    pub const fn minimum_distinct_tenants(&self) -> u64 {
        self.minimum_distinct_tenants
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionBaselineEvidence {
    entity_rows: u64,
    link_rows: u64,
    entity_bytes: u64,
    link_bytes: u64,
    distinct_tenants: u64,
    tenant_predicate_coverage_bps: u32,
}

impl PartitionBaselineEvidence {
    pub fn new(
        entity_rows: u64,
        link_rows: u64,
        entity_bytes: u64,
        link_bytes: u64,
        distinct_tenants: u64,
        tenant_predicate_coverage_bps: u32,
    ) -> Result<Self, PartitionAdmissionError> {
        if entity_rows == 0 {
            return Err(PartitionAdmissionError::InvalidEvidence(
                "entity_rows must be positive",
            ));
        }
        if entity_bytes == 0 {
            return Err(PartitionAdmissionError::InvalidEvidence(
                "entity_bytes must be positive",
            ));
        }
        if distinct_tenants == 0 {
            return Err(PartitionAdmissionError::InvalidEvidence(
                "distinct_tenants must be positive",
            ));
        }
        if tenant_predicate_coverage_bps > BASIS_POINTS {
            return Err(PartitionAdmissionError::InvalidEvidence(
                "tenant predicate coverage must not exceed 10000 basis points",
            ));
        }
        Ok(Self {
            entity_rows,
            link_rows,
            entity_bytes,
            link_bytes,
            distinct_tenants,
            tenant_predicate_coverage_bps,
        })
    }

    pub fn total_rows(&self) -> Result<u64, PartitionAdmissionError> {
        self.entity_rows
            .checked_add(self.link_rows)
            .ok_or(PartitionAdmissionError::MetricOverflow("total rows"))
    }

    pub fn total_bytes(&self) -> Result<u64, PartitionAdmissionError> {
        self.entity_bytes
            .checked_add(self.link_bytes)
            .ok_or(PartitionAdmissionError::MetricOverflow("total bytes"))
    }

    pub const fn distinct_tenants(&self) -> u64 {
        self.distinct_tenants
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartitionMeasurementCoverage {
    query_runs: u32,
    mutation_runs: u32,
    maintenance_runs: u32,
    cutover_rehearsals: u32,
}

impl PartitionMeasurementCoverage {
    pub const fn new(
        query_runs: u32,
        mutation_runs: u32,
        maintenance_runs: u32,
        cutover_rehearsals: u32,
    ) -> Self {
        Self {
            query_runs,
            mutation_runs,
            maintenance_runs,
            cutover_rehearsals,
        }
    }

    pub const fn query_runs(self) -> u32 {
        self.query_runs
    }

    pub const fn mutation_runs(self) -> u32 {
        self.mutation_runs
    }

    pub const fn maintenance_runs(self) -> u32 {
        self.maintenance_runs
    }

    pub const fn cutover_rehearsals(self) -> u32 {
        self.cutover_rehearsals
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionShadowEvidence {
    evidence_id: String,
    strategy: PartitionStrategy,
    measurement_coverage: PartitionMeasurementCoverage,
    entity_digest_matches: bool,
    link_digest_matches: bool,
    shadow_caught_up: bool,
    foreign_keys_validated: bool,
    orphan_links: u64,
    query_plan_regressions: u32,
    query_p95_regression_bps: u32,
    mutation_p95_regression_bps: u32,
    wal_amplification_bps: u32,
    maximum_partition_size_to_mean_bps: u32,
    cutover_lock_ms: u64,
}

impl PartitionShadowEvidence {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        evidence_id: impl Into<String>,
        strategy: PartitionStrategy,
        measurement_coverage: PartitionMeasurementCoverage,
        entity_digest_matches: bool,
        link_digest_matches: bool,
        shadow_caught_up: bool,
        foreign_keys_validated: bool,
        orphan_links: u64,
        query_plan_regressions: u32,
        query_p95_regression_bps: u32,
        mutation_p95_regression_bps: u32,
        wal_amplification_bps: u32,
        maximum_partition_size_to_mean_bps: u32,
        cutover_lock_ms: u64,
    ) -> Result<Self, PartitionAdmissionError> {
        let evidence_id = evidence_id.into();
        validate_evidence_id(&evidence_id)?;
        if wal_amplification_bps == 0 {
            return Err(PartitionAdmissionError::InvalidEvidence(
                "WAL amplification must use 10000 basis points as parity",
            ));
        }
        if maximum_partition_size_to_mean_bps == 0 {
            return Err(PartitionAdmissionError::InvalidEvidence(
                "partition size skew must use 10000 basis points as parity",
            ));
        }
        Ok(Self {
            evidence_id,
            strategy,
            measurement_coverage,
            entity_digest_matches,
            link_digest_matches,
            shadow_caught_up,
            foreign_keys_validated,
            orphan_links,
            query_plan_regressions,
            query_p95_regression_bps,
            mutation_p95_regression_bps,
            wal_amplification_bps,
            maximum_partition_size_to_mean_bps,
            cutover_lock_ms,
        })
    }

    pub fn evidence_id(&self) -> &str {
        &self.evidence_id
    }

    pub const fn strategy(&self) -> PartitionStrategy {
        self.strategy
    }

    pub const fn measurement_coverage(&self) -> PartitionMeasurementCoverage {
        self.measurement_coverage
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionEvidence {
    baseline: PartitionBaselineEvidence,
    shadow: PartitionShadowEvidence,
}

impl PartitionEvidence {
    pub const fn new(baseline: PartitionBaselineEvidence, shadow: PartitionShadowEvidence) -> Self {
        Self { baseline, shadow }
    }

    pub const fn baseline(&self) -> &PartitionBaselineEvidence {
        &self.baseline
    }

    pub const fn shadow(&self) -> &PartitionShadowEvidence {
        &self.shadow
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartitionAdmissionReason {
    BelowMinimumRows { actual: u64, minimum: u64 },
    BelowMinimumBytes { actual: u64, minimum: u64 },
    InsufficientDistinctTenants { actual: u64, minimum: u64 },
    InsufficientTenantsForModulus { actual: u64, modulus: u16 },
    TenantPredicateCoverage { actual_bps: u32, required_bps: u32 },
    MissingQueryMeasurements,
    MissingMutationMeasurements,
    MissingMaintenanceMeasurements,
    MissingCutoverRehearsal,
    EntityDigestMismatch,
    LinkDigestMismatch,
    ShadowNotCaughtUp,
    ForeignKeysNotValidated,
    OrphanLinks { count: u64 },
    QueryPlanRegressions { count: u32 },
    QueryLatencyRegression { actual_bps: u32, maximum_bps: u32 },
    MutationLatencyRegression { actual_bps: u32, maximum_bps: u32 },
    WalAmplification { actual_bps: u32, maximum_bps: u32 },
    PartitionSizeSkew { actual_bps: u32, maximum_bps: u32 },
    CutoverLockExceeded { actual_ms: u64, maximum_ms: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartitionAdmissionOutcome {
    KeepUnpartitioned {
        reasons: Vec<PartitionAdmissionReason>,
    },
    Admitted(PartitionShadowPlan),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionRelationPlan {
    parent_name: String,
    partition_names: Vec<String>,
}

impl PartitionRelationPlan {
    pub fn parent_name(&self) -> &str {
        &self.parent_name
    }

    pub fn partition_names(&self) -> &[String] {
        &self.partition_names
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionShadowPlan {
    evidence_id: String,
    strategy: PartitionStrategy,
    definition_hash: String,
    entities: PartitionRelationPlan,
    links: PartitionRelationPlan,
}

impl PartitionShadowPlan {
    fn new(shadow: &PartitionShadowEvidence) -> Self {
        let definition = format!(
            "rustok-index-partition\u{1f}{SHADOW_PLAN_VERSION}\u{1f}{}\u{1f}{}\u{1f}{}",
            shadow.evidence_id,
            shadow.strategy.tag(),
            shadow.strategy.modulus(),
        );
        let definition_hash = hex::encode(Sha256::digest(definition.as_bytes()));
        let suffix = &definition_hash[..24];
        let modulus = shadow.strategy.modulus();
        let entities_parent = format!("index_entities_shadow_{suffix}");
        let links_parent = format!("index_links_shadow_{suffix}");
        Self {
            evidence_id: shadow.evidence_id.clone(),
            strategy: shadow.strategy,
            definition_hash,
            entities: relation_plan(entities_parent, modulus),
            links: relation_plan(links_parent, modulus),
        }
    }

    pub fn evidence_id(&self) -> &str {
        &self.evidence_id
    }

    pub const fn strategy(&self) -> PartitionStrategy {
        self.strategy
    }

    pub fn definition_hash(&self) -> &str {
        &self.definition_hash
    }

    pub const fn entities(&self) -> &PartitionRelationPlan {
        &self.entities
    }

    pub const fn links(&self) -> &PartitionRelationPlan {
        &self.links
    }

    pub fn bootstrap_statements(&self) -> Vec<String> {
        let mut statements = Vec::with_capacity(2 + usize::from(self.strategy.modulus()) * 2);
        statements.push(shadow_parent_statement(
            "index_entities",
            self.entities.parent_name(),
        ));
        statements.push(shadow_parent_statement(
            "index_links",
            self.links.parent_name(),
        ));
        for (remainder, name) in self.entities.partition_names().iter().enumerate() {
            statements.push(shadow_partition_statement(
                self.entities.parent_name(),
                name,
                self.strategy.modulus(),
                remainder,
            ));
        }
        for (remainder, name) in self.links.partition_names().iter().enumerate() {
            statements.push(shadow_partition_statement(
                self.links.parent_name(),
                name,
                self.strategy.modulus(),
                remainder,
            ));
        }
        statements
    }
}

pub fn evaluate_partition_admission(
    policy: &PartitionAdmissionPolicy,
    evidence: &PartitionEvidence,
) -> Result<PartitionAdmissionOutcome, PartitionAdmissionError> {
    let total_rows = evidence.baseline.total_rows()?;
    let total_bytes = evidence.baseline.total_bytes()?;
    let coverage = evidence.shadow.measurement_coverage;
    let mut reasons = Vec::new();

    if total_rows < policy.minimum_total_rows {
        reasons.push(PartitionAdmissionReason::BelowMinimumRows {
            actual: total_rows,
            minimum: policy.minimum_total_rows,
        });
    }
    if total_bytes < policy.minimum_total_bytes {
        reasons.push(PartitionAdmissionReason::BelowMinimumBytes {
            actual: total_bytes,
            minimum: policy.minimum_total_bytes,
        });
    }
    if evidence.baseline.distinct_tenants < policy.minimum_distinct_tenants {
        reasons.push(PartitionAdmissionReason::InsufficientDistinctTenants {
            actual: evidence.baseline.distinct_tenants,
            minimum: policy.minimum_distinct_tenants,
        });
    }
    if evidence.baseline.distinct_tenants < u64::from(evidence.shadow.strategy.modulus()) {
        reasons.push(PartitionAdmissionReason::InsufficientTenantsForModulus {
            actual: evidence.baseline.distinct_tenants,
            modulus: evidence.shadow.strategy.modulus(),
        });
    }
    if evidence.baseline.tenant_predicate_coverage_bps
        < policy.required_tenant_predicate_coverage_bps
    {
        reasons.push(PartitionAdmissionReason::TenantPredicateCoverage {
            actual_bps: evidence.baseline.tenant_predicate_coverage_bps,
            required_bps: policy.required_tenant_predicate_coverage_bps,
        });
    }
    if coverage.query_runs == 0 {
        reasons.push(PartitionAdmissionReason::MissingQueryMeasurements);
    }
    if coverage.mutation_runs == 0 {
        reasons.push(PartitionAdmissionReason::MissingMutationMeasurements);
    }
    if coverage.maintenance_runs == 0 {
        reasons.push(PartitionAdmissionReason::MissingMaintenanceMeasurements);
    }
    if coverage.cutover_rehearsals == 0 {
        reasons.push(PartitionAdmissionReason::MissingCutoverRehearsal);
    }
    if !evidence.shadow.entity_digest_matches {
        reasons.push(PartitionAdmissionReason::EntityDigestMismatch);
    }
    if !evidence.shadow.link_digest_matches {
        reasons.push(PartitionAdmissionReason::LinkDigestMismatch);
    }
    if !evidence.shadow.shadow_caught_up {
        reasons.push(PartitionAdmissionReason::ShadowNotCaughtUp);
    }
    if !evidence.shadow.foreign_keys_validated {
        reasons.push(PartitionAdmissionReason::ForeignKeysNotValidated);
    }
    if evidence.shadow.orphan_links != 0 {
        reasons.push(PartitionAdmissionReason::OrphanLinks {
            count: evidence.shadow.orphan_links,
        });
    }
    if evidence.shadow.query_plan_regressions != 0 {
        reasons.push(PartitionAdmissionReason::QueryPlanRegressions {
            count: evidence.shadow.query_plan_regressions,
        });
    }
    if evidence.shadow.query_p95_regression_bps > policy.maximum_query_p95_regression_bps {
        reasons.push(PartitionAdmissionReason::QueryLatencyRegression {
            actual_bps: evidence.shadow.query_p95_regression_bps,
            maximum_bps: policy.maximum_query_p95_regression_bps,
        });
    }
    if evidence.shadow.mutation_p95_regression_bps > policy.maximum_mutation_p95_regression_bps {
        reasons.push(PartitionAdmissionReason::MutationLatencyRegression {
            actual_bps: evidence.shadow.mutation_p95_regression_bps,
            maximum_bps: policy.maximum_mutation_p95_regression_bps,
        });
    }
    if evidence.shadow.wal_amplification_bps > policy.maximum_wal_amplification_bps {
        reasons.push(PartitionAdmissionReason::WalAmplification {
            actual_bps: evidence.shadow.wal_amplification_bps,
            maximum_bps: policy.maximum_wal_amplification_bps,
        });
    }
    if evidence.shadow.maximum_partition_size_to_mean_bps
        > policy.maximum_partition_size_to_mean_bps
    {
        reasons.push(PartitionAdmissionReason::PartitionSizeSkew {
            actual_bps: evidence.shadow.maximum_partition_size_to_mean_bps,
            maximum_bps: policy.maximum_partition_size_to_mean_bps,
        });
    }
    if evidence.shadow.cutover_lock_ms > policy.maximum_cutover_lock_ms {
        reasons.push(PartitionAdmissionReason::CutoverLockExceeded {
            actual_ms: evidence.shadow.cutover_lock_ms,
            maximum_ms: policy.maximum_cutover_lock_ms,
        });
    }

    if reasons.is_empty() {
        Ok(PartitionAdmissionOutcome::Admitted(
            PartitionShadowPlan::new(&evidence.shadow),
        ))
    } else {
        Ok(PartitionAdmissionOutcome::KeepUnpartitioned { reasons })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PartitionAdmissionError {
    #[error("invalid partition admission policy: {0}")]
    InvalidPolicy(&'static str),
    #[error("invalid partition evidence: {0}")]
    InvalidEvidence(&'static str),
    #[error("partition metric overflow: {0}")]
    MetricOverflow(&'static str),
    #[error("partition modulus must be a power of two between 2 and 128")]
    InvalidModulus,
}

fn validate_modulus(modulus: u16) -> Result<(), PartitionAdmissionError> {
    if !(2..=MAX_PARTITION_MODULUS).contains(&modulus) || !modulus.is_power_of_two() {
        return Err(PartitionAdmissionError::InvalidModulus);
    }
    Ok(())
}

fn validate_evidence_id(evidence_id: &str) -> Result<(), PartitionAdmissionError> {
    if evidence_id.len() != EVIDENCE_ID_HEX_BYTES
        || !evidence_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(PartitionAdmissionError::InvalidEvidence(
            "evidence_id must be a lowercase 64-character SHA-256 hex digest",
        ));
    }
    Ok(())
}

fn relation_plan(parent_name: String, modulus: u16) -> PartitionRelationPlan {
    let partition_names = (0..modulus)
        .map(|remainder| format!("{parent_name}_p{remainder:03}"))
        .collect();
    PartitionRelationPlan {
        parent_name,
        partition_names,
    }
}

fn shadow_parent_statement(source: &str, shadow: &str) -> String {
    format!(
        "CREATE TABLE {} (LIKE {} INCLUDING DEFAULTS INCLUDING GENERATED INCLUDING IDENTITY INCLUDING STORAGE INCLUDING COMMENTS) PARTITION BY HASH (tenant_id)",
        quote_identifier(shadow),
        quote_identifier(source),
    )
}

fn shadow_partition_statement(
    parent: &str,
    partition: &str,
    modulus: u16,
    remainder: usize,
) -> String {
    format!(
        "CREATE TABLE {} PARTITION OF {} FOR VALUES WITH (MODULUS {modulus}, REMAINDER {remainder})",
        quote_identifier(partition),
        quote_identifier(parent),
    )
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
