from pathlib import Path


def replace_once(path: Path, old: str, new: str, label: str) -> None:
    source = path.read_text()
    count = source.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: {label}: expected 1 match, got {count}")
    path.write_text(source.replace(old, new, 1))


metrics = Path("crates/rustok-telemetry/src/metrics.rs")
replace_once(
    metrics,
    '''// ============================================================================
// Span/Trace Metrics
// ============================================================================
''',
    '''// ============================================================================
// Tenant Resolution Metrics
// ============================================================================

lazy_static! {
    /// Tenant resolution outcomes by transport, typed source and final outcome.
    pub static ref TENANT_RESOLUTIONS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "rustok_tenant_resolutions_total",
            "Total tenant resolution outcomes by transport, source and outcome"
        ),
        &["transport", "source", "outcome"]
    )
    .expect("Failed to create tenant_resolutions_total");
}

// ============================================================================
// Span/Trace Metrics
// ============================================================================
''',
    "tenant metric declaration",
)
replace_once(
    metrics,
    '''    registry.register(Box::new(CACHE_OPERATION_DURATION_SECONDS.clone()))?;

    // Spans/Traces
''',
    '''    registry.register(Box::new(CACHE_OPERATION_DURATION_SECONDS.clone()))?;

    // Tenant resolution
    registry.register(Box::new(TENANT_RESOLUTIONS_TOTAL.clone()))?;

    // Spans/Traces
''',
    "tenant metric registration",
)
replace_once(
    metrics,
    '''pub fn record_cache_operation(cache: &str, operation: &str, result: &str) {
    CACHE_OPERATIONS_TOTAL
        .with_label_values(&[cache, operation, result])
        .inc();
}

/// Update cache size
''',
    '''pub fn record_cache_operation(cache: &str, operation: &str, result: &str) {
    CACHE_OPERATIONS_TOTAL
        .with_label_values(&[cache, operation, result])
        .inc();
}

/// Record a tenant resolution outcome with bounded labels.
pub fn record_tenant_resolution(transport: &str, source: &str, outcome: &str) {
    TENANT_RESOLUTIONS_TOTAL
        .with_label_values(&[transport, source, outcome])
        .inc();
}

/// Update cache size
''',
    "tenant metric helper",
)

metrics_test = Path("crates/rustok-telemetry/tests/metrics_test.rs")
replace_once(
    metrics_test,
    '''    metrics::record_cache_operation("tenant_cache", "get", "hit");
    metrics::record_module_entrypoint_call("catalog", "create_product", "success");
''',
    '''    metrics::record_cache_operation("tenant_cache", "get", "hit");
    metrics::record_tenant_resolution("http", "header", "success");
    metrics::record_module_entrypoint_call("catalog", "create_product", "success");
''',
    "seed tenant metric",
)
replace_once(
    metrics_test,
    '''    assert!(metric_names.contains(&"rustok_cache_operations_total".to_string()));
    assert!(metric_names.contains(&"rustok_module_entrypoint_calls_total".to_string()));
''',
    '''    assert!(metric_names.contains(&"rustok_cache_operations_total".to_string()));
    assert!(metric_names.contains(&"rustok_tenant_resolutions_total".to_string()));
    assert!(metric_names.contains(&"rustok_module_entrypoint_calls_total".to_string()));
''',
    "assert tenant metric registered",
)
replace_once(
    metrics_test,
    '''#[test]
fn test_span_metrics() {
''',
    '''#[test]
fn test_tenant_resolution_metrics() {
    metrics::record_tenant_resolution("http", "header", "success");
    metrics::record_tenant_resolution("http", "development_fallback", "success");
    metrics::record_tenant_resolution("graphql_ws", "self_resolving_handshake", "not_found");
}

#[test]
fn test_span_metrics() {
''',
    "tenant metric test",
)

runtime = Path("apps/server/src/middleware/tenant.rs")
replace_once(
    runtime,
    '''#[derive(Debug)]
pub(crate) enum TenantContextLoadError {
''',
    '''#[derive(Debug, Clone, Copy)]
enum TenantResolutionTransport {
    Http,
    GraphqlWebSocket,
}

impl TenantResolutionTransport {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::GraphqlWebSocket => "graphql_ws",
        }
    }
}

#[derive(Debug)]
pub(crate) enum TenantContextLoadError {
''',
    "typed tenant transport",
)
replace_once(
    runtime,
    '''    pub(crate) const fn client_message(&self) -> &'static str {
        match self {
            Self::InvalidIdentifier(_) => "Invalid tenant identifier",
            Self::InvalidAssertion(_) => "Conflicting tenant assertions",
            Self::NotFound => "Tenant not found",
            Self::Disabled => "Tenant is disabled",
            Self::InfrastructureUnavailable
            | Self::CacheUnavailable(_)
            | Self::ClockUnavailable(_)
            | Self::BackendUnavailable(_) => "Failed to resolve tenant",
        }
    }
''',
    '''    pub(crate) const fn client_message(&self) -> &'static str {
        match self {
            Self::InvalidIdentifier(_) => "Invalid tenant identifier",
            Self::InvalidAssertion(_) => "Conflicting tenant assertions",
            Self::NotFound => "Tenant not found",
            Self::Disabled => "Tenant is disabled",
            Self::InfrastructureUnavailable
            | Self::CacheUnavailable(_)
            | Self::ClockUnavailable(_)
            | Self::BackendUnavailable(_) => "Failed to resolve tenant",
        }
    }

    const fn metric_outcome(&self) -> &'static str {
        match self {
            Self::InvalidIdentifier(_) => "invalid_identifier",
            Self::InvalidAssertion(_) => "invalid_assertion",
            Self::InfrastructureUnavailable => "infrastructure_unavailable",
            Self::NotFound => "not_found",
            Self::Disabled => "disabled",
            Self::CacheUnavailable(_) => "cache_unavailable",
            Self::ClockUnavailable(_) => "clock_unavailable",
            Self::BackendUnavailable(_) => "backend_unavailable",
        }
    }
''',
    "tenant error metric outcome",
)
old_runtime = '''fn record_resolution_source(source: TenantResolutionSource) {
    rustok_telemetry::metrics::record_cache_operation(
        "tenant_resolution",
        "resolve",
        source.as_str(),
    );
}

async fn load_resolved_tenant_context(
    ctx: &ServerRuntimeContext,
    resolution: &TenantResolution,
) -> Result<TenantContext, TenantContextLoadError> {
    let context = load_tenant_context(ctx, &resolution.identifier).await?;
    resolution
        .validate_resolved_slug(&context.slug)
        .map_err(|error| TenantContextLoadError::InvalidAssertion(error.to_string()))?;
    Ok(context)
}

pub(crate) async fn resolve_tenant_context_by_slug(
    ctx: &ServerRuntimeContext,
    slug: &str,
) -> Result<TenantContext, TenantContextLoadError> {
    let resolution = resolve_explicit_slug(slug)
        .map_err(|error| TenantContextLoadError::InvalidIdentifier(error.to_string()))?;
    record_resolution_source(resolution.source);
    load_resolved_tenant_context(ctx, &resolution).await
}
'''
new_runtime = '''fn record_resolution_outcome(
    transport: TenantResolutionTransport,
    source: TenantResolutionSource,
    outcome: &str,
) {
    rustok_telemetry::metrics::record_tenant_resolution(
        transport.as_str(),
        source.as_str(),
        outcome,
    );
}

async fn load_resolved_tenant_context(
    ctx: &ServerRuntimeContext,
    resolution: &TenantResolution,
    transport: TenantResolutionTransport,
) -> Result<TenantContext, TenantContextLoadError> {
    let result = async {
        let context = load_tenant_context(ctx, &resolution.identifier).await?;
        resolution
            .validate_resolved_slug(&context.slug)
            .map_err(|error| TenantContextLoadError::InvalidAssertion(error.to_string()))?;
        Ok(context)
    }
    .await;

    let outcome = match &result {
        Ok(_) => "success",
        Err(error) => error.metric_outcome(),
    };
    record_resolution_outcome(transport, resolution.source, outcome);
    result
}

pub(crate) async fn resolve_tenant_context_by_slug(
    ctx: &ServerRuntimeContext,
    slug: &str,
) -> Result<TenantContext, TenantContextLoadError> {
    let resolution = resolve_explicit_slug(slug)
        .map_err(|error| TenantContextLoadError::InvalidIdentifier(error.to_string()))?;
    load_resolved_tenant_context(
        ctx,
        &resolution,
        TenantResolutionTransport::GraphqlWebSocket,
    )
    .await
}
'''
replace_once(runtime, old_runtime, new_runtime, "replace cache metric with tenant metric")
replace_once(
    runtime,
    '''    record_resolution_source(resolution.source);
    if resolution.source == TenantResolutionSource::DevelopmentFallback {
        rustok_telemetry::metrics::record_cache_operation(
            "tenant_resolution",
            "fallback",
            "default_tenant",
        );
        tracing::warn!(
''',
    '''    if resolution.source == TenantResolutionSource::DevelopmentFallback {
        tracing::warn!(
''',
    "remove fallback cache metric",
)
replace_once(
    runtime,
    '''    let context = load_resolved_tenant_context(&ctx, &resolution)
''',
    '''    let context = load_resolved_tenant_context(
        &ctx,
        &resolution,
        TenantResolutionTransport::Http,
    )
''',
    "http typed transport",
)

metrics_docs = Path("docs/guides/metrics.md")
source = metrics_docs.read_text()
append = '''

## Tenant resolution

`rustok_tenant_resolutions_total{transport,source,outcome}` records the final tenant-context resolution outcome. `transport` is bounded to server transports such as `http` and `graphql_ws`; `source` comes from the typed tenant resolution result; `outcome` is a bounded success or failure class. Tenant resolution must not be reported through `rustok_cache_operations_total`.
'''
if "rustok_tenant_resolutions_total" not in source:
    metrics_docs.write_text(source.rstrip() + append + "\n")
