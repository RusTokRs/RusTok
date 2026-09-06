use super::{
    PartitionAdmissionError, PartitionAdmissionOutcome, PartitionAdmissionPolicy,
    PartitionAdmissionReason, PartitionBaselineEvidence, PartitionEvidence,
    PartitionMeasurementCoverage, PartitionShadowEvidence, PartitionStrategy,
    evaluate_partition_admission,
};

const EVIDENCE_ID: &str = "1111111111111111111111111111111111111111111111111111111111111111";

fn policy() -> PartitionAdmissionPolicy {
    PartitionAdmissionPolicy::new(
        1_000_000,
        4 * 1024 * 1024 * 1024,
        16,
        10_000,
        500,
        500,
        11_000,
        15_000,
        250,
    )
    .unwrap()
}

fn baseline() -> PartitionBaselineEvidence {
    PartitionBaselineEvidence::new(
        1_000_000,
        3_000_000,
        4 * 1024 * 1024 * 1024,
        2 * 1024 * 1024 * 1024,
        64,
        10_000,
    )
    .unwrap()
}

fn shadow() -> PartitionShadowEvidence {
    PartitionShadowEvidence::new(
        EVIDENCE_ID,
        PartitionStrategy::tenant_hash(16).unwrap(),
        PartitionMeasurementCoverage::new(3, 3, 3, 1),
        true,
        true,
        true,
        true,
        0,
        0,
        100,
        200,
        10_500,
        12_000,
        100,
    )
    .unwrap()
}

#[test]
fn admitted_plan_is_stable_and_shadow_only() {
    let evidence = PartitionEvidence::new(baseline(), shadow());
    let first = evaluate_partition_admission(&policy(), &evidence).unwrap();
    let second = evaluate_partition_admission(&policy(), &evidence).unwrap();
    assert_eq!(first, second);

    let PartitionAdmissionOutcome::Admitted(plan) = first else {
        panic!("complete evidence should admit a shadow plan");
    };
    assert_eq!(plan.evidence_id(), EVIDENCE_ID);
    assert_eq!(plan.strategy().modulus(), 16);
    assert_eq!(plan.definition_hash().len(), 64);
    assert!(
        plan.entities()
            .parent_name()
            .starts_with("index_entities_shadow_")
    );
    assert!(
        plan.links()
            .parent_name()
            .starts_with("index_links_shadow_")
    );
    assert_eq!(plan.entities().partition_names().len(), 16);
    assert_eq!(plan.links().partition_names().len(), 16);

    let statements = plan.bootstrap_statements();
    assert_eq!(statements.len(), 34);
    assert!(statements[0].contains("PARTITION BY HASH (tenant_id)"));
    assert!(statements[1].contains("PARTITION BY HASH (tenant_id)"));
    assert!(
        statements
            .iter()
            .any(|sql| sql.contains("FOR VALUES WITH (MODULUS 16, REMAINDER 15)"))
    );
    assert!(statements.iter().all(|sql| !sql.contains("DROP TABLE")));
    assert!(statements.iter().all(|sql| !sql.contains("RENAME TO")));
    assert!(
        statements
            .iter()
            .all(|sql| !sql.starts_with("ALTER TABLE \"index_entities\""))
    );
    assert!(
        statements
            .iter()
            .all(|sql| !sql.starts_with("ALTER TABLE \"index_links\""))
    );
}

#[test]
fn incomplete_or_regressed_evidence_keeps_storage_unpartitioned() {
    let baseline = PartitionBaselineEvidence::new(100, 50, 1024, 512, 4, 9_500).unwrap();
    let shadow = PartitionShadowEvidence::new(
        EVIDENCE_ID,
        PartitionStrategy::tenant_hash(16).unwrap(),
        PartitionMeasurementCoverage::new(0, 0, 0, 0),
        false,
        false,
        false,
        false,
        7,
        2,
        800,
        900,
        12_000,
        20_000,
        500,
    )
    .unwrap();
    let outcome =
        evaluate_partition_admission(&policy(), &PartitionEvidence::new(baseline, shadow)).unwrap();
    let PartitionAdmissionOutcome::KeepUnpartitioned { reasons } = outcome else {
        panic!("incomplete evidence must fail closed");
    };

    assert!(
        reasons
            .iter()
            .any(|reason| matches!(reason, PartitionAdmissionReason::BelowMinimumRows { .. }))
    );
    assert!(
        reasons
            .iter()
            .any(|reason| matches!(reason, PartitionAdmissionReason::BelowMinimumBytes { .. }))
    );
    assert!(reasons.iter().any(|reason| matches!(
        reason,
        PartitionAdmissionReason::InsufficientDistinctTenants { .. }
    )));
    assert!(reasons.iter().any(|reason| matches!(
        reason,
        PartitionAdmissionReason::InsufficientTenantsForModulus { .. }
    )));
    assert!(reasons.iter().any(|reason| matches!(
        reason,
        PartitionAdmissionReason::TenantPredicateCoverage { .. }
    )));
    assert!(reasons.contains(&PartitionAdmissionReason::MissingQueryMeasurements));
    assert!(reasons.contains(&PartitionAdmissionReason::MissingMutationMeasurements));
    assert!(reasons.contains(&PartitionAdmissionReason::MissingMaintenanceMeasurements));
    assert!(reasons.contains(&PartitionAdmissionReason::MissingCutoverRehearsal));
    assert!(reasons.contains(&PartitionAdmissionReason::EntityDigestMismatch));
    assert!(reasons.contains(&PartitionAdmissionReason::LinkDigestMismatch));
    assert!(reasons.contains(&PartitionAdmissionReason::ShadowNotCaughtUp));
    assert!(reasons.contains(&PartitionAdmissionReason::ForeignKeysNotValidated));
    assert!(
        reasons
            .iter()
            .any(|reason| matches!(reason, PartitionAdmissionReason::OrphanLinks { count: 7 }))
    );
    assert!(reasons.iter().any(|reason| matches!(
        reason,
        PartitionAdmissionReason::QueryPlanRegressions { count: 2 }
    )));
    assert!(reasons.iter().any(|reason| matches!(
        reason,
        PartitionAdmissionReason::QueryLatencyRegression { .. }
    )));
    assert!(reasons.iter().any(|reason| matches!(
        reason,
        PartitionAdmissionReason::MutationLatencyRegression { .. }
    )));
    assert!(
        reasons
            .iter()
            .any(|reason| matches!(reason, PartitionAdmissionReason::WalAmplification { .. }))
    );
    assert!(
        reasons
            .iter()
            .any(|reason| matches!(reason, PartitionAdmissionReason::PartitionSizeSkew { .. }))
    );
    assert!(
        reasons
            .iter()
            .any(|reason| matches!(reason, PartitionAdmissionReason::CutoverLockExceeded { .. }))
    );
}

#[test]
fn policy_strategy_and_evidence_validation_fail_closed() {
    assert_eq!(
        PartitionStrategy::tenant_hash(3),
        Err(PartitionAdmissionError::InvalidModulus)
    );
    assert_eq!(
        PartitionStrategy::tenant_hash(256),
        Err(PartitionAdmissionError::InvalidModulus)
    );
    assert!(matches!(
        PartitionAdmissionPolicy::new(0, 1, 2, 10_000, 0, 0, 10_000, 10_000, 1),
        Err(PartitionAdmissionError::InvalidPolicy(_))
    ));
    assert!(matches!(
        PartitionAdmissionPolicy::new(1, 1, 2, 9_999, 0, 0, 10_000, 10_000, 1),
        Err(PartitionAdmissionError::InvalidPolicy(_))
    ));
    assert!(matches!(
        PartitionBaselineEvidence::new(1, 0, 1, 0, 1, 10_001),
        Err(PartitionAdmissionError::InvalidEvidence(_))
    ));
    assert!(matches!(
        PartitionShadowEvidence::new(
            "not-a-sha256",
            PartitionStrategy::tenant_hash(2).unwrap(),
            PartitionMeasurementCoverage::new(1, 1, 1, 1),
            true,
            true,
            true,
            true,
            0,
            0,
            0,
            0,
            10_000,
            10_000,
            1,
        ),
        Err(PartitionAdmissionError::InvalidEvidence(_))
    ));
}
