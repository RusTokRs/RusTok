/// Custom Prometheus Metrics for RusToK
///
/// This module provides specialized metrics for:
/// - EventBus throughput and queue depth
/// - Circuit Breaker states
/// - Cache hit/miss rates
/// - Span counts by operation
/// - Error rates by module
use lazy_static::lazy_static;
use prometheus::{
    GaugeVec, HistogramOpts, HistogramVec, IntCounterVec, IntGauge, IntGaugeVec, Opts, Registry,
};

// ============================================================================
// EventBus Metrics
// ============================================================================

lazy_static! {
    /// Total events published through EventBus
    pub static ref EVENT_BUS_PUBLISHED_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_event_bus_published_total",
            "Total events published through EventBus"
        ),
        &["event_type", "tenant_id"]
    )
    .expect("Failed to create event_bus_published_total");

    /// Total events dispatched to handlers
    pub static ref EVENT_BUS_DISPATCHED_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_event_bus_dispatched_total",
            "Total events dispatched to handlers"
        ),
        &["event_type", "handler"]
    )
    .expect("Failed to create event_bus_dispatched_total");

    /// Current event queue depth
    pub static ref EVENT_BUS_QUEUE_DEPTH: IntGaugeVec = IntGaugeVec::new(
        Opts::new(
            "rustok_event_bus_queue_depth",
            "Current event queue depth"
        ),
        &["transport"]
    )
    .expect("Failed to create event_bus_queue_depth");

    /// Event processing duration in seconds
    pub static ref EVENT_BUS_PROCESSING_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "rustok_event_bus_processing_duration_seconds",
            "Event processing duration in seconds"
        )
        .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]),
        &["event_type", "handler"]
    )
    .expect("Failed to create event_bus_processing_duration_seconds");

    /// Event processing errors
    pub static ref EVENT_BUS_ERRORS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_event_bus_errors_total",
            "Total event processing errors"
        ),
        &["event_type", "error_type"]
    )
    .expect("Failed to create event_bus_errors_total");

    /// Event lag (time between publish and processing)
    pub static ref EVENT_BUS_LAG_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "rustok_event_bus_lag_seconds",
            "Event lag in seconds (publish to processing)"
        )
        .buckets(vec![0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0, 30.0, 60.0]),
        &["event_type"]
    )
    .expect("Failed to create event_bus_lag_seconds");

    /// Total times a consumer lagged and skipped messages
    pub static ref EVENT_CONSUMER_LAGGED_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_event_consumer_lagged_total",
            "Total times an event consumer lagged and skipped messages"
        ),
        &["consumer"]
    )
    .expect("Failed to create event_consumer_lagged_total");

    /// Total consumer loop (re)starts for long-lived subscribers
    pub static ref EVENT_CONSUMER_RESTARTED_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_event_consumer_restarted_total",
            "Total long-lived event consumer loop starts and restarts"
        ),
        &["consumer", "reason"]
    )
    .expect("Failed to create event_consumer_restarted_total");

    /// End-to-end dispatch latency in milliseconds
    pub static ref EVENT_DISPATCH_LATENCY_MS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "rustok_event_dispatch_latency_ms",
            "End-to-end event dispatch latency in milliseconds"
        )
        .buckets(vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0]),
        &["consumer", "event_type"]
    )
    .expect("Failed to create event_dispatch_latency_ms");
}

// ============================================================================
// Circuit Breaker Metrics
// ============================================================================

lazy_static! {
    /// Circuit breaker state (0=closed, 1=open, 2=half-open)
    pub static ref CIRCUIT_BREAKER_STATE: IntGaugeVec = IntGaugeVec::new(
        Opts::new(
            "rustok_circuit_breaker_state",
            "Circuit breaker state (0=closed, 1=open, 2=half-open)"
        ),
        &["service"]
    )
    .expect("Failed to create circuit_breaker_state");

    /// Circuit breaker state transitions
    pub static ref CIRCUIT_BREAKER_TRANSITIONS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_circuit_breaker_transitions_total",
            "Total circuit breaker state transitions"
        ),
        &["service", "from_state", "to_state"]
    )
    .expect("Failed to create circuit_breaker_transitions_total");

    /// Calls through circuit breaker
    pub static ref CIRCUIT_BREAKER_CALLS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_circuit_breaker_calls_total",
            "Total calls through circuit breaker"
        ),
        &["service", "result"]  // result: success, failure, rejected
    )
    .expect("Failed to create circuit_breaker_calls_total");

    /// Circuit breaker failure count
    pub static ref CIRCUIT_BREAKER_FAILURES: IntGaugeVec = IntGaugeVec::new(
        Opts::new(
            "rustok_circuit_breaker_failures",
            "Current failure count for circuit breaker"
        ),
        &["service"]
    )
    .expect("Failed to create circuit_breaker_failures");
}

// ============================================================================
// Cache Metrics
// ============================================================================

lazy_static! {
    /// Cache operations total
    pub static ref CACHE_OPERATIONS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_cache_operations_total",
            "Total cache operations"
        ),
        &["cache", "operation", "result"]  // operation: get, set, delete; result: hit, miss
    )
    .expect("Failed to create cache_operations_total");

    /// Cache hit rate (derived from operations_total)
    pub static ref CACHE_HIT_RATE: GaugeVec = GaugeVec::new(
        Opts::new(
            "rustok_cache_hit_rate",
            "Cache hit rate (0.0 to 1.0)"
        ),
        &["cache"]
    )
    .expect("Failed to create cache_hit_rate");

    /// Cache size (current entries)
    pub static ref CACHE_SIZE: IntGaugeVec = IntGaugeVec::new(
        Opts::new(
            "rustok_cache_size",
            "Current number of entries in cache"
        ),
        &["cache"]
    )
    .expect("Failed to create cache_size");

    /// Cache evictions total
    pub static ref CACHE_EVICTIONS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_cache_evictions_total",
            "Total cache evictions"
        ),
        &["cache", "reason"]  // reason: capacity, ttl, explicit
    )
    .expect("Failed to create cache_evictions_total");

    /// Cache operation duration
    pub static ref CACHE_OPERATION_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "rustok_cache_operation_duration_seconds",
            "Cache operation duration in seconds"
        )
        .buckets(vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1]),
        &["cache", "operation"]
    )
    .expect("Failed to create cache_operation_duration_seconds");
}

// ============================================================================
// Span/Trace Metrics
// ============================================================================

lazy_static! {
    /// Spans created by operation
    pub static ref SPANS_CREATED_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_spans_created_total",
            "Total spans created"
        ),
        &["operation", "tenant_id"]
    )
    .expect("Failed to create spans_created_total");

    /// Span duration by operation
    pub static ref SPAN_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "rustok_span_duration_seconds",
            "Span duration in seconds"
        )
        .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]),
        &["operation"]
    )
    .expect("Failed to create span_duration_seconds");

    /// Error spans by operation
    pub static ref SPANS_WITH_ERRORS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_spans_with_errors_total",
            "Total spans with errors"
        ),
        &["operation", "error_type"]
    )
    .expect("Failed to create spans_with_errors_total");
}

// ============================================================================
// Module-Specific Error Metrics
// ============================================================================

lazy_static! {
    /// Module entry-point invocations split by integration path.
    ///
    /// `path` label values:
    /// - `library`: call goes through rustok module/library API.
    /// - `bypass`: direct/legacy path that bypasses shared module API.
    pub static ref MODULE_ENTRYPOINT_CALLS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_module_entrypoint_calls_total",
            "Total module entry-point invocations by integration path"
        ),
        &["module", "entry_point", "path"]
    )
    .expect("Failed to create module_entrypoint_calls_total");

    /// Errors by module
    pub static ref MODULE_ERRORS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_module_errors_total",
            "Total errors by module"
        ),
        &["module", "error_type", "severity"]
    )
    .expect("Failed to create module_errors_total");

    /// Error rate (errors per second)
    pub static ref MODULE_ERROR_RATE: GaugeVec = GaugeVec::new(
        Opts::new(
            "rustok_module_error_rate",
            "Error rate by module (errors/sec)"
        ),
        &["module"]
    )
    .expect("Failed to create module_error_rate");
}

// ============================================================================
// Database Metrics
// ============================================================================

lazy_static! {
    /// Database query duration
    pub static ref DATABASE_QUERY_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "rustok_database_query_duration_seconds",
            "Database query duration in seconds"
        )
        .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]),
        &["query_type", "table"]
    )
    .expect("Failed to create database_query_duration_seconds");

    /// Database connections (active)
    pub static ref DATABASE_CONNECTIONS: IntGaugeVec = IntGaugeVec::new(
        Opts::new(
            "rustok_database_connections",
            "Active database connections"
        ),
        &["state"]  // state: active, idle
    )
    .expect("Failed to create database_connections");

    /// Database query errors
    pub static ref DATABASE_QUERY_ERRORS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_database_query_errors_total",
            "Total database query errors"
        ),
        &["query_type", "error_type"]
    )
    .expect("Failed to create database_query_errors_total");
}

// ============================================================================
// HTTP/API Metrics (enhanced)
// ============================================================================

lazy_static! {
    /// HTTP requests by endpoint
    pub static ref HTTP_REQUESTS_BY_ENDPOINT: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_http_requests_by_endpoint",
            "HTTP requests by endpoint"
        ),
        &["method", "endpoint", "status"]
    )
    .expect("Failed to create http_requests_by_endpoint");

    /// HTTP request size in bytes
    pub static ref HTTP_REQUEST_SIZE_BYTES: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "rustok_http_request_size_bytes",
            "HTTP request size in bytes"
        )
        .buckets(vec![100.0, 1000.0, 10_000.0, 100_000.0, 1_000_000.0]),
        &["method", "endpoint"]
    )
    .expect("Failed to create http_request_size_bytes");

    /// HTTP response size in bytes
    pub static ref HTTP_RESPONSE_SIZE_BYTES: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "rustok_http_response_size_bytes",
            "HTTP response size in bytes"
        )
        .buckets(vec![100.0, 1000.0, 10_000.0, 100_000.0, 1_000_000.0]),
        &["method", "endpoint"]
    )
    .expect("Failed to create http_response_size_bytes");

    /// Active HTTP connections
    pub static ref HTTP_ACTIVE_CONNECTIONS: IntGauge = IntGauge::new(
        "rustok_http_active_connections",
        "Active HTTP connections"
    )
    .expect("Failed to create http_active_connections");
}

// ============================================================================
// Registration Helper
// ============================================================================

/// Register all custom metrics with the provided registry
pub fn register_all(registry: &Registry) -> Result<(), prometheus::Error> {
    // EventBus
    registry.register(Box::new(EVENT_BUS_PUBLISHED_TOTAL.clone()))?;
    registry.register(Box::new(EVENT_BUS_DISPATCHED_TOTAL.clone()))?;
    registry.register(Box::new(EVENT_BUS_QUEUE_DEPTH.clone()))?;
    registry.register(Box::new(EVENT_BUS_PROCESSING_DURATION_SECONDS.clone()))?;
    registry.register(Box::new(EVENT_BUS_ERRORS_TOTAL.clone()))?;
    registry.register(Box::new(EVENT_BUS_LAG_SECONDS.clone()))?;
    registry.register(Box::new(EVENT_CONSUMER_LAGGED_TOTAL.clone()))?;
    registry.register(Box::new(EVENT_CONSUMER_RESTARTED_TOTAL.clone()))?;
    registry.register(Box::new(EVENT_DISPATCH_LATENCY_MS.clone()))?;

    // Circuit Breaker
    registry.register(Box::new(CIRCUIT_BREAKER_STATE.clone()))?;
    registry.register(Box::new(CIRCUIT_BREAKER_TRANSITIONS_TOTAL.clone()))?;
    registry.register(Box::new(CIRCUIT_BREAKER_CALLS_TOTAL.clone()))?;
    registry.register(Box::new(CIRCUIT_BREAKER_FAILURES.clone()))?;

    // Cache
    registry.register(Box::new(CACHE_OPERATIONS_TOTAL.clone()))?;
    registry.register(Box::new(CACHE_HIT_RATE.clone()))?;
    registry.register(Box::new(CACHE_SIZE.clone()))?;
    registry.register(Box::new(CACHE_EVICTIONS_TOTAL.clone()))?;
    registry.register(Box::new(CACHE_OPERATION_DURATION_SECONDS.clone()))?;

    // Spans/Traces
    registry.register(Box::new(SPANS_CREATED_TOTAL.clone()))?;
    registry.register(Box::new(SPAN_DURATION_SECONDS.clone()))?;
    registry.register(Box::new(SPANS_WITH_ERRORS_TOTAL.clone()))?;

    // Errors
    registry.register(Box::new(MODULE_ENTRYPOINT_CALLS_TOTAL.clone()))?;
    registry.register(Box::new(MODULE_ERRORS_TOTAL.clone()))?;
    registry.register(Box::new(MODULE_ERROR_RATE.clone()))?;

    // Database
    registry.register(Box::new(DATABASE_QUERY_DURATION_SECONDS.clone()))?;
    registry.register(Box::new(DATABASE_CONNECTIONS.clone()))?;
    registry.register(Box::new(DATABASE_QUERY_ERRORS_TOTAL.clone()))?;

    // HTTP
    registry.register(Box::new(HTTP_REQUESTS_BY_ENDPOINT.clone()))?;
    registry.register(Box::new(HTTP_REQUEST_SIZE_BYTES.clone()))?;
    registry.register(Box::new(HTTP_RESPONSE_SIZE_BYTES.clone()))?;
    registry.register(Box::new(HTTP_ACTIVE_CONNECTIONS.clone()))?;

    Ok(())
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Record EventBus event publication
pub fn record_event_published(event_type: &str, tenant_id: &str) {
    EVENT_BUS_PUBLISHED_TOTAL
        .with_label_values(&[event_type, tenant_id])
        .inc();
}

/// Record EventBus event dispatch
pub fn record_event_dispatched(event_type: &str, handler: &str) {
    EVENT_BUS_DISPATCHED_TOTAL
        .with_label_values(&[event_type, handler])
        .inc();
}

/// Update EventBus queue depth
pub fn update_queue_depth(transport: &str, depth: i64) {
    EVENT_BUS_QUEUE_DEPTH
        .with_label_values(&[transport])
        .set(depth);
}

/// Record event processing duration
pub fn record_event_processing_duration(event_type: &str, handler: &str, duration_secs: f64) {
    EVENT_BUS_PROCESSING_DURATION_SECONDS
        .with_label_values(&[event_type, handler])
        .observe(duration_secs);
}

/// Record event processing error
pub fn record_event_error(event_type: &str, error_type: &str) {
    EVENT_BUS_ERRORS_TOTAL
        .with_label_values(&[event_type, error_type])
        .inc();
}

/// Record event lag
pub fn record_event_lag(event_type: &str, lag_secs: f64) {
    EVENT_BUS_LAG_SECONDS
        .with_label_values(&[event_type])
        .observe(lag_secs);
}

/// Record that a consumer lagged and skipped messages
pub fn record_event_consumer_lagged(consumer: &str) {
    EVENT_CONSUMER_LAGGED_TOTAL
        .with_label_values(&[consumer])
        .inc();
}

/// Record a long-lived consumer loop bootstrap or restart
pub fn record_event_consumer_restarted(consumer: &str, reason: &str) {
    EVENT_CONSUMER_RESTARTED_TOTAL
        .with_label_values(&[consumer, reason])
        .inc();
}

/// Record dispatch latency in milliseconds
pub fn record_event_dispatch_latency_ms(consumer: &str, event_type: &str, latency_ms: f64) {
    EVENT_DISPATCH_LATENCY_MS
        .with_label_values(&[consumer, event_type])
        .observe(latency_ms);
}

/// Update circuit breaker state (0=closed, 1=open, 2=half-open)
pub fn update_circuit_breaker_state(service: &str, state: i64) {
    CIRCUIT_BREAKER_STATE
        .with_label_values(&[service])
        .set(state);
}

/// Record circuit breaker state transition
pub fn record_circuit_breaker_transition(service: &str, from: &str, to: &str) {
    CIRCUIT_BREAKER_TRANSITIONS_TOTAL
        .with_label_values(&[service, from, to])
        .inc();
}

/// Record circuit breaker call result
pub fn record_circuit_breaker_call(service: &str, result: &str) {
    CIRCUIT_BREAKER_CALLS_TOTAL
        .with_label_values(&[service, result])
        .inc();
}

/// Update circuit breaker failure count
pub fn update_circuit_breaker_failures(service: &str, failures: i64) {
    CIRCUIT_BREAKER_FAILURES
        .with_label_values(&[service])
        .set(failures);
}

/// Record cache operation
pub fn record_cache_operation(cache: &str, operation: &str, result: &str) {
    CACHE_OPERATIONS_TOTAL
        .with_label_values(&[cache, operation, result])
        .inc();
}

/// Update cache size
pub fn update_cache_size(cache: &str, size: i64) {
    CACHE_SIZE.with_label_values(&[cache]).set(size);
}

/// Record cache eviction
pub fn record_cache_eviction(cache: &str, reason: &str) {
    CACHE_EVICTIONS_TOTAL
        .with_label_values(&[cache, reason])
        .inc();
}

/// Record cache operation duration
pub fn record_cache_duration(cache: &str, operation: &str, duration_secs: f64) {
    CACHE_OPERATION_DURATION_SECONDS
        .with_label_values(&[cache, operation])
        .observe(duration_secs);
}

/// Record span creation
pub fn record_span_created(operation: &str, tenant_id: &str) {
    SPANS_CREATED_TOTAL
        .with_label_values(&[operation, tenant_id])
        .inc();
}

/// Record span duration
pub fn record_span_duration(operation: &str, duration_secs: f64) {
    SPAN_DURATION_SECONDS
        .with_label_values(&[operation])
        .observe(duration_secs);
}

/// Record span with error
pub fn record_span_error(operation: &str, error_type: &str) {
    SPANS_WITH_ERRORS_TOTAL
        .with_label_values(&[operation, error_type])
        .inc();
}

/// Record module error
pub fn record_module_error(module: &str, error_type: &str, severity: &str) {
    MODULE_ERRORS_TOTAL
        .with_label_values(&[module, error_type, severity])
        .inc();
}

/// Record module entry-point invocation path (`library` or `bypass`).
pub fn record_module_entrypoint_call(module: &str, entry_point: &str, path: &str) {
    MODULE_ENTRYPOINT_CALLS_TOTAL
        .with_label_values(&[module, entry_point, path])
        .inc();
}

/// Record database query duration
pub fn record_db_query_duration(query_type: &str, table: &str, duration_secs: f64) {
    DATABASE_QUERY_DURATION_SECONDS
        .with_label_values(&[query_type, table])
        .observe(duration_secs);
}

/// Update database connections
pub fn update_db_connections(state: &str, count: i64) {
    DATABASE_CONNECTIONS.with_label_values(&[state]).set(count);
}

/// Record database query error
pub fn record_db_query_error(query_type: &str, error_type: &str) {
    DATABASE_QUERY_ERRORS_TOTAL
        .with_label_values(&[query_type, error_type])
        .inc();
}
