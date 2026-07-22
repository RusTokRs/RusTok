#[test]
fn seller_balance_transfer_source_is_append_only_replay_safe_and_capacity_bound() {
    let dto = include_str!("../src/dto.rs");
    let service = include_str!("../src/balance_transfer.rs");
    let balance = include_str!("../src/balance.rs");
    let ports = include_str!("../src/ports.rs");
    let schema = include_str!(
        "../src/migrations/m20260721_000003_add_seller_balance_transfers.rs"
    );
    let immutability = include_str!(
        "../src/migrations/m20260721_000004_enforce_seller_balance_transfer_immutability.rs"
    );
    let migration_registry = include_str!("../src/migrations/mod.rs");
    let ledger_contract = include_str!("../contracts/marketplace-ledger-fba-registry.json");
    let transfer_contract = include_str!("../contracts/seller-balance-transfer-v1.json");
    let family_contract =
        include_str!("../../rustok-marketplace/contracts/marketplace-fba-registry.json");

    for kind in [
        "PendingRelease",
        "ReserveHold",
        "ReserveRelease",
        "PayoutSettlement",
        "PayoutReversal",
    ] {
        assert!(dto.contains(kind), "missing transfer kind {kind}");
    }
    for mapping in [
        "Self::PendingRelease",
        "Self::ReserveHold",
        "Self::ReserveRelease",
        "Self::PayoutSettlement",
        "Self::PayoutReversal",
    ] {
        assert!(dto.contains(mapping), "missing bucket mapping {mapping}");
    }

    assert!(service.contains("post_seller_balance_transfer"));
    assert!(service.contains("replay_existing_command"));
    assert!(service.contains("admit_command_receipt"));
    assert!(service.contains("complete_receipt"));
    assert!(service.contains("rollback_receipt"));
    assert!(service.contains("lock_exclusive"));
    assert!(service.contains("newly committed transfer entries participate"));
    assert!(service.contains("prior_reference_amounts"));
    assert!(service.contains("cumulative transfer amount"));
    assert!(service.contains("must belong to one order"));
    assert!(service.contains("debit_total_amount: Set(total_amount)"));
    assert!(service.contains("credit_total_amount: Set(total_amount)"));
    assert!(service.contains("MarketplaceLedgerAccountCode::SellerPayable"));
    assert!(service.contains("entry_balance_bucket::ActiveModel"));
    assert!(!service.contains("rustok_marketplace_payout::"));
    assert!(!service.contains("rustok_order::"));
    assert!(!service.contains("rustok_payment::"));

    assert!(balance.contains("entry_balance_bucket::Entity"));
    assert!(balance.contains("explicit_classifications"));
    assert!(balance.contains("reversal_classifications"));
    assert!(balance.contains("unwrap_or(MarketplaceSellerBalanceBucket::Pending)"));

    assert!(ports.contains("post_seller_balance_transfer"));
    assert!(ports.contains("balance_transfer_not_supported"));
    assert!(ports.contains("balance_transfer_already_posted"));
    assert!(ports.contains("PortCallPolicy::write()"));

    for table in [
        "marketplace_ledger_entry_balance_buckets",
        "marketplace_seller_balance_transfers",
        "marketplace_seller_balance_transfer_lines",
    ] {
        assert!(schema.contains(table), "missing transfer table {table}");
    }
    assert!(schema.contains("uq_mkt_balance_transfer_tenant_id"));
    assert!(schema.contains("uq_mkt_balance_transfer_source"));
    assert!(schema.contains("uq_mkt_balance_transfer_line_reference"));
    assert!(schema.contains("seller_balance_pending_release"));
    assert!(schema.contains("seller_balance_reserve_hold"));
    assert!(schema.contains("seller_balance_reserve_release"));
    assert!(schema.contains("seller_balance_payout_settlement"));
    assert!(schema.contains("seller_balance_payout_reversal"));

    assert!(immutability.contains("DatabaseBackend::Postgres"));
    assert!(immutability.contains("DatabaseBackend::Sqlite"));
    assert!(immutability.contains("DatabaseBackend::MySql"));
    assert!(immutability.contains("BEFORE UPDATE OR DELETE"));
    assert!(immutability.contains("append-only"));
    assert!(migration_registry.contains("m20260721_000003_add_seller_balance_transfers"));
    assert!(migration_registry.contains(
        "m20260721_000004_enforce_seller_balance_transfer_immutability"
    ));

    assert!(ledger_contract.contains("marketplace.ledger.v3"));
    assert!(ledger_contract.contains("post_seller_balance_transfer"));
    assert!(ledger_contract.contains("cumulative_transfer_must_not_exceed_reference_credit"));
    assert!(ledger_contract.contains("fresh_locking_reread_after_wait"));
    assert!(ledger_contract.contains("append_only_database_guards"));

    assert!(transfer_contract.contains("marketplace_ledger.seller_balance_transfer.v1"));
    assert!(transfer_contract.contains("pending_release"));
    assert!(transfer_contract.contains("reserve_hold"));
    assert!(transfer_contract.contains("reserve_release"));
    assert!(transfer_contract.contains("payout_settlement"));
    assert!(transfer_contract.contains("payout_reversal"));
    assert!(transfer_contract.contains("projection_used_for_admission\": false"));
    assert!(transfer_contract.contains("cumulative_reference_amount_must_not_exceed_credit"));
    assert!(transfer_contract.contains("append_only_guards"));

    assert!(family_contract.contains("marketplace.family.v3"));
    assert!(family_contract.contains("seller_balance_transfer_owner_contract"));
    assert!(family_contract.contains("root_persistence\": false"));
}
