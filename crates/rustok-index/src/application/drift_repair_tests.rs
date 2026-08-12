use uuid::Uuid;

use crate::{EntityKey, EntityName, LocaleKey, ModuleName, SchemaRef, SchemaVersion};

use super::*;

fn entity_key(tenant_id: Uuid) -> EntityKey {
    EntityKey {
        tenant_id,
        schema: SchemaRef {
            module: ModuleName::new("catalog").expect("valid module"),
            entity: EntityName::new("product").expect("valid entity"),
            version: SchemaVersion::new(1),
        },
        entity_id: Uuid::from_u128(2),
        locale: Some(LocaleKey::new("en").expect("valid locale")),
    }
}

fn digest(byte: char) -> String {
    std::iter::repeat(byte).take(64).collect()
}

#[test]
fn repair_command_binds_exact_tenant_and_redacts_operator_text() {
    let tenant_id = Uuid::from_u128(1);
    let target =
        IndexDriftRepairTarget::missing_entity(entity_key(tenant_id), 7, 9).expect("valid target");
    let actor =
        IndexDriftFindingLifecycleActor::new("operator", "private-subject").expect("valid actor");
    let command = IndexDriftRepairCommand::new(
        tenant_id,
        Uuid::from_u128(3),
        Uuid::from_u128(4),
        target,
        actor,
        "private-reason",
    )
    .expect("valid command");

    let debug = format!("{command:?}");
    assert!(!debug.contains("private-subject"));
    assert!(!debug.contains("private-reason"));
    assert!(debug.contains("actor_subject_len"));
    assert!(debug.contains("reason_len"));
}

#[test]
fn repair_command_rejects_cross_tenant_target() {
    let target = IndexDriftRepairTarget::missing_entity(entity_key(Uuid::from_u128(1)), 7, 9)
        .expect("valid target");
    let actor = IndexDriftFindingLifecycleActor::new("operator", "subject").expect("valid actor");

    let error = IndexDriftRepairCommand::new(
        Uuid::from_u128(5),
        Uuid::from_u128(3),
        Uuid::from_u128(4),
        target,
        actor,
        "reason",
    )
    .expect_err("cross-tenant target must fail");
    assert_eq!(error, IndexDriftRepairValidationError::TargetTenantMismatch);
}

#[test]
fn repaired_completion_requires_after_and_owner_receipt_digests() {
    let invalid = IndexDriftRepairCompletion::new(
        "index_missing_entity_owner".to_owned(),
        IndexDriftRepairReceiptOutcome::Repaired,
        digest('a'),
        None,
        None,
    );
    assert_eq!(invalid, Err(IndexDriftRepairValidationError::InvalidDigest));

    let after = digest('b');
    let owner_receipt = digest('c');
    let valid = IndexDriftRepairCompletion::new(
        "index_missing_entity_owner".to_owned(),
        IndexDriftRepairReceiptOutcome::Repaired,
        digest('a'),
        Some(after.clone()),
        Some(owner_receipt.clone()),
    )
    .expect("complete repaired evidence");
    assert_eq!(valid.after_digest(), Some(after.as_str()));
    assert_eq!(valid.owner_receipt_digest(), Some(owner_receipt.as_str()));
}

#[test]
fn target_requires_positive_versions_and_identity() {
    let tenant_id = Uuid::from_u128(1);
    let error = IndexDriftRepairTarget::missing_entity(entity_key(tenant_id), 0, 9)
        .expect_err("zero indexed version must fail");
    assert_eq!(error, IndexDriftRepairValidationError::InvalidSourceVersion);
}
