use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rustok_cache::{CacheCompareAndSetOutcome, CacheService};
use tokio::sync::Barrier;

#[tokio::test]
async fn concurrent_local_compare_and_set_has_exactly_one_winner() {
    let service = CacheService::from_url(None);
    let backend = service.memory_backend(Duration::from_secs(60), 64);
    backend
        .set("contended".to_string(), b"old".to_vec())
        .await
        .unwrap();

    const CONTENDERS: usize = 16;
    let barrier = Arc::new(Barrier::new(CONTENDERS + 1));
    let mut tasks = Vec::with_capacity(CONTENDERS);
    for contender in 0..CONTENDERS {
        let backend = Arc::clone(&backend);
        let barrier = Arc::clone(&barrier);
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            let value = format!("winner-{contender}").into_bytes();
            let outcome = backend
                .compare_and_set("contended", b"old", value.clone(), None)
                .await
                .unwrap();
            (outcome, value)
        }));
    }

    barrier.wait().await;
    let mut applied = Vec::new();
    let mut mismatches = 0;
    for task in tasks {
        let (outcome, value) = task.await.unwrap();
        match outcome {
            CacheCompareAndSetOutcome::Applied => applied.push(value),
            CacheCompareAndSetOutcome::Mismatch => mismatches += 1,
        }
    }

    assert_eq!(applied.len(), 1);
    assert_eq!(mismatches, CONTENDERS - 1);
    assert_eq!(backend.get("contended").await.unwrap(), Some(applied.remove(0)));
}

#[tokio::test]
#[ignore = "requires a live Redis instance; set RUSTOK_CACHE_REAL_REDIS_URL"]
async fn real_redis_compare_and_set_is_binary_safe_and_conditionally_deletes() {
    let Ok(redis_url) = std::env::var("RUSTOK_CACHE_REAL_REDIS_URL") else {
        eprintln!("skipping live Redis CAS test: RUSTOK_CACHE_REAL_REDIS_URL is not set");
        return;
    };
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let service = CacheService::from_url(Some(&redis_url));
    let backend = service
        .backend(
            &format!("rustok:cache-cas:integration:{suffix}"),
            Duration::from_secs(60),
            16,
        )
        .await;

    let original = vec![0x00, 0xff, 0x80, b'R', 0x00];
    let replacement = vec![0xfe, 0x00, 0x81, b'S', 0xff];
    backend
        .set("binary".to_string(), original.clone())
        .await
        .unwrap();

    assert_eq!(
        backend
            .compare_and_set("binary", b"not-the-current-value", vec![1, 2, 3], None)
            .await
            .unwrap(),
        CacheCompareAndSetOutcome::Mismatch
    );
    assert_eq!(backend.get("binary").await.unwrap(), Some(original.clone()));

    assert_eq!(
        backend
            .compare_and_set("binary", &original, replacement.clone(), None)
            .await
            .unwrap(),
        CacheCompareAndSetOutcome::Applied
    );
    assert_eq!(
        backend.get("binary").await.unwrap(),
        Some(replacement.clone())
    );

    assert_eq!(
        backend
            .compare_and_set(
                "binary",
                &replacement,
                Vec::new(),
                Some(Duration::ZERO),
            )
            .await
            .unwrap(),
        CacheCompareAndSetOutcome::Applied
    );
    assert_eq!(backend.get("binary").await.unwrap(), None);
}
