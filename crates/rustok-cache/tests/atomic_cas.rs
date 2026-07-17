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
    assert_eq!(
        backend.get("contended").await.unwrap(),
        Some(applied.remove(0))
    );
}

#[tokio::test]
async fn expired_local_entry_cannot_be_revived_by_compare_and_set() {
    let service = CacheService::from_url(None);
    let backend = service.memory_backend(Duration::from_secs(60), 64);
    backend
        .set_with_ttl(
            "expired".to_string(),
            b"old".to_vec(),
            Duration::from_millis(5),
        )
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(25)).await;

    assert_eq!(
        backend
            .compare_and_set("expired", b"old", b"new".to_vec(), None)
            .await
            .unwrap(),
        CacheCompareAndSetOutcome::Mismatch
    );
    assert_eq!(backend.get("expired").await.unwrap(), None);
}

#[tokio::test]
async fn evicted_local_entry_cannot_be_revived_by_compare_and_set() {
    let service = CacheService::from_url(None);
    let backend = service.memory_backend(Duration::from_secs(60), 1);
    let first = ("eviction-first", b"first-old".to_vec());
    let second = ("eviction-second", b"second-old".to_vec());

    backend
        .set(first.0.to_string(), first.1.clone())
        .await
        .unwrap();
    backend
        .set(second.0.to_string(), second.1.clone())
        .await
        .unwrap();

    let (evicted_key, evicted_value) = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let first_value = backend.get(first.0).await.unwrap();
            let second_value = backend.get(second.0).await.unwrap();
            match (first_value, second_value) {
                (None, _) => break (first.0, first.1.clone()),
                (_, None) => break (second.0, second.1.clone()),
                (Some(_), Some(_)) => tokio::task::yield_now().await,
            }
        }
    })
    .await
    .expect("entry-count cache did not evict either key");

    assert_eq!(
        backend
            .compare_and_set(evicted_key, &evicted_value, b"revived".to_vec(), None)
            .await
            .unwrap(),
        CacheCompareAndSetOutcome::Mismatch
    );
    assert_eq!(backend.get(evicted_key).await.unwrap(), None);
}

#[tokio::test]
async fn concurrent_local_invalidation_cannot_be_lost_to_compare_and_set() {
    let service = CacheService::from_url(None);
    let backend = service.memory_backend(Duration::from_secs(60), 256);

    for iteration in 0..128 {
        let key = format!("invalidate-race-{iteration}");
        backend.set(key.clone(), b"old".to_vec()).await.unwrap();

        let barrier = Arc::new(Barrier::new(3));
        let cas_backend = Arc::clone(&backend);
        let cas_barrier = Arc::clone(&barrier);
        let cas_key = key.clone();
        let cas = tokio::spawn(async move {
            cas_barrier.wait().await;
            cas_backend
                .compare_and_set(&cas_key, b"old", b"new".to_vec(), None)
                .await
                .unwrap()
        });

        let invalidate_backend = Arc::clone(&backend);
        let invalidate_barrier = Arc::clone(&barrier);
        let invalidate_key = key.clone();
        let invalidate = tokio::spawn(async move {
            invalidate_barrier.wait().await;
            invalidate_backend
                .invalidate(&invalidate_key)
                .await
                .unwrap();
        });

        barrier.wait().await;
        let outcome = cas.await.unwrap();
        invalidate.await.unwrap();

        assert!(matches!(
            outcome,
            CacheCompareAndSetOutcome::Applied | CacheCompareAndSetOutcome::Mismatch
        ));
        assert_eq!(backend.get(&key).await.unwrap(), None);
    }
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
            .compare_and_set("binary", &replacement, Vec::new(), Some(Duration::ZERO),)
            .await
            .unwrap(),
        CacheCompareAndSetOutcome::Applied
    );
    assert_eq!(backend.get("binary").await.unwrap(), None);
}
