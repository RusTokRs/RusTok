use uuid::Uuid;

use rustok_modules::{
    ConflictFenceSet, TransitionApplyReceipt, TransitionCancellationReceipt,
    TransitionConfirmationReceipt, TransitionPreviewReceipt, TransitionReceiptError,
    TransitionRollbackReceipt, UpdateMode,
};

fn sample_fences(slug: &str, tenant_id: Option<Uuid>) -> ConflictFenceSet {
    ConflictFenceSet::derive_module_update_fences(slug, tenant_id, &[])
}

#[test]
fn test_preview_receipt_generation_and_digest() {
    let op_id = Uuid::new_v4();
    let fences = sample_fences("blog", None);

    let preview = TransitionPreviewReceipt::new(
        op_id,
        "blog".to_string(),
        None,
        "sha256:1111111111111111111111111111111111111111111111111111111111111111".to_string(),
        Some("sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string()),
        UpdateMode::Automatic,
        "sha256:preflight_digest_abc".to_string(),
        fences,
    );

    let digest = preview.digest();
    assert!(digest.starts_with("sha256:"));
    assert_eq!(preview.digest(), digest, "Digest must be deterministic");
}

#[test]
fn test_confirmation_receipt_binding_and_validation() {
    let op_id = Uuid::new_v4();
    let fences = sample_fences("analytics", None);

    let preview = TransitionPreviewReceipt::new(
        op_id,
        "analytics".to_string(),
        None,
        "sha256:candidate_digest".to_string(),
        None,
        UpdateMode::Maintenance,
        "sha256:preflight_digest".to_string(),
        fences,
    );

    let valid_confirmation = TransitionConfirmationReceipt::new(
        op_id,
        preview.digest(),
        UpdateMode::Maintenance,
        "operator_admin_1".to_string(),
        2,
    );

    assert!(valid_confirmation.validate_matches_preview(&preview).is_ok());

    let invalid_confirmation = TransitionConfirmationReceipt::new(
        op_id,
        "sha256:tampered_or_stale_preview_digest".to_string(),
        UpdateMode::Maintenance,
        "operator_admin_1".to_string(),
        2,
    );

    let err = invalid_confirmation
        .validate_matches_preview(&preview)
        .unwrap_err();
    assert!(matches!(
        err,
        TransitionReceiptError::PreviewDigestMismatch { .. }
    ));
}

#[test]
fn test_apply_receipt_issuance_invariants() {
    let op_id = Uuid::new_v4();
    let fences = sample_fences("cart", None);

    // 1. Automatic mode can issue apply without confirmation
    let auto_preview = TransitionPreviewReceipt::new(
        op_id,
        "cart".to_string(),
        None,
        "sha256:cart_candidate".to_string(),
        None,
        UpdateMode::Automatic,
        "sha256:preflight_1".to_string(),
        fences.clone(),
    );

    let apply_auto = TransitionApplyReceipt::issue(&auto_preview, None, 1, 1).expect("apply auto");
    assert_eq!(apply_auto.operation_id, op_id);
    assert_eq!(apply_auto.serving_generation, 1);
    assert!(apply_auto.confirmation_digest.is_none());

    // 2. Maintenance mode requires confirmation
    let maint_preview = TransitionPreviewReceipt::new(
        op_id,
        "cart".to_string(),
        None,
        "sha256:cart_candidate".to_string(),
        None,
        UpdateMode::Maintenance,
        "sha256:preflight_2".to_string(),
        fences,
    );

    let missing_conf_err = TransitionApplyReceipt::issue(&maint_preview, None, 2, 1).unwrap_err();
    assert_eq!(
        missing_conf_err,
        TransitionReceiptError::ConfirmationRequired
    );

    // Confirmed maintenance mode succeeds
    let conf = TransitionConfirmationReceipt::new(
        op_id,
        maint_preview.digest(),
        UpdateMode::Maintenance,
        "operator_root".to_string(),
        1,
    );

    let apply_maint =
        TransitionApplyReceipt::issue(&maint_preview, Some(&conf), 2, 1).expect("apply maint");
    assert_eq!(apply_maint.confirmation_digest, Some(conf.digest()));

    // 3. Candidate digest cannot be empty
    let empty_preview = TransitionPreviewReceipt::new(
        op_id,
        "cart".to_string(),
        None,
        "".to_string(),
        None,
        UpdateMode::Automatic,
        "sha256:preflight_3".to_string(),
        sample_fences("cart", None),
    );
    assert_eq!(
        TransitionApplyReceipt::issue(&empty_preview, None, 1, 1).unwrap_err(),
        TransitionReceiptError::CandidateDigestRequired
    );
}

#[test]
fn test_cancellation_receipt_guards() {
    let op_id = Uuid::new_v4();

    // Cancellation before point of no return succeeds
    let cancel = TransitionCancellationReceipt::cancel(
        op_id,
        "inventory".to_string(),
        None,
        "Operator aborted during prestaging".to_string(),
        "operator_user_1".to_string(),
        "prestaging".to_string(),
        false,
    )
    .expect("cancel");

    assert_eq!(cancel.operation_id, op_id);
    assert_eq!(cancel.phase_at_cancellation, "prestaging");
    assert!(cancel.digest().starts_with("sha256:"));

    // Cancellation after point of no return fails
    let err = TransitionCancellationReceipt::cancel(
        op_id,
        "inventory".to_string(),
        None,
        "Too late".to_string(),
        "operator_user_1".to_string(),
        "activating".to_string(),
        true,
    )
    .unwrap_err();

    assert!(matches!(
        err,
        TransitionReceiptError::PastPointOfNoReturn(..)
    ));
}

#[test]
fn test_rollback_receipt_predecessor_and_reversibility_invariants() {
    let op_id = Uuid::new_v4();
    let predecessor = "sha256:predecessor_stable_hash".to_string();

    // 1. Reversible rollback succeeds
    let rollback = TransitionRollbackReceipt::issue(
        op_id,
        "pricing".to_string(),
        None,
        predecessor.clone(),
        true,
        "Candidate readiness probe failed".to_string(),
        1,
    )
    .expect("rollback");

    assert_eq!(rollback.operation_id, op_id);
    assert_eq!(rollback.predecessor_digest, predecessor);
    assert!(rollback.is_reversible);
    assert_eq!(rollback.recovery_attempt, 1);
    assert!(rollback.digest().starts_with("sha256:"));

    // 2. Irreversible rollback fails closed
    let err_irreversible = TransitionRollbackReceipt::issue(
        op_id,
        "pricing".to_string(),
        None,
        predecessor,
        false,
        "Candidate failed after irreversible column drop".to_string(),
        1,
    )
    .unwrap_err();

    assert!(matches!(
        err_irreversible,
        TransitionReceiptError::RollbackProhibited(..)
    ));

    // 3. Empty predecessor digest fails
    let err_empty = TransitionRollbackReceipt::issue(
        op_id,
        "pricing".to_string(),
        None,
        "".to_string(),
        true,
        "reason".to_string(),
        1,
    )
    .unwrap_err();

    assert_eq!(
        err_empty,
        TransitionReceiptError::InvalidPredecessorDigest
    );
}
