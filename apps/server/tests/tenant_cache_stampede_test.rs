use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_singleflight_pattern_coalesces_concurrent_requests() {
    use std::collections::HashMap;
    use tokio::sync::{Mutex, Notify};

    let in_flight: Arc<Mutex<HashMap<String, Arc<Notify>>>> = Arc::new(Mutex::new(HashMap::new()));
    let db_query_count = Arc::new(AtomicU64::new(0));
    let cache_key = "tenant:uuid:12345".to_string();

    let mut handles = vec![];

    for i in 0..100 {
        let in_flight = in_flight.clone();
        let counter = db_query_count.clone();
        let key = cache_key.clone();

        let handle = tokio::spawn(async move {
            let notify = {
                let mut map = in_flight.lock().await;

                if let Some(existing) = map.get(&key) {
                    let notify = existing.clone();
                    drop(map);
                    notify.notified().await;
                    return;
                }

                let notify = Arc::new(Notify::new());
                map.insert(key.clone(), notify.clone());
                notify
            };

            sleep(Duration::from_millis(50)).await;
            counter.fetch_add(1, Ordering::SeqCst);

            {
                let mut map = in_flight.lock().await;
                map.remove(&key);
            }
            notify.notify_waiters();
        });

        handles.push(handle);

        if i < 10 {
            sleep(Duration::from_micros(100)).await;
        }
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let queries = db_query_count.load(Ordering::SeqCst);
    assert!(
        queries <= 5,
        "Singleflight should coalesce concurrent requests"
    );
}

#[tokio::test]
async fn test_singleflight_pattern_handles_racy_arrival() {
    use std::collections::HashMap;
    use tokio::sync::{Mutex, Notify};

    // Simulate the singleflight pattern
    let in_flight: Arc<Mutex<HashMap<String, Arc<Notify>>>> = Arc::new(Mutex::new(HashMap::new()));
    let db_query_count = Arc::new(AtomicU64::new(0));
    let cache_key = "tenant:uuid:12345".to_string();

    let mut handles = vec![];

    // Simulate 100 concurrent requests for the same tenant
    for i in 0..100 {
        let in_flight = in_flight.clone();
        let counter = db_query_count.clone();
        let key = cache_key.clone();

        let handle = tokio::spawn(async move {
            let notify = {
                let mut map = in_flight.lock().await;

                if let Some(existing) = map.get(&key) {
                    let notify = existing.clone();
                    drop(map);

                    // Wait for the in-flight request to complete
                    notify.notified().await;

                    // Check cache (would return result in real implementation)
                    return;
                }

                // First request - insert notify
                let notify = Arc::new(Notify::new());
                map.insert(key.clone(), notify.clone());
                notify
            };

            // Simulate DB query (only first request does this)
            sleep(Duration::from_millis(50)).await;
            counter.fetch_add(1, Ordering::SeqCst);

            // Remove from in-flight and notify waiters
            {
                let mut map = in_flight.lock().await;
                map.remove(&key);
            }
            notify.notify_waiters();
        });

        handles.push(handle);

        // Small delay to ensure some requests arrive simultaneously
        if i < 10 {
            sleep(Duration::from_micros(100)).await;
        }
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let queries = db_query_count.load(Ordering::SeqCst);

    // With singleflight pattern, only 1 request should hit the DB
    println!("Total DB queries with singleflight: {}", queries);
    assert!(
        queries <= 5,
        "With singleflight, only a few requests should hit the DB (ideally 1, but allowing for race conditions in test)"
    );
}

#[test]
fn test_tenant_cache_stats_includes_coalesced_metric() {
    // Verify that TenantCacheStats includes the coalesced_requests field

    // This would be a compile-time check - if the code compiles, the field exists
    let stats = rustok_server::middleware::tenant::TenantCacheStats {
        hits: 100,
        misses: 10,
        evictions: 5,
        negative_hits: 2,
        negative_misses: 8,
        negative_evictions: 1,
        entries: 50,
        negative_entries: 3,
        negative_inserts: 2,
        coalesced_requests: 45, // New field
        invalidation_listener_status: 2,
    };

    assert_eq!(stats.coalesced_requests, 45);
    println!("✓ TenantCacheStats includes coalesced_requests field");
}
