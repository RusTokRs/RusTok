pub mod metrics;
pub mod otel;

use lazy_static::lazy_static;
use once_cell::sync::OnceCell;
use prometheus::{Encoder, IntGauge, Registry, TextEncoder};
use std::sync::Arc;
use tracing_subscriber::{
    fmt, layer::SubscriberExt, EnvFilter, Layer, Registry as TracingRegistry,
};

static METRICS_HANDLE: OnceCell<Arc<MetricsHandle>> = OnceCell::new();
static REGISTRY: OnceCell<Registry> = OnceCell::new();

#[derive(Clone, Debug)]
pub struct MetricsHandle {
    registry: Arc<Registry>,
}

impl Default for MetricsHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsHandle {
    pub fn new() -> Self {
        let registry = Registry::new();
        Self {
            registry: Arc::new(registry),
        }
    }

    pub fn render(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).ok();
        String::from_utf8(buffer).unwrap_or_else(|_| String::from("Failed to encode metrics"))
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }
}

#[derive(Debug, Clone, Copy)]
pub enum LogFormat {
    Json,
    Pretty,
}

#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    pub service_name: String,
    pub log_format: LogFormat,
    pub metrics: bool,
    pub otel: Option<otel::OtelConfig>,
}

#[derive(Clone)]
pub struct TelemetryHandles {
    pub metrics: Option<Arc<MetricsHandle>>,
}

impl std::fmt::Debug for TelemetryHandles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TelemetryHandles")
            .field("metrics", &self.metrics.is_some())
            .finish()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    #[error("failed to set global tracing subscriber")]
    SubscriberAlreadySet,
    #[error("prometheus registry error: {0}")]
    Prometheus(#[from] prometheus::Error),
}

use prometheus::{CounterVec, HistogramOpts, HistogramVec, Opts};

lazy_static! {
    pub static ref CONTENT_OPERATIONS_TOTAL: CounterVec = CounterVec::new(
        Opts::new(
            "rustok_content_operations_total",
            "Total content operations"
        ),
        &["operation", "kind", "status"]
    )
    .expect("Failed to create content_operations_total");
    pub static ref CONTENT_OPERATION_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "rustok_content_operation_duration_seconds",
            "Duration of content operations"
        ),
        &["operation", "kind"]
    )
    .expect("Failed to create content_operation_duration_seconds");
    pub static ref CONTENT_NODES_TOTAL: IntGauge = IntGauge::new(
        "rustok_content_nodes_total",
        "Total number of content nodes"
    )
    .expect("Failed to create content_nodes_total");
    pub static ref COMMERCE_OPERATIONS_TOTAL: CounterVec = CounterVec::new(
        Opts::new(
            "rustok_commerce_operations_total",
            "Total commerce operations"
        ),
        &["operation", "kind", "status"]
    )
    .expect("Failed to create commerce_operations_total");
    pub static ref COMMERCE_OPERATION_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "rustok_commerce_operation_duration_seconds",
            "Duration of commerce operations"
        ),
        &["operation", "kind"]
    )
    .expect("Failed to create commerce_operation_duration_seconds");
    pub static ref COMMERCE_PRODUCTS_TOTAL: IntGauge =
        IntGauge::new("rustok_commerce_products_total", "Total number of products")
            .expect("Failed to create commerce_products_total");
    pub static ref COMMERCE_ORDERS_TOTAL: IntGauge =
        IntGauge::new("rustok_commerce_orders_total", "Total number of orders")
            .expect("Failed to create commerce_orders_total");
    pub static ref HTTP_REQUESTS_TOTAL: CounterVec = CounterVec::new(
        Opts::new("rustok_http_requests_total", "Total HTTP requests"),
        &["method", "path", "status"]
    )
    .expect("Failed to create http_requests_total");
    pub static ref HTTP_REQUEST_DURATION_SECONDS: HistogramVec = HistogramVec::new(
        HistogramOpts::new(
            "rustok_http_request_duration_seconds",
            "HTTP request duration"
        ),
        &["method", "path"]
    )
    .expect("Failed to create http_request_duration_seconds");
}

pub fn init(config: TelemetryConfig) -> Result<TelemetryHandles, TelemetryError> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer: Box<dyn Layer<_> + Send + Sync> = match config.log_format {
        LogFormat::Json => fmt::layer()
            .with_span_events(fmt::format::FmtSpan::CLOSE)
            .json()
            .boxed(),
        LogFormat::Pretty => fmt::layer()
            .with_span_events(fmt::format::FmtSpan::CLOSE)
            .pretty()
            .boxed(),
    };

    // Initialize subscriber with or without OpenTelemetry
    if let Some(otel_config) = config.otel {
        // Try to initialize OpenTelemetry layer
        let subscriber = TracingRegistry::default().with(env_filter).with(fmt_layer);

        // Initialize OTel in a blocking context since we're in a sync function
        let otel_layer = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                match otel::init_otel_layer(otel_config).await {
                    Ok(layer) => Some(layer),
                    Err(e) => {
                        tracing::warn!(
                            "Failed to initialize OpenTelemetry: {}, continuing without it",
                            e
                        );
                        None
                    }
                }
            })
        });

        if let Some(otel_tracer) = otel_layer {
            let otel_layer = tracing_opentelemetry::layer().with_tracer(otel_tracer);
            let subscriber = subscriber.with(otel_layer);
            tracing::subscriber::set_global_default(subscriber)
                .map_err(|_| TelemetryError::SubscriberAlreadySet)?;
            tracing::info!("Telemetry initialized with OpenTelemetry support");
        } else {
            tracing::subscriber::set_global_default(subscriber)
                .map_err(|_| TelemetryError::SubscriberAlreadySet)?;
            tracing::info!("Telemetry initialized without OpenTelemetry");
        }
    } else {
        let subscriber = TracingRegistry::default().with(env_filter).with(fmt_layer);
        tracing::subscriber::set_global_default(subscriber)
            .map_err(|_| TelemetryError::SubscriberAlreadySet)?;
        tracing::info!("Telemetry initialized (OpenTelemetry disabled)");
    }

    let metrics_handle = init_metrics_handle(config.metrics)?;

    Ok(TelemetryHandles {
        metrics: metrics_handle,
    })
}

pub fn init_metrics(metrics: bool) -> Result<TelemetryHandles, TelemetryError> {
    let metrics_handle = init_metrics_handle(metrics)?;
    Ok(TelemetryHandles {
        metrics: metrics_handle,
    })
}

fn init_metrics_handle(metrics: bool) -> Result<Option<Arc<MetricsHandle>>, TelemetryError> {
    if !metrics {
        return Ok(None);
    }

    if let Some(handle) = METRICS_HANDLE.get() {
        return Ok(Some(handle.clone()));
    }

    let handle = Arc::new(MetricsHandle::new());
    let registry = handle.registry();

    // Register all metrics
    registry.register(Box::new(CONTENT_OPERATIONS_TOTAL.clone()))?;
    registry.register(Box::new(CONTENT_OPERATION_DURATION_SECONDS.clone()))?;
    registry.register(Box::new(CONTENT_NODES_TOTAL.clone()))?;
    registry.register(Box::new(COMMERCE_OPERATIONS_TOTAL.clone()))?;
    registry.register(Box::new(COMMERCE_OPERATION_DURATION_SECONDS.clone()))?;
    registry.register(Box::new(COMMERCE_PRODUCTS_TOTAL.clone()))?;
    registry.register(Box::new(COMMERCE_ORDERS_TOTAL.clone()))?;
    registry.register(Box::new(HTTP_REQUESTS_TOTAL.clone()))?;
    registry.register(Box::new(HTTP_REQUEST_DURATION_SECONDS.clone()))?;

    // Register all custom metrics
    metrics::register_all(registry)?;

    let _ = REGISTRY.set(registry.clone());
    let _ = METRICS_HANDLE.set(handle.clone());
    Ok(Some(handle))
}

pub fn metrics_handle() -> Option<Arc<MetricsHandle>> {
    METRICS_HANDLE.get().cloned()
}

pub fn render_metrics() -> Result<String, prometheus::Error> {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY
        .get()
        .ok_or(prometheus::Error::Msg(
            "Registry not initialized".to_string(),
        ))?
        .gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer)?;
    Ok(String::from_utf8(buffer).unwrap_or_else(|_| String::from("Failed to encode metrics")))
}

pub fn current_trace_id() -> Option<String> {
    let span = tracing::Span::current();
    span.id().map(|id| id.into_u64().to_string())
}

#[cfg(test)]
mod contract_tests;
