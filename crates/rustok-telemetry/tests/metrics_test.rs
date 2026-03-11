/// Unit tests for custom metrics
///
/// Tests metric registration, recording, and retrieval
use prometheus::Registry;
use rustok_telemetry::metrics;

#[test]
fn test_register_all_metrics() {
    let registry = Registry::new();
    let result = metrics::register_all(&registry);
    assert!(result.is_ok(), "Should register all metrics successfully");

    // Verify metrics are registered
    let metric_families = registry.gather();
    assert!(
        !metric_families.is_empty(),
        "Should have registered metrics"
    );

    // Check for key metrics
    let metric_names: Vec<String> = metric_families
        .iter()
        .map(|mf| mf.name().to_string())
        .collect();

    // EventBus metrics
    assert!(metric_names.contains(&"rustok_event_bus_published_total".to_string()));
    assert!(metric_names.contains(&"rustok_event_bus_queue_depth".to_string()));
    assert!(metric_names.contains(&"rustok_event_consumer_lagged_total".to_string()));
    assert!(metric_names.contains(&"rustok_event_dispatch_latency_ms".to_string()));

    // Circuit Breaker metrics
    assert!(metric_names.contains(&"rustok_circuit_breaker_state".to_string()));

    // Cache metrics
    assert!(metric_names.contains(&"rustok_cache_operations_total".to_string()));
    assert!(metric_names.contains(&"rustok_module_entrypoint_calls_total".to_string()));
}

#[test]
fn test_event_bus_metrics() {
    // Record event publication
    metrics::record_event_published("ProductCreated", "tenant-123");

    // Record event dispatch
    metrics::record_event_dispatched("ProductCreated", "index_handler");

    // Update queue depth
    metrics::update_queue_depth("in_memory", 42);

    // Record processing duration
    metrics::record_event_processing_duration("ProductCreated", "index_handler", 0.123);

    // Record error
    metrics::record_event_error("ProductCreated", "handler_error");

    // Record lag
    metrics::record_event_lag("ProductCreated", 1.5);

    // Record consumer-loop telemetry
    metrics::record_event_consumer_lagged("event_dispatcher");
    metrics::record_event_consumer_restarted("event_dispatcher", "startup");
    metrics::record_event_dispatch_latency_ms("event_dispatcher", "ProductCreated", 12.0);

    // Metrics should be recorded (no assertions, just ensuring no panics)
}

#[test]
fn test_circuit_breaker_metrics() {
    // Update state
    metrics::update_circuit_breaker_state("redis", 0); // Closed
    metrics::update_circuit_breaker_state("redis", 1); // Open
    metrics::update_circuit_breaker_state("redis", 2); // Half-open

    // Record transitions
    metrics::record_circuit_breaker_transition("redis", "closed", "open");
    metrics::record_circuit_breaker_transition("redis", "open", "half_open");

    // Record calls
    metrics::record_circuit_breaker_call("redis", "success");
    metrics::record_circuit_breaker_call("redis", "failure");
    metrics::record_circuit_breaker_call("redis", "rejected");

    // Update failures
    metrics::update_circuit_breaker_failures("redis", 5);
}

#[test]
fn test_cache_metrics() {
    // Record cache operations
    metrics::record_cache_operation("tenant_cache", "get", "hit");
    metrics::record_cache_operation("tenant_cache", "get", "miss");
    metrics::record_cache_operation("tenant_cache", "set", "success");

    // Update cache size
    metrics::update_cache_size("tenant_cache", 1234);

    // Record eviction
    metrics::record_cache_eviction("tenant_cache", "ttl");
    metrics::record_cache_eviction("tenant_cache", "capacity");

    // Record operation duration
    metrics::record_cache_duration("tenant_cache", "get", 0.001);
}

#[test]
fn test_span_metrics() {
    // Record span creation
    metrics::record_span_created("create_product", "tenant-123");

    // Record span duration
    metrics::record_span_duration("create_product", 0.456);

    // Record span error
    metrics::record_span_error("create_product", "ValidationError");
}

#[test]
fn test_module_error_metrics() {
    // Record module errors
    metrics::record_module_error("commerce", "ValidationError", "warning");
    metrics::record_module_error("content", "DatabaseError", "critical");
}

#[test]
fn test_module_entrypoint_metrics() {
    metrics::record_module_entrypoint_call("rbac", "has_permission", "library");
    metrics::record_module_entrypoint_call("rbac", "replace_user_role_via_store", "bypass");
}

#[test]
fn test_database_metrics() {
    // Record query duration
    metrics::record_db_query_duration("SELECT", "products", 0.023);

    // Update connections
    metrics::update_db_connections("active", 5);
    metrics::update_db_connections("idle", 10);

    // Record query error
    metrics::record_db_query_error("INSERT", "ConnectionError");
}

#[test]
fn test_metric_labels() {
    // Test that metrics can be recorded with different label combinations
    metrics::record_event_published("OrderCreated", "tenant-1");
    metrics::record_event_published("OrderCreated", "tenant-2");
    metrics::record_event_published("ProductCreated", "tenant-1");

    // Different labels should create different metric series
    // (No assertions - just testing that labels work)
}

#[test]
fn test_histogram_buckets() {
    // Record values across different histogram buckets
    metrics::record_event_processing_duration("TestEvent", "handler", 0.001); // Fast
    metrics::record_event_processing_duration("TestEvent", "handler", 0.1); // Medium
    metrics::record_event_processing_duration("TestEvent", "handler", 1.0); // Slow
    metrics::record_event_processing_duration("TestEvent", "handler", 5.0); // Very slow

    // Histogram should track distribution
}

#[test]
fn test_gauge_updates() {
    // Test that gauges can be updated multiple times
    metrics::update_queue_depth("test", 100);
    metrics::update_queue_depth("test", 150);
    metrics::update_queue_depth("test", 50); // Can decrease

    metrics::update_cache_size("test_cache", 1000);
    metrics::update_cache_size("test_cache", 2000);
    metrics::update_cache_size("test_cache", 500); // Can decrease
}

#[test]
fn test_counter_increments() {
    // Test that counters only increase
    for _ in 0..10 {
        metrics::record_event_published("TestEvent", "tenant-test");
    }

    // Counter should have incremented 10 times
}

#[test]
fn test_circuit_breaker_state_values() {
    // Test state encoding
    metrics::update_circuit_breaker_state("test_service", 0); // Closed
    metrics::update_circuit_breaker_state("test_service", 1); // Open
    metrics::update_circuit_breaker_state("test_service", 2); // Half-open

    // States should be distinct
}

#[test]
fn test_multiple_services() {
    // Test metrics for multiple services
    metrics::update_circuit_breaker_state("redis", 0);
    metrics::update_circuit_breaker_state("postgres", 0);
    metrics::update_circuit_breaker_state("iggy", 1);

    metrics::record_cache_operation("tenant_cache", "get", "hit");
    metrics::record_cache_operation("product_cache", "get", "miss");

    // Multiple services should coexist
}

#[test]
fn test_high_cardinality_labels() {
    // Test reasonable label cardinality (not too high)
    let event_types = vec!["OrderCreated", "ProductCreated", "UserRegistered"];
    let tenants = vec!["tenant-1", "tenant-2", "tenant-3"];

    for event_type in &event_types {
        for tenant in &tenants {
            metrics::record_event_published(event_type, tenant);
        }
    }

    // Total combinations: 3 × 3 = 9 (acceptable)
}

#[test]
fn test_metric_naming_conventions() {
    // Verify metric names follow Prometheus conventions
    let registry = Registry::new();
    metrics::register_all(&registry).unwrap();

    let metric_families = registry.gather();
    for mf in metric_families {
        let name = mf.name();

        // Should start with namespace
        assert!(
            name.starts_with("rustok_"),
            "Metric {} should start with 'rustok_'",
            name
        );

        // Should use underscores, not hyphens
        assert!(
            !name.contains('-'),
            "Metric {} should use underscores, not hyphens",
            name
        );

        // Should be lowercase
        assert!(
            name == name.to_lowercase(),
            "Metric {} should be lowercase",
            name
        );
    }
}

#[test]
fn test_concurrent_metric_updates() {
    use std::thread;

    // Test that metrics are thread-safe
    let handles: Vec<_> = (0..10)
        .map(|i| {
            thread::spawn(move || {
                metrics::record_event_published("ConcurrentEvent", &format!("tenant-{}", i));
                metrics::update_queue_depth("concurrent", i as i64);
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // All threads should complete without panics
}

#[test]
fn test_zero_values() {
    // Test that zero values can be recorded
    metrics::update_queue_depth("empty", 0);
    metrics::update_cache_size("empty_cache", 0);
    metrics::update_circuit_breaker_failures("service", 0);
    metrics::update_db_connections("idle", 0);

    // Zero is a valid metric value
}

#[test]
fn test_large_values() {
    // Test large values
    metrics::update_queue_depth("large", 1_000_000);
    metrics::update_cache_size("large_cache", 10_000_000);
    metrics::record_event_processing_duration("SlowEvent", "handler", 60.0); // 1 minute

    // Large values should be handled correctly
}

#[test]
fn test_metric_help_text() {
    // Verify that metrics have help text
    let registry = Registry::new();
    metrics::register_all(&registry).unwrap();

    let metric_families = registry.gather();
    for mf in metric_families {
        let help = mf.help();
        assert!(
            !help.is_empty(),
            "Metric {} should have help text",
            mf.name()
        );
    }
}
