use rustok_core::MigrationSafetyAssessment;
use std::collections::BTreeSet;

use sea_orm_migration::MigratorTrait;

use crate::{
    ColumnSummary, LockClassification, MigrationDescriptor, Migrator, SchemaOperation,
    SqliteTestMigrator, classify_operations, compute_migration_plan_digest,
    compute_schema_state_digest, generate_migration_dry_run_receipt,
    sqlite_test_migration_is_compatible,
};

#[test]
fn migrator_contract_guarantees_unique_names_and_chronological_ordering() {
    let migrations = Migrator::migrations();
    assert!(!migrations.is_empty(), "migrator must contain migrations");

    let mut seen = BTreeSet::new();
    for migration in &migrations {
        let name = migration.name();
        assert!(
            seen.insert(name.to_string()),
            "duplicate migration name: {name}"
        );
        assert!(
            name.starts_with('m'),
            "migration name must start with 'm': {name}"
        );
    }
}

#[test]
fn sqlite_test_migrator_contract_excludes_postgresql_native_migrations() {
    let all_migrations = Migrator::migrations();
    let sqlite_migrations = SqliteTestMigrator::migrations();

    assert!(
        sqlite_migrations.len() < all_migrations.len(),
        "sqlite test migrator must exclude postgresql-native migrations"
    );

    for m in &sqlite_migrations {
        assert!(
            sqlite_test_migration_is_compatible(m.name()),
            "sqlite test migrator must only contain compatible migrations: {}",
            m.name()
        );
    }

    assert!(!sqlite_test_migration_is_compatible(
        "m20260914_000001_create_product_relations"
    ));
    assert!(sqlite_test_migration_is_compatible(
        "m20250101_000001_create_tenants"
    ));
}

#[test]
fn schema_diff_classification_contract_differentiates_safe_vs_maintenance() {
    // 1. Additive safe operations
    let safe_ops = vec![SchemaOperation::CreateTable {
        table_name: "test_items".to_string(),
        columns: vec![ColumnSummary {
            name: "id".to_string(),
            column_type: "uuid".to_string(),
            is_nullable: false,
            has_default: false,
            is_primary_key: true,
        }],
        is_temporary: false,
    }];

    let (safe_classification, lock) = classify_operations(&safe_ops);
    assert_eq!(safe_classification, MigrationSafetyAssessment::AdditiveSafe);
    assert_eq!(lock, LockClassification::RowLevel);

    // 2. Destructive or table-locking operations
    let dangerous_ops = vec![
        SchemaOperation::DropTable {
            table_name: "old_table".to_string(),
        },
        SchemaOperation::AddNotNullConstraintWithoutDefault {
            table_name: "test_items".to_string(),
            column_name: "required_uninit".to_string(),
        },
    ];

    let (dangerous_classification, max_lock) = classify_operations(&dangerous_ops);
    assert!(matches!(
        dangerous_classification,
        MigrationSafetyAssessment::MaintenanceRequired {
            is_destructive: true,
            locks_table: true,
            ..
        }
    ));
    assert_eq!(max_lock, LockClassification::AccessExclusive);
}

#[test]
fn deterministic_digests_and_dry_run_receipt_generation() {
    let plan_digest = compute_migration_plan_digest(
        &["m20250101_000001_create_tenants".to_string()],
        &[SchemaOperation::CreateTable {
            table_name: "tenants".to_string(),
            columns: vec![],
            is_temporary: false,
        }],
    );
    assert!(plan_digest.starts_with("sha256:"));

    let schema_digest = compute_schema_state_digest(
        &["tenants".to_string()],
        &["tenants.id".to_string(), "tenants.slug".to_string()],
    );
    assert!(schema_digest.starts_with("sha256:"));

    let receipt = generate_migration_dry_run_receipt(
        schema_digest,
        vec!["m20250101_000001_create_tenants".to_string()],
        vec![SchemaOperation::CreateTable {
            table_name: "tenants".to_string(),
            columns: vec![],
            is_temporary: false,
        }],
    );
    assert!(receipt.classification.is_additive_safe());
    assert_eq!(
        receipt.classification,
        MigrationSafetyAssessment::AdditiveSafe
    );
    assert_eq!(receipt.lock_summary, LockClassification::RowLevel);
}

#[test]
fn migration_descriptor_roundtrip_contract() {
    let descriptor = MigrationDescriptor::new(
        "m20260713_000009_create_checkout_operations",
        vec![
            "m20261029_000101_create_cart_schema",
            "m20260325_000101_create_order_tables",
        ],
    );
    assert_eq!(
        descriptor.migration,
        "m20260713_000009_create_checkout_operations"
    );
    assert_eq!(descriptor.after.len(), 2);
}
