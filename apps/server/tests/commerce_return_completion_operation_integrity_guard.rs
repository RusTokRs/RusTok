#[test]
fn return_completion_operation_persistence_invariants_are_registered() {
    let journal = include_str!(
        "../../../crates/modules/rustok-commerce/src/services/return_completion_operation.rs"
    );
    let migration = include_str!(
        "../../../crates/modules/rustok-commerce/src/migrations/m20260930_000009_harden_return_completion_operation_identity.rs"
    );
    let migrations = include_str!("../../../crates/modules/rustok-commerce/src/migrations/mod.rs");

    for marker in [
        "request_hash must be a 64-character hexadecimal SHA-256 digest",
        "normalize_lease_seconds",
        "normalize_lease_owner",
    ] {
        assert!(
            journal.contains(marker),
            "return completion journal is missing normalization invariant {marker}"
        );
    }

    for marker in [
        "ck_return_completion_operations_request_hash_sha256",
        "ck_return_completion_operations_pending_stage",
        "ck_return_completion_operations_completed_stage",
        "DatabaseBackend::Postgres",
        "DatabaseBackend::MySql",
        "DatabaseBackend::Sqlite",
        "REGEXP_LIKE(NEW.request_hash, '^[0-9a-f]{64}$', 'c') = 0",
        "length(NEW.request_hash) <> 64",
        "NEW.status = 'pending'",
        "NEW.stage = 'completed'",
    ] {
        assert!(
            migration.contains(marker),
            "return completion integrity migration is missing invariant {marker}"
        );
    }

    assert!(
        migrations.contains("mod m20260930_000009_harden_return_completion_operation_identity;"),
        "return completion integrity migration must be declared"
    );
    assert!(
        migrations.contains(
            "m20260930_000009_harden_return_completion_operation_identity::Migration"
        ),
        "return completion integrity migration must be registered"
    );
    assert!(
        migrations.contains(
            ""m20260930_000009_harden_return_completion_operation_identity""
        ),
        "return completion integrity migration must be present in dependency descriptors"
    );
}
