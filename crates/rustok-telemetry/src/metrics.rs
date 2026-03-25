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
    /// - `library`: call goes through rustok shared module/library API.
    /// - `core_runtime`: call is served by platform kernel path (`apps/server` + core crates).
    /// - `bypass`: direct/legacy path that should be migrated away from shared contracts.
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

    /// Requested read-path limits before clamping
    pub static ref READ_PATH_REQUESTED_LIMIT: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "rustok_read_path_requested_limit",
            "Requested read-path limits before clamp/default handling"
        )
        .buckets(vec![1.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0]),
        &["surface", "path"]
    )
    .expect("Failed to create read_path_requested_limit");

    /// Effective read-path limits after clamping/default handling
    pub static ref READ_PATH_EFFECTIVE_LIMIT: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "rustok_read_path_effective_limit",
            "Effective read-path limits after clamp/default handling"
        )
        .buckets(vec![1.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0]),
        &["surface", "path"]
    )
    .expect("Failed to create read_path_effective_limit");

    /// Number of items returned by bounded read paths
    pub static ref READ_PATH_RETURNED_ITEMS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "rustok_read_path_returned_items",
            "Items returned by bounded read paths"
        )
        .buckets(vec![0.0, 1.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0]),
        &["surface", "path"]
    )
    .expect("Failed to create read_path_returned_items");

    /// Total times a requested limit had to be clamped to fit runtime budget
    pub static ref READ_PATH_LIMIT_CLAMPED_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_read_path_limit_clamped_total",
            "Total times a read-path limit was clamped to runtime budget"
        ),
        &["surface", "path"]
    )
    .expect("Failed to create read_path_limit_clamped_total");

    /// Query latency inside bounded read paths
    pub static ref READ_PATH_QUERY_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "rustok_read_path_query_duration_seconds",
            "Database/query step duration for bounded read paths"
        )
        .buckets(vec![0.0005, 0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5]),
        &["surface", "path", "query"]
    )
    .expect("Failed to create read_path_query_duration_seconds");

    /// Rows or aggregate volume observed per bounded read-path query step
    pub static ref READ_PATH_QUERY_ROWS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "rustok_read_path_query_rows",
            "Rows or aggregate volume observed per bounded read-path query step"
        )
        .buckets(vec![0.0, 1.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 5000.0]),
        &["surface", "path", "query"]
    )
    .expect("Failed to create read_path_query_rows");

    /// Reindex run lifecycle transitions
    pub static ref INDEX_REINDEX_RUNS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_index_reindex_runs_total",
            "Total reindex runs by indexer, operation, and status"
        ),
        &["indexer", "operation", "status"]
    )
    .expect("Failed to create index_reindex_runs_total");

    /// Reindex entity totals by outcome
    pub static ref INDEX_REINDEX_ENTITIES_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_index_reindex_entities_total",
            "Total entities observed by reindex runs"
        ),
        &["indexer", "operation", "outcome"]
    )
    .expect("Failed to create index_reindex_entities_total");

    /// Reindex run duration in seconds
    pub static ref INDEX_REINDEX_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "rustok_index_reindex_duration_seconds",
            "Duration of reindex runs in seconds"
        )
        .buckets(vec![0.001, 0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0]),
        &["indexer", "operation"]
    )
    .expect("Failed to create index_reindex_duration_seconds");

    /// Current runtime config values exposed for operators
    pub static ref INDEX_REINDEX_RUNTIME_CONFIG: IntGaugeVec = IntGaugeVec::new(
        Opts::new(
            "rustok_index_reindex_runtime_config",
            "Current configured runtime values for reindex workers"
        ),
        &["indexer", "setting"]
    )
    .expect("Failed to create index_reindex_runtime_config");

    /// Search query executions by surface/engine/status.
    pub static ref SEARCH_QUERIES_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_search_queries_total",
            "Total search query executions"
        ),
        &["surface", "engine", "status"]
    )
    .expect("Failed to create search_queries_total");

    /// Search query duration in seconds.
    pub static ref SEARCH_QUERY_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "rustok_search_query_duration_seconds",
            "Search query duration in seconds"
        )
        .buckets(vec![0.0005, 0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5]),
        &["surface", "engine"]
    )
    .expect("Failed to create search_query_duration_seconds");

    /// Search results returned by surface/engine.
    pub static ref SEARCH_RESULTS_RETURNED: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "rustok_search_results_returned",
            "Search results returned"
        )
        .buckets(vec![0.0, 1.0, 5.0, 10.0, 20.0, 50.0, 100.0]),
        &["surface", "engine"]
    )
    .expect("Failed to create search_results_returned");

    /// Zero-result searches by surface/engine.
    pub static ref SEARCH_ZERO_RESULTS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_search_zero_results_total",
            "Total zero-result search queries"
        ),
        &["surface", "engine"]
    )
    .expect("Failed to create search_zero_results_total");

    /// Search indexing/rebuild operations by operation/entity/status.
    pub static ref SEARCH_INDEXING_OPERATIONS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_search_indexing_operations_total",
            "Total search indexing and rebuild operations"
        ),
        &["operation", "entity", "status"]
    )
    .expect("Failed to create search_indexing_operations_total");

    /// Search indexing/rebuild duration in seconds.
    pub static ref SEARCH_INDEXING_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "rustok_search_indexing_duration_seconds",
            "Search indexing and rebuild duration in seconds"
        )
        .buckets(vec![0.001, 0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0]),
        &["operation", "entity"]
    )
    .expect("Failed to create search_indexing_duration_seconds");

    /// Current rate-limit backend health by namespace/backend
    pub static ref RATE_LIMIT_BACKEND_STATUS: IntGaugeVec = IntGaugeVec::new(
        Opts::new(
            "rustok_rate_limit_backend_status",
            "Current rate-limit backend health (1=healthy, 0=unhealthy)"
        ),
        &["namespace", "backend"]
    )
    .expect("Failed to create rate_limit_backend_status");

    /// Current number of active clients tracked by a limiter
    pub static ref RATE_LIMIT_ACTIVE_CLIENTS: IntGaugeVec = IntGaugeVec::new(
        Opts::new(
            "rustok_rate_limit_active_clients",
            "Current number of active clients tracked by a limiter"
        ),
        &["namespace"]
    )
    .expect("Failed to create rate_limit_active_clients");

    /// Current number of internal limiter entries
    pub static ref RATE_LIMIT_TOTAL_ENTRIES: IntGaugeVec = IntGaugeVec::new(
        Opts::new(
            "rustok_rate_limit_total_entries",
            "Current number of internal rate-limit entries"
        ),
        &["namespace"]
    )
    .expect("Failed to create rate_limit_total_entries");

    /// Whether the limiter is running in distributed mode
    pub static ref RATE_LIMIT_DISTRIBUTED_MODE: IntGaugeVec = IntGaugeVec::new(
        Opts::new(
            "rustok_rate_limit_distributed_mode",
            "Whether the limiter is running in distributed mode (1=yes, 0=no)"
        ),
        &["namespace"]
    )
    .expect("Failed to create rate_limit_distributed_mode");

    /// Total backend-unavailable failures encountered by rate limiting
    pub static ref RATE_LIMIT_BACKEND_UNAVAILABLE_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_rate_limit_backend_unavailable_total",
            "Total rate-limit backend unavailable failures"
        ),
        &["namespace"]
    )
    .expect("Failed to create rate_limit_backend_unavailable_total");

    /// Total rate-limit exceeded outcomes
    pub static ref RATE_LIMIT_EXCEEDED_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_rate_limit_exceeded_total",
            "Total rate-limit exceeded outcomes"
        ),
        &["namespace"]
    )
    .expect("Failed to create rate_limit_exceeded_total");
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
    registry.register(Box::new(READ_PATH_REQUESTED_LIMIT.clone()))?;
    registry.register(Box::new(READ_PATH_EFFECTIVE_LIMIT.clone()))?;
    registry.register(Box::new(READ_PATH_RETURNED_ITEMS.clone()))?;
    registry.register(Box::new(READ_PATH_LIMIT_CLAMPED_TOTAL.clone()))?;
    registry.register(Box::new(READ_PATH_QUERY_DURATION_SECONDS.clone()))?;
    registry.register(Box::new(READ_PATH_QUERY_ROWS.clone()))?;
    registry.register(Box::new(INDEX_REINDEX_RUNS_TOTAL.clone()))?;
    registry.register(Box::new(INDEX_REINDEX_ENTITIES_TOTAL.clone()))?;
    registry.register(Box::new(INDEX_REINDEX_DURATION_SECONDS.clone()))?;
    registry.register(Box::new(INDEX_REINDEX_RUNTIME_CONFIG.clone()))?;
    registry.register(Box::new(SEARCH_QUERIES_TOTAL.clone()))?;
    registry.register(Box::new(SEARCH_QUERY_DURATION_SECONDS.clone()))?;
    registry.register(Box::new(SEARCH_RESULTS_RETURNED.clone()))?;
    registry.register(Box::new(SEARCH_ZERO_RESULTS_TOTAL.clone()))?;
    registry.register(Box::new(SEARCH_INDEXING_OPERATIONS_TOTAL.clone()))?;
    registry.register(Box::new(SEARCH_INDEXING_DURATION_SECONDS.clone()))?;
    registry.register(Box::new(RATE_LIMIT_BACKEND_STATUS.clone()))?;
    registry.register(Box::new(RATE_LIMIT_ACTIVE_CLIENTS.clone()))?;
    registry.register(Box::new(RATE_LIMIT_TOTAL_ENTRIES.clone()))?;
    registry.register(Box::new(RATE_LIMIT_DISTRIBUTED_MODE.clone()))?;
    registry.register(Box::new(RATE_LIMIT_BACKEND_UNAVAILABLE_TOTAL.clone()))?;
    registry.register(Box::new(RATE_LIMIT_EXCEEDED_TOTAL.clone()))?;

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

/// Record module entry-point invocation path (`library`, `core_runtime`, or `bypass`).
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

/// Record runtime budgets for bounded read-paths.
pub fn record_read_path_budget(
    surface: &str,
    path: &str,
    requested_limit: Option<u64>,
    effective_limit: u64,
    returned_items: usize,
) {
    if let Some(requested_limit) = requested_limit {
        READ_PATH_REQUESTED_LIMIT
            .with_label_values(&[surface, path])
            .observe(requested_limit as f64);
        if requested_limit != effective_limit {
            READ_PATH_LIMIT_CLAMPED_TOTAL
                .with_label_values(&[surface, path])
                .inc();
        }
    }

    READ_PATH_EFFECTIVE_LIMIT
        .with_label_values(&[surface, path])
        .observe(effective_limit as f64);
    READ_PATH_RETURNED_ITEMS
        .with_label_values(&[surface, path])
        .observe(returned_items as f64);
}

/// Record latency and row volume for an individual query step inside a read path.
pub fn record_read_path_query(
    surface: &str,
    path: &str,
    query: &str,
    duration_secs: f64,
    rows: u64,
) {
    READ_PATH_QUERY_DURATION_SECONDS
        .with_label_values(&[surface, path, query])
        .observe(duration_secs);
    READ_PATH_QUERY_ROWS
        .with_label_values(&[surface, path, query])
        .observe(rows as f64);
}

/// Record operator-visible runtime config for an indexer.
pub fn record_index_reindex_runtime_config(
    indexer: &str,
    parallelism: usize,
    entity_budget: usize,
    yield_every: u64,
) {
    INDEX_REINDEX_RUNTIME_CONFIG
        .with_label_values(&[indexer, "parallelism"])
        .set(parallelism as i64);
    INDEX_REINDEX_RUNTIME_CONFIG
        .with_label_values(&[indexer, "entity_budget"])
        .set(entity_budget as i64);
    INDEX_REINDEX_RUNTIME_CONFIG
        .with_label_values(&[indexer, "yield_every"])
        .set(yield_every as i64);
}

/// Record a lifecycle transition for a reindex run.
pub fn record_index_reindex_run(indexer: &str, operation: &str, status: &str) {
    INDEX_REINDEX_RUNS_TOTAL
        .with_label_values(&[indexer, operation, status])
        .inc();
}

/// Record entity volume for a reindex run.
pub fn record_index_reindex_entities(indexer: &str, operation: &str, outcome: &str, count: u64) {
    if count == 0 {
        return;
    }

    INDEX_REINDEX_ENTITIES_TOTAL
        .with_label_values(&[indexer, operation, outcome])
        .inc_by(count);
}

/// Record the total duration of a reindex run.
pub fn record_index_reindex_duration(indexer: &str, operation: &str, duration_secs: f64) {
    INDEX_REINDEX_DURATION_SECONDS
        .with_label_values(&[indexer, operation])
        .observe(duration_secs);
}

/// Record a search query execution.
pub fn record_search_query(
    surface: &str,
    engine: &str,
    status: &str,
    duration_secs: f64,
    returned_items: u64,
) {
    SEARCH_QUERIES_TOTAL
        .with_label_values(&[surface, engine, status])
        .inc();
    SEARCH_QUERY_DURATION_SECONDS
        .with_label_values(&[surface, engine])
        .observe(duration_secs);
    SEARCH_RESULTS_RETURNED
        .with_label_values(&[surface, engine])
        .observe(returned_items as f64);
    if returned_items == 0 && status == "success" {
        SEARCH_ZERO_RESULTS_TOTAL
            .with_label_values(&[surface, engine])
            .inc();
    }
}

/// Record a search indexing or rebuild operation.
pub fn record_search_indexing_operation(
    operation: &str,
    entity: &str,
    status: &str,
    duration_secs: f64,
) {
    SEARCH_INDEXING_OPERATIONS_TOTAL
        .with_label_values(&[operation, entity, status])
        .inc();
    SEARCH_INDEXING_DURATION_SECONDS
        .with_label_values(&[operation, entity])
        .observe(duration_secs);
}

/// Update observable runtime stats for a rate limiter.
pub fn update_rate_limit_runtime(
    namespace: &str,
    backend: &str,
    distributed: bool,
    active_clients: usize,
    total_entries: usize,
    healthy: bool,
) {
    RATE_LIMIT_BACKEND_STATUS
        .with_label_values(&[namespace, backend])
        .set(if healthy { 1 } else { 0 });
    RATE_LIMIT_ACTIVE_CLIENTS
        .with_label_values(&[namespace])
        .set(active_clients as i64);
    RATE_LIMIT_TOTAL_ENTRIES
        .with_label_values(&[namespace])
        .set(total_entries as i64);
    RATE_LIMIT_DISTRIBUTED_MODE
        .with_label_values(&[namespace])
        .set(if distributed { 1 } else { 0 });
}

/// Record a backend-unavailable rate-limit outcome.
pub fn record_rate_limit_backend_unavailable(namespace: &str) {
    RATE_LIMIT_BACKEND_UNAVAILABLE_TOTAL
        .with_label_values(&[namespace])
        .inc();
}

/// Record a rate-limit exceeded outcome.
pub fn record_rate_limit_exceeded(namespace: &str) {
    RATE_LIMIT_EXCEEDED_TOTAL
        .with_label_values(&[namespace])
        .inc();
}

// ============================================================================
// Media Metrics
// ============================================================================

lazy_static! {
    /// Total media files uploaded, by tenant and MIME category (image/video/…).
    pub static ref MEDIA_UPLOADS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("rustok_media_uploads_total", "Total media files uploaded"),
        &["tenant_id", "mime_category"],
    )
    .expect("Failed to create media_uploads_total");

    /// Total bytes uploaded, by tenant.
    pub static ref MEDIA_UPLOAD_BYTES_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_media_upload_bytes_total",
            "Total bytes of media uploaded",
        ),
        &["tenant_id"],
    )
    .expect("Failed to create media_upload_bytes_total");

    /// Total media files deleted, by tenant.
    pub static ref MEDIA_DELETES_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("rustok_media_deletes_total", "Total media files deleted"),
        &["tenant_id"],
    )
    .expect("Failed to create media_deletes_total");

    /// Storage health status: 1 = healthy, 0 = unhealthy.
    pub static ref MEDIA_STORAGE_HEALTH: IntGaugeVec = IntGaugeVec::new(
        Opts::new(
            "rustok_media_storage_health",
            "Storage backend health: 1=healthy 0=unhealthy",
        ),
        &["driver"],
    )
    .expect("Failed to create media_storage_health");
}

/// Record a successful media upload.
pub fn record_media_upload(tenant_id: &str, mime_type: &str, bytes: u64) {
    let category = mime_type.split('/').next().unwrap_or("other");
    MEDIA_UPLOADS_TOTAL
        .with_label_values(&[tenant_id, category])
        .inc();
    MEDIA_UPLOAD_BYTES_TOTAL
        .with_label_values(&[tenant_id])
        .inc_by(bytes);
}

/// Record a media deletion.
pub fn record_media_delete(tenant_id: &str) {
    MEDIA_DELETES_TOTAL.with_label_values(&[tenant_id]).inc();
}

/// Update storage backend health.
pub fn update_storage_health(driver: &str, healthy: bool) {
    MEDIA_STORAGE_HEALTH
        .with_label_values(&[driver])
        .set(if healthy { 1 } else { 0 });
}
