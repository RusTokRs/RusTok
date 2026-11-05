use rustok_core::MigrationSafetyAssessment;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Individual schema operation performed during a migration phase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "details")]
pub enum SchemaOperation {
    CreateTable {
        table_name: String,
        columns: Vec<ColumnSummary>,
        is_temporary: bool,
    },
    AddColumn {
        table_name: String,
        column_name: String,
        column_type: String,
        is_nullable: bool,
        has_default: bool,
    },
    AddIndex {
        table_name: String,
        index_name: String,
        columns: Vec<String>,
        is_unique: bool,
        is_concurrent: bool,
    },
    AddForeignKey {
        table_name: String,
        foreign_table: String,
        column_name: String,
    },
    DropTable {
        table_name: String,
    },
    DropColumn {
        table_name: String,
        column_name: String,
    },
    AlterColumnType {
        table_name: String,
        column_name: String,
        old_type: String,
        new_type: String,
    },
    AddNotNullConstraintWithoutDefault {
        table_name: String,
        column_name: String,
    },
    RenameColumn {
        table_name: String,
        old_name: String,
        new_name: String,
    },
    RenameTable {
        old_name: String,
        new_name: String,
    },
    RawSql {
        statement_summary: String,
        is_destructive: bool,
        locks_table: bool,
    },
}

/// Summary of a column definition in a newly created table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnSummary {
    pub name: String,
    pub column_type: String,
    pub is_nullable: bool,
    pub has_default: bool,
    pub is_primary_key: bool,
}

/// Coarse lock summary inferred from supplied operations. `None` means no
/// known lock fact; an unknown assessment is never proof of lock-free execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub enum LockClassification {
    None,
    RowLevel,
    ShareUpdateExclusive,
    TableExclusive,
    AccessExclusive,
}

/// Diagnostic receipt of supplied operations, not observed schema evidence or
/// authorization to execute a production migration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationDryRunReceipt {
    pub source_schema_digest: String,
    pub target_schema_digest: String,
    pub migration_plan_digest: String,
    pub target_migrations: Vec<String>,
    pub operations: Vec<SchemaOperation>,
    pub classification: MigrationSafetyAssessment,
    pub lock_summary: LockClassification,
}

#[derive(Debug, Error)]
pub enum MigrationDiffError {
    #[error("Database error during schema introspection: {0}")]
    Introspection(String),
    #[error("Invalid migration plan: {0}")]
    InvalidPlan(String),
    #[error(
        "Schema drift detected: current schema digest {actual} does not match expected {expected}"
    )]
    SchemaDrift { expected: String, actual: String },
}

/// Classifies supplied operation facts, without observing a database or
/// authorizing execution. Unknown coverage remains explicitly unknown.
pub fn classify_operation(op: &SchemaOperation) -> (MigrationSafetyAssessment, LockClassification) {
    use LockClassification::{
        AccessExclusive, None as NoLock, RowLevel, ShareUpdateExclusive, TableExclusive,
    };
    let maintenance = |reason: String, lock: LockClassification, is_destructive| {
        (
            MigrationSafetyAssessment::MaintenanceRequired {
                reasons: vec![reason],
                locks_table: lock >= TableExclusive,
                is_destructive,
            },
            lock,
        )
    };
    match op {
        SchemaOperation::CreateTable { .. } => (MigrationSafetyAssessment::AdditiveSafe, RowLevel),
        SchemaOperation::AddColumn {
            table_name,
            column_name,
            ..
        } => maintenance(
            format!(
                "Adding column '{table_name}.{column_name}' requires ACCESS EXCLUSIVE; lock-duration and default/rewrite safety are unproven"
            ),
            AccessExclusive,
            false,
        ),
        SchemaOperation::AddIndex {
            is_concurrent: true,
            ..
        } => maintenance(
            "Concurrent index creation is nontransactional and requires owner recovery evidence"
                .to_string(),
            ShareUpdateExclusive,
            false,
        ),
        SchemaOperation::AddIndex { .. } => maintenance(
            "Index creation is non-concurrent (locks table for writes)".to_string(),
            TableExclusive,
            false,
        ),
        SchemaOperation::AddForeignKey {
            table_name,
            foreign_table,
            column_name,
        } => maintenance(
            format!(
                "Foreign key on '{table_name}.{column_name}' -> '{foreign_table}' acquires table lock"
            ),
            TableExclusive,
            false,
        ),
        SchemaOperation::DropTable { table_name } => maintenance(
            format!("Destructive operation: Drop table '{table_name}'"),
            AccessExclusive,
            true,
        ),
        SchemaOperation::DropColumn {
            table_name,
            column_name,
        } => maintenance(
            format!("Destructive operation: Drop column '{column_name}' from '{table_name}'"),
            AccessExclusive,
            true,
        ),
        SchemaOperation::AlterColumnType {
            table_name,
            column_name,
            old_type,
            new_type,
        } => maintenance(
            format!(
                "Destructive/locking type alteration on '{table_name}.{column_name}': '{old_type}' -> '{new_type}'"
            ),
            AccessExclusive,
            true,
        ),
        SchemaOperation::AddNotNullConstraintWithoutDefault {
            table_name,
            column_name,
        } => maintenance(
            format!(
                "Added NOT NULL constraint on existing column '{table_name}.{column_name}' without default"
            ),
            AccessExclusive,
            false,
        ),
        SchemaOperation::RenameColumn {
            table_name,
            old_name,
            new_name,
        } => maintenance(
            format!(
                "Renaming column '{old_name}' to '{new_name}' in table '{table_name}' breaks version N"
            ),
            AccessExclusive,
            false,
        ),
        SchemaOperation::RenameTable { old_name, new_name } => maintenance(
            format!("Renaming table '{old_name}' to '{new_name}' breaks version N"),
            AccessExclusive,
            false,
        ),
        SchemaOperation::RawSql {
            statement_summary,
            is_destructive,
            locks_table,
        } => {
            let reason = format!("Unverified raw SQL statement: {statement_summary}");
            if *is_destructive || *locks_table {
                maintenance(
                    reason,
                    if *is_destructive {
                        AccessExclusive
                    } else {
                        TableExclusive
                    },
                    *is_destructive,
                )
            } else {
                (
                    MigrationSafetyAssessment::Unknown {
                        reasons: vec![reason],
                    },
                    NoLock,
                )
            }
        }
    }
}

/// Combines the exact supplied operation inventory. An empty inventory is not
/// evidence of an owner-proven no-op.
pub fn classify_operations(
    ops: &[SchemaOperation],
) -> (MigrationSafetyAssessment, LockClassification) {
    let mut reasons = Vec::new();
    let mut highest_lock = LockClassification::None;
    let mut is_destructive = false;
    let mut locks_table = false;
    let mut requires_maintenance = false;
    let mut unknown = ops.is_empty();
    for op in ops {
        let (assessment, lock) = classify_operation(op);
        highest_lock = highest_lock.max(lock);
        reasons.extend(assessment.denial_reasons());
        match assessment {
            MigrationSafetyAssessment::AdditiveSafe => {}
            MigrationSafetyAssessment::MaintenanceRequired {
                locks_table: locking,
                is_destructive: destructive,
                ..
            } => {
                requires_maintenance = true;
                locks_table |= locking;
                is_destructive |= destructive;
            }
            MigrationSafetyAssessment::Unknown { .. } => unknown = true,
        }
    }
    let assessment = if requires_maintenance {
        MigrationSafetyAssessment::MaintenanceRequired {
            reasons,
            locks_table,
            is_destructive,
        }
    } else if unknown {
        MigrationSafetyAssessment::Unknown { reasons }
    } else {
        MigrationSafetyAssessment::AdditiveSafe
    };
    (assessment, highest_lock)
}

/// Computes the deterministic SHA-256 digest of a migration plan.
pub fn compute_migration_plan_digest(
    target_migrations: &[String],
    operations: &[SchemaOperation],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"rustok:migration_plan:v1:");
    // These closed, derived types contain only strings, booleans and arrays;
    // serde_json cannot fail without an IO writer or fallible custom serializer.
    let bytes = serde_json::to_vec(&(target_migrations, operations))
        .expect("closed migration plan values always serialize to JSON");
    hasher.update(bytes);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

/// Computes the deterministic SHA-256 digest of a schema state representation.
pub fn compute_schema_state_digest(tables: &[String], columns: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"rustok:schema_state:v1:");
    // Arrays of UTF-8 strings have no fallible JSON serialization path.
    let bytes = serde_json::to_vec(&(tables, columns))
        .expect("schema name arrays always serialize to JSON");
    hasher.update(bytes);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

/// Generates a dry-run receipt from source schema digest, migrations, and operations.
pub fn generate_migration_dry_run_receipt(
    source_schema_digest: String,
    target_migrations: Vec<String>,
    operations: Vec<SchemaOperation>,
) -> MigrationDryRunReceipt {
    let (mut classification, lock_summary) = classify_operations(&operations);
    let identities: std::collections::HashSet<_> = target_migrations.iter().collect();
    if target_migrations.is_empty()
        || identities.len() != target_migrations.len()
        || target_migrations.iter().any(|name| name.trim().is_empty())
    {
        let reason = "Migration identity inventory is missing or invalid".to_owned();
        match &mut classification {
            MigrationSafetyAssessment::MaintenanceRequired { reasons, .. }
            | MigrationSafetyAssessment::Unknown { reasons } => reasons.push(reason),
            MigrationSafetyAssessment::AdditiveSafe => {
                classification = MigrationSafetyAssessment::Unknown {
                    reasons: vec![reason],
                };
            }
        }
    }
    let migration_plan_digest = compute_migration_plan_digest(&target_migrations, &operations);

    // Project target schema digest from source + operations
    let mut target_hasher = Sha256::new();
    target_hasher.update(source_schema_digest.as_bytes());
    target_hasher.update(b"->");
    target_hasher.update(migration_plan_digest.as_bytes());
    let target_schema_digest = format!("sha256:{}", hex::encode(target_hasher.finalize()));

    MigrationDryRunReceipt {
        source_schema_digest,
        target_schema_digest,
        migration_plan_digest,
        target_migrations,
        operations,
        classification,
        lock_summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nullable_and_default_flags_cannot_hide_add_column_lock_and_rewrite_risks() {
        for is_nullable in [false, true] {
            for has_default in [false, true] {
                let (assessment, lock) = classify_operations(&[SchemaOperation::AddColumn {
                    table_name: "posts".to_owned(),
                    column_name: "title".to_owned(),
                    column_type: "text".to_owned(),
                    is_nullable,
                    has_default,
                }]);
                assert_eq!(lock, LockClassification::AccessExclusive);
                assert!(matches!(
                    assessment,
                    MigrationSafetyAssessment::MaintenanceRequired {
                        locks_table: true,
                        ..
                    }
                ));
            }
        }
    }

    #[test]
    fn locking_foreign_key_keeps_a_typed_maintenance_reason() {
        let operation = SchemaOperation::AddForeignKey {
            table_name: "posts".to_owned(),
            foreign_table: "tenants".to_owned(),
            column_name: "tenant_id".to_owned(),
        };
        let (assessment, lock) = classify_operations(&[operation]);
        assert_eq!(lock, LockClassification::TableExclusive);
        assert!(matches!(
            assessment,
            MigrationSafetyAssessment::MaintenanceRequired {
                locks_table: true,
                is_destructive: false,
                ..
            }
        ));
        assert!(
            assessment
                .denial_reasons()
                .iter()
                .any(|reason| reason.contains("Foreign key"))
        );
    }

    #[test]
    fn raw_sql_never_becomes_safe_from_caller_supplied_negative_flags() {
        let (assessment, _) = classify_operations(&[SchemaOperation::RawSql {
            statement_summary: "opaque helper".to_owned(),
            is_destructive: false,
            locks_table: false,
        }]);
        assert!(matches!(
            assessment,
            MigrationSafetyAssessment::Unknown { .. }
        ));
        let (assessment, _) = classify_operations(&[SchemaOperation::RawSql {
            statement_summary: "data removal".to_owned(),
            is_destructive: true,
            locks_table: false,
        }]);
        assert!(matches!(
            assessment,
            MigrationSafetyAssessment::MaintenanceRequired {
                is_destructive: true,
                ..
            }
        ));
    }

    #[test]
    fn incomplete_inventory_does_not_erase_known_destructive_facts() {
        let receipt = generate_migration_dry_run_receipt(
            "source".to_owned(),
            vec![],
            vec![
                SchemaOperation::DropTable {
                    table_name: "old_data".to_owned(),
                },
                SchemaOperation::RawSql {
                    statement_summary: "unclassified helper".to_owned(),
                    is_destructive: false,
                    locks_table: false,
                },
            ],
        );
        assert_eq!(receipt.lock_summary, LockClassification::AccessExclusive);
        assert!(matches!(
            receipt.classification,
            MigrationSafetyAssessment::MaintenanceRequired {
                locks_table: true,
                is_destructive: true,
                ..
            }
        ));
        let reasons = receipt.classification.denial_reasons();
        assert!(
            reasons
                .iter()
                .any(|reason| reason.contains("identity inventory"))
        );
        assert!(
            reasons
                .iter()
                .any(|reason| reason.contains("unclassified helper"))
        );
    }

    #[test]
    fn incomplete_inventory_and_nontransactional_index_cannot_assert_additive_safety() {
        assert!(matches!(
            classify_operations(&[]).0,
            MigrationSafetyAssessment::Unknown { .. }
        ));
        let (assessment, lock) = classify_operations(&[SchemaOperation::AddIndex {
            table_name: "posts".to_owned(),
            index_name: "posts_title".to_owned(),
            columns: vec!["title".to_owned()],
            is_unique: false,
            is_concurrent: true,
        }]);
        assert_eq!(lock, LockClassification::ShareUpdateExclusive);
        assert!(matches!(
            assessment,
            MigrationSafetyAssessment::MaintenanceRequired {
                locks_table: false,
                is_destructive: false,
                ..
            }
        ));
        assert!(
            assessment
                .denial_reasons()
                .iter()
                .any(|reason| reason.contains("nontransactional"))
        );
        for identities in [
            vec![],
            vec![" ".to_owned()],
            vec!["m1".to_owned(), "m1".to_owned()],
        ] {
            let receipt = generate_migration_dry_run_receipt(
                "source".to_owned(),
                identities,
                vec![SchemaOperation::CreateTable {
                    table_name: "posts".to_owned(),
                    columns: vec![],
                    is_temporary: false,
                }],
            );
            assert!(matches!(
                receipt.classification,
                MigrationSafetyAssessment::Unknown { .. }
            ));
        }
    }

    #[test]
    fn digest_inputs_preserve_array_boundaries() {
        assert_ne!(
            compute_migration_plan_digest(&["a,b".to_owned()], &[]),
            compute_migration_plan_digest(&["a".to_owned(), "b".to_owned()], &[])
        );
        assert_ne!(
            compute_schema_state_digest(&["a;b".to_owned()], &[]),
            compute_schema_state_digest(&["a".to_owned(), "b".to_owned()], &[])
        );
    }

    #[test]
    fn test_additive_safe_plan_classification() {
        let ops = vec![SchemaOperation::CreateTable {
            table_name: "test_table".to_string(),
            columns: vec![ColumnSummary {
                name: "id".to_string(),
                column_type: "uuid".to_string(),
                is_nullable: false,
                has_default: false,
                is_primary_key: true,
            }],
            is_temporary: false,
        }];

        let receipt = generate_migration_dry_run_receipt(
            "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            vec!["m20260901_000001_test_migration".to_string()],
            ops,
        );

        assert!(receipt.classification.is_additive_safe());
        assert_eq!(
            receipt.classification,
            MigrationSafetyAssessment::AdditiveSafe
        );
        assert_eq!(receipt.lock_summary, LockClassification::RowLevel);
        assert!(receipt.migration_plan_digest.starts_with("sha256:"));
    }

    #[test]
    fn test_destructive_plan_classification() {
        let ops = vec![SchemaOperation::DropColumn {
            table_name: "users".to_string(),
            column_name: "legacy_field".to_string(),
        }];

        let receipt = generate_migration_dry_run_receipt(
            "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            vec!["m20260901_000002_drop_column".to_string()],
            ops,
        );

        assert!(!receipt.classification.is_additive_safe());
        match receipt.classification {
            MigrationSafetyAssessment::MaintenanceRequired {
                reasons,
                locks_table,
                is_destructive,
            } => {
                assert!(is_destructive);
                assert!(locks_table);
                assert_eq!(reasons.len(), 1);
                assert!(reasons[0].contains("Drop column"));
            }
            _ => panic!("expected MaintenanceRequired"),
        }
    }

    #[test]
    fn test_not_null_without_default_requires_maintenance() {
        let ops = vec![SchemaOperation::AddColumn {
            table_name: "users".to_string(),
            column_name: "mandatory_field".to_string(),
            column_type: "text".to_string(),
            is_nullable: false,
            has_default: false,
        }];

        let receipt = generate_migration_dry_run_receipt(
            "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            vec!["m20260901_000003_not_null_col".to_string()],
            ops,
        );

        assert!(!receipt.classification.is_additive_safe());
        assert!(matches!(
            receipt.classification,
            MigrationSafetyAssessment::MaintenanceRequired { .. }
        ));
    }

    #[test]
    fn test_deterministic_plan_digest() {
        let migrations = vec![
            "m20260101_000001".to_string(),
            "m20260101_000002".to_string(),
        ];
        let ops = vec![SchemaOperation::CreateTable {
            table_name: "sample".to_string(),
            columns: vec![],
            is_temporary: false,
        }];

        let digest1 = compute_migration_plan_digest(&migrations, &ops);
        let digest2 = compute_migration_plan_digest(&migrations, &ops);
        assert_eq!(digest1, digest2);
        assert!(digest1.starts_with("sha256:"));
    }
}
