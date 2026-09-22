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

/// Classification of lock level acquired by DDL operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub enum LockClassification {
    None,
    RowLevel,
    TableExclusive,
    AccessExclusive,
}

/// Safety classification of a migration plan determining whether automatic
/// code update without maintenance window is allowed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", content = "details")]
pub enum MigrationSafetyClassification {
    /// Safe for automatic update and rollback without DB rollback.
    /// All operations are additive and backward-compatible with version N.
    AdditiveSafe,
    /// Requires maintenance mode. Automatic rollback is prohibited because
    /// operations are destructive, locking, or incompatible with version N.
    MaintenanceRequired {
        reasons: Vec<String>,
        locks_table: bool,
        is_destructive: bool,
    },
}

/// Immutable receipt of a migration plan dry-run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationDryRunReceipt {
    pub source_schema_digest: String,
    pub target_schema_digest: String,
    pub migration_plan_digest: String,
    pub target_migrations: Vec<String>,
    pub operations: Vec<SchemaOperation>,
    pub classification: MigrationSafetyClassification,
    pub lock_summary: LockClassification,
    pub is_automatic_eligible: bool,
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

/// Classifies a single schema operation.
pub fn classify_operation(op: &SchemaOperation) -> (bool, LockClassification, Option<String>) {
    match op {
        SchemaOperation::CreateTable { .. } => (true, LockClassification::RowLevel, None),
        SchemaOperation::AddColumn {
            is_nullable,
            has_default,
            table_name,
            column_name,
            ..
        } => {
            if *is_nullable || *has_default {
                (true, LockClassification::RowLevel, None)
            } else {
                (
                    false,
                    LockClassification::TableExclusive,
                    Some(format!(
                        "Column '{column_name}' in table '{table_name}' added as NOT NULL without default value"
                    )),
                )
            }
        }
        SchemaOperation::AddIndex { is_concurrent, .. } => {
            if *is_concurrent {
                (true, LockClassification::RowLevel, None)
            } else {
                // Non-concurrent index locks table for writes in Postgres
                (
                    false,
                    LockClassification::TableExclusive,
                    Some("Index creation is non-concurrent (locks table for writes)".to_string()),
                )
            }
        }
        SchemaOperation::AddForeignKey {
            table_name,
            foreign_table,
            column_name,
        } => (
            true,
            LockClassification::TableExclusive,
            Some(format!(
                "Foreign key on '{table_name}.{column_name}' -> '{foreign_table}' acquires table lock"
            )),
        ),
        SchemaOperation::DropTable { table_name } => (
            false,
            LockClassification::AccessExclusive,
            Some(format!("Destructive operation: Drop table '{table_name}'")),
        ),
        SchemaOperation::DropColumn {
            table_name,
            column_name,
        } => (
            false,
            LockClassification::AccessExclusive,
            Some(format!(
                "Destructive operation: Drop column '{column_name}' from '{table_name}'"
            )),
        ),
        SchemaOperation::AlterColumnType {
            table_name,
            column_name,
            old_type,
            new_type,
        } => (
            false,
            LockClassification::AccessExclusive,
            Some(format!(
                "Destructive/locking type alteration on '{table_name}.{column_name}': '{old_type}' -> '{new_type}'"
            )),
        ),
        SchemaOperation::AddNotNullConstraintWithoutDefault {
            table_name,
            column_name,
        } => (
            false,
            LockClassification::TableExclusive,
            Some(format!(
                "Added NOT NULL constraint on existing column '{table_name}.{column_name}' without default"
            )),
        ),
        SchemaOperation::RenameColumn {
            table_name,
            old_name,
            new_name,
        } => (
            false,
            LockClassification::AccessExclusive,
            Some(format!(
                "Renaming column '{old_name}' to '{new_name}' in table '{table_name}' breaks version N"
            )),
        ),
        SchemaOperation::RenameTable { old_name, new_name } => (
            false,
            LockClassification::AccessExclusive,
            Some(format!(
                "Renaming table '{old_name}' to '{new_name}' breaks version N"
            )),
        ),
        SchemaOperation::RawSql {
            statement_summary,
            is_destructive,
            locks_table,
        } => {
            if *is_destructive || *locks_table {
                (
                    false,
                    if *is_destructive {
                        LockClassification::AccessExclusive
                    } else {
                        LockClassification::TableExclusive
                    },
                    Some(format!("Unverified raw SQL statement: {statement_summary}")),
                )
            } else {
                (true, LockClassification::RowLevel, None)
            }
        }
    }
}

/// Classifies a collection of operations.
pub fn classify_operations(
    ops: &[SchemaOperation],
) -> (MigrationSafetyClassification, LockClassification) {
    let mut reasons = Vec::new();
    let mut highest_lock = LockClassification::None;
    let mut is_destructive = false;
    let mut locks_table = false;

    for op in ops {
        let (is_safe, lock, reason) = classify_operation(op);
        if lock > highest_lock {
            highest_lock = lock;
        }
        if lock >= LockClassification::TableExclusive {
            locks_table = true;
        }
        if matches!(
            op,
            SchemaOperation::DropTable { .. }
                | SchemaOperation::DropColumn { .. }
                | SchemaOperation::AlterColumnType { .. }
        ) {
            is_destructive = true;
        }

        if !is_safe {
            reasons.push(
                reason.unwrap_or_else(|| format!("Operation is not additive-safe: {:?}", op)),
            );
        }
    }

    let classification = if reasons.is_empty() && !locks_table && !is_destructive {
        MigrationSafetyClassification::AdditiveSafe
    } else {
        MigrationSafetyClassification::MaintenanceRequired {
            reasons,
            locks_table,
            is_destructive,
        }
    };

    (classification, highest_lock)
}

/// Computes the deterministic SHA-256 digest of a migration plan.
pub fn compute_migration_plan_digest(
    target_migrations: &[String],
    operations: &[SchemaOperation],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"rustok:migration_plan:v1:");
    for migration in target_migrations {
        hasher.update(migration.as_bytes());
        hasher.update(b",");
    }
    hasher.update(b"|ops:");
    if let Ok(json_bytes) = serde_json::to_vec(operations) {
        hasher.update(&json_bytes);
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

/// Computes the deterministic SHA-256 digest of a schema state representation.
pub fn compute_schema_state_digest(tables: &[String], columns: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"rustok:schema_state:v1:");
    for t in tables {
        hasher.update(t.as_bytes());
        hasher.update(b";");
    }
    hasher.update(b"|cols:");
    for c in columns {
        hasher.update(c.as_bytes());
        hasher.update(b";");
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

/// Generates a dry-run receipt from source schema digest, migrations, and operations.
pub fn generate_migration_dry_run_receipt(
    source_schema_digest: String,
    target_migrations: Vec<String>,
    operations: Vec<SchemaOperation>,
) -> MigrationDryRunReceipt {
    let (classification, lock_summary) = classify_operations(&operations);
    let migration_plan_digest = compute_migration_plan_digest(&target_migrations, &operations);

    // Project target schema digest from source + operations
    let mut target_hasher = Sha256::new();
    target_hasher.update(source_schema_digest.as_bytes());
    target_hasher.update(b"->");
    target_hasher.update(migration_plan_digest.as_bytes());
    let target_schema_digest = format!("sha256:{}", hex::encode(target_hasher.finalize()));

    let is_automatic_eligible =
        matches!(classification, MigrationSafetyClassification::AdditiveSafe);

    MigrationDryRunReceipt {
        source_schema_digest,
        target_schema_digest,
        migration_plan_digest,
        target_migrations,
        operations,
        classification,
        lock_summary,
        is_automatic_eligible,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_additive_safe_plan_classification() {
        let ops = vec![
            SchemaOperation::CreateTable {
                table_name: "test_table".to_string(),
                columns: vec![ColumnSummary {
                    name: "id".to_string(),
                    column_type: "uuid".to_string(),
                    is_nullable: false,
                    has_default: false,
                    is_primary_key: true,
                }],
                is_temporary: false,
            },
            SchemaOperation::AddColumn {
                table_name: "test_table".to_string(),
                column_name: "description".to_string(),
                column_type: "text".to_string(),
                is_nullable: true,
                has_default: false,
            },
            SchemaOperation::AddColumn {
                table_name: "test_table".to_string(),
                column_name: "status".to_string(),
                column_type: "text".to_string(),
                is_nullable: false,
                has_default: true,
            },
            SchemaOperation::AddIndex {
                table_name: "test_table".to_string(),
                index_name: "idx_test_desc".to_string(),
                columns: vec!["description".to_string()],
                is_unique: false,
                is_concurrent: true,
            },
        ];

        let receipt = generate_migration_dry_run_receipt(
            "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            vec!["m20260901_000001_test_migration".to_string()],
            ops,
        );

        assert!(receipt.is_automatic_eligible);
        assert_eq!(
            receipt.classification,
            MigrationSafetyClassification::AdditiveSafe
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

        assert!(!receipt.is_automatic_eligible);
        match receipt.classification {
            MigrationSafetyClassification::MaintenanceRequired {
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

        assert!(!receipt.is_automatic_eligible);
        assert!(matches!(
            receipt.classification,
            MigrationSafetyClassification::MaintenanceRequired { .. }
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
