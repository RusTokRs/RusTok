#[test]
fn payout_reservation_orchestration_preserves_owner_and_recovery_contracts() {
    let service = include_str!("../src/service.rs");
    let ports = include_str!("../src/ports.rs");
    let orchestration = include_str!("../src/operation_orchestration.rs");

    assert!(service.contains("ledger_writer: Option<Arc<dyn MarketplaceLedgerCommandPort>>"));
    assert!(service.contains("pub fn with_ledger_writer"));
    assert!(service.contains("pub(crate) async fn schedule_with_receipt"));
    assert!(ports.contains("schedule_with_operation"));

    assert!(orchestration.contains("BTreeMap"));
    assert!(orchestration.contains("entry.order_id"));
    assert!(orchestration.contains("MarketplaceSellerBalanceTransferKind::ReserveHold"));
    assert!(orchestration.contains("MarketplaceSellerBalanceTransferKind::ReserveRelease"));
    assert!(orchestration.contains("line.credit_entry.id"));
    assert!(orchestration.contains("order_by_desc(operation_transfer::Column::SequenceNo)"));
    assert!(orchestration.contains("with_idempotency_key(executing.idempotency_key.clone())"));
    assert!(orchestration.contains("payload.response = Some(response.clone())"));
    assert!(orchestration.contains("payout_schedule_outcome_is_ambiguous"));
    assert!(orchestration.contains("marketplace_payout.transfer_payload_corrupt"));
    assert!(orchestration.contains("let _ = self.ledger_writer()?"));
    assert!(orchestration.contains("LeaseOwner.eq(lease_owner)"));
    assert!(!orchestration.contains("last_error_message"));
}
