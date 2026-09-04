use rustok_modules::data_backfill::{
    BackfillError, BackfillPageResult, BackfillStatus, DataBackfillCoordinator,
    InMemoryBackfillCheckpointStore,
};
use uuid::Uuid;

#[tokio::test]
async fn test_multi_page_backfill_convergence() {
    let store = InMemoryBackfillCheckpointStore::default();
    let coordinator = DataBackfillCoordinator::new(store);
    let backfill_id = Uuid::new_v4();
    let operation_id = Uuid::new_v4();
    let data_owner_id = Uuid::new_v4();

    let initial = coordinator
        .start_or_resume(backfill_id, operation_id, "sample_module", data_owner_id)
        .await
        .expect("start");
    assert_eq!(initial.status, BackfillStatus::InProgress);
    assert_eq!(initial.items_processed, 0);

    // Page 1: 50 items
    let page1 = coordinator
        .process_page(backfill_id, |cursor| async move {
            assert!(cursor.is_none());
            Ok(BackfillPageResult {
                processed_count: 50,
                next_cursor: Some("cursor_50".to_string()),
                has_more: true,
            })
        })
        .await
        .expect("page 1");
    assert_eq!(page1.items_processed, 50);
    assert_eq!(page1.cursor.as_deref(), Some("cursor_50"));
    assert_eq!(page1.status, BackfillStatus::InProgress);

    // Page 2: 50 items, terminal
    let page2 = coordinator
        .process_page(backfill_id, |cursor| async move {
            assert_eq!(cursor.as_deref(), Some("cursor_50"));
            Ok(BackfillPageResult {
                processed_count: 50,
                next_cursor: Some("cursor_100".to_string()),
                has_more: false,
            })
        })
        .await
        .expect("page 2");
    assert_eq!(page2.items_processed, 100);
    assert_eq!(page2.status, BackfillStatus::Converged);

    // Subsequent call on converged backfill is idempotent
    let idempotent = coordinator
        .process_page(backfill_id, |_| async {
            panic!("should not be called on converged backfill");
        })
        .await
        .expect("idempotent");
    assert_eq!(idempotent.status, BackfillStatus::Converged);
    assert_eq!(idempotent.items_processed, 100);
}

#[tokio::test]
async fn test_resumption_from_last_checkpoint_after_crash() {
    let store = InMemoryBackfillCheckpointStore::default();
    let coordinator = DataBackfillCoordinator::new(store);
    let backfill_id = Uuid::new_v4();
    let operation_id = Uuid::new_v4();
    let data_owner_id = Uuid::new_v4();

    coordinator
        .start_or_resume(backfill_id, operation_id, "sample_module", data_owner_id)
        .await
        .expect("start");

    // Commit page 1
    coordinator
        .process_page(backfill_id, |_| async {
            Ok(BackfillPageResult {
                processed_count: 25,
                next_cursor: Some("key_25".to_string()),
                has_more: true,
            })
        })
        .await
        .expect("page 1");

    // Simulate crash and restart by calling start_or_resume again
    let resumed = coordinator
        .start_or_resume(backfill_id, operation_id, "sample_module", data_owner_id)
        .await
        .expect("resume");
    assert_eq!(resumed.items_processed, 25);
    assert_eq!(resumed.cursor.as_deref(), Some("key_25"));
    assert_eq!(resumed.status, BackfillStatus::InProgress);

    // Continue processing from resumed cursor
    let page2 = coordinator
        .process_page(backfill_id, |cursor| async move {
            assert_eq!(cursor.as_deref(), Some("key_25"));
            Ok(BackfillPageResult {
                processed_count: 25,
                next_cursor: Some("key_50".to_string()),
                has_more: false,
            })
        })
        .await
        .expect("page 2");
    assert_eq!(page2.items_processed, 50);
    assert_eq!(page2.status, BackfillStatus::Converged);
}

#[tokio::test]
async fn test_uncertain_outcome_reconciliation() {
    let store = InMemoryBackfillCheckpointStore::default();
    let coordinator = DataBackfillCoordinator::new(store);
    let backfill_id = Uuid::new_v4();
    let operation_id = Uuid::new_v4();
    let data_owner_id = Uuid::new_v4();

    coordinator
        .start_or_resume(backfill_id, operation_id, "sample_module", data_owner_id)
        .await
        .expect("start");

    // Enter uncertain outcome due to ambiguous commit timeout
    let uncertain = coordinator
        .record_uncertain_outcome(backfill_id)
        .await
        .expect("record uncertain");
    assert_eq!(uncertain.status, BackfillStatus::UncertainOutcomeReconciling);

    // Reconcile and resume cleanly
    let reconciled = coordinator
        .start_or_resume(backfill_id, operation_id, "sample_module", data_owner_id)
        .await
        .expect("reconcile");
    assert_eq!(reconciled.status, BackfillStatus::InProgress);

    // Successfully complete remaining work
    let finished = coordinator
        .process_page(backfill_id, |_| async {
            Ok(BackfillPageResult {
                processed_count: 10,
                next_cursor: Some("final".to_string()),
                has_more: false,
            })
        })
        .await
        .expect("finish");
    assert_eq!(finished.status, BackfillStatus::Converged);
    assert_eq!(finished.items_processed, 10);
}

#[tokio::test]
async fn test_corrupted_checkpoint_fails_closed() {
    use rustok_modules::data_backfill::BackfillCheckpointStore;

    let store = InMemoryBackfillCheckpointStore::default();
    let backfill_id = Uuid::new_v4();
    let operation_id = Uuid::new_v4();
    let data_owner_id = Uuid::new_v4();

    let mut corrupted = rustok_modules::data_backfill::BackfillCheckpoint::new(
        backfill_id,
        operation_id,
        "sample_module",
        data_owner_id,
    );
    corrupted.checkpoint_digest = "sha256:corrupted_invalid_digest".to_string();
    store.save_checkpoint(&corrupted).await.expect("save");

    let coordinator = DataBackfillCoordinator::new(store);
    let err = coordinator
        .start_or_resume(backfill_id, operation_id, "sample_module", data_owner_id)
        .await
        .expect_err("tampered checkpoint must fail closed");

    assert!(matches!(err, BackfillError::DigestMismatch { .. }));
}
