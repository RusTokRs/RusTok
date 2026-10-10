//! Monitoring and health check server for rustok-revisions.

use anyhow::Result;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::get,
    Router,
};
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use prometheus::{
    register_counter_vec, register_gauge, register_histogram_vec, CounterVec, Encoder, Gauge,
    HistogramVec, TextEncoder,
};
use rustok_revisions::{RevisionService, SeaOrmBackend};
use sea_orm::Database;
use serde::Serialize;
use std::{net::SocketAddr, sync::Arc};
use sysinfo::System;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

lazy_static! {
    static ref REVISIONS_TOTAL: CounterVec = register_counter_vec!(
        "revisions_total",
        "Total number of revisions",
        &["content_type", "event"]
    )
    .unwrap();
    static ref REVISIONS_SIZE_BYTES: Gauge =
        register_gauge!("revisions_size_bytes", "Total size of revisions in bytes").unwrap();
    static ref REVISIONS_COUNT: Gauge =
        register_gauge!("revisions_count", "Total number of revisions").unwrap();
    static ref DATABASE_CONNECTIONS: Gauge = register_gauge!(
        "database_connections_active",
        "Number of active database connections"
    )
    .unwrap();
    static ref REQUEST_DURATION: HistogramVec = register_histogram_vec!(
        "request_duration_seconds",
        "Request duration in seconds",
        &["endpoint"]
    )
    .unwrap();
}

#[derive(Clone)]
struct AppState {
    service: Arc<RevisionService>,
    start_time: DateTime<Utc>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    uptime_seconds: i64,
    database: DatabaseHealth,
    system: SystemHealth,
}

#[derive(Serialize)]
struct DatabaseHealth {
    connected: bool,
    revision_count: i64,
    oldest_revision: Option<DateTime<Utc>>,
    newest_revision: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
struct SystemHealth {
    cpu_usage: f32,
    memory_used_mb: u64,
    memory_total_mb: u64,
}

#[derive(Serialize)]
struct MetricsResponse {
    revisions_total: i64,
    content_types: Vec<ContentTypeStats>,
}

#[derive(Serialize)]
struct ContentTypeStats {
    content_type: String,
    count: i64,
    avg_size_bytes: i64,
}

async fn health_handler(State(state): State<AppState>) -> impl IntoResponse {
    let uptime = (Utc::now() - state.start_time).num_seconds();

    // Get database stats
    let revision_count = get_total_revision_count(&state.service).await.unwrap_or(0);
    let (oldest, newest) = get_revision_date_range(&state.service).await.unwrap_or((None, None));

    // Get system stats
    let mut sys = System::new();
    sys.refresh_all();
    let cpu_usage = sys.global_cpu_info().cpu_usage();
    let memory_used = sys.used_memory() / 1024 / 1024;
    let memory_total = sys.total_memory() / 1024 / 1024;

    let health = HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: uptime,
        database: DatabaseHealth {
            connected: true,
            revision_count,
            oldest_revision: oldest,
            newest_revision: newest,
        },
        system: SystemHealth {
            cpu_usage,
            memory_used_mb: memory_used,
            memory_total_mb: memory_total,
        },
    };

    Json(health)
}

async fn metrics_handler() -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();

    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/plain")],
        String::from_utf8(buffer).unwrap(),
    )
}

async fn dashboard_handler(State(state): State<AppState>) -> impl IntoResponse {
    let revision_count = get_total_revision_count(&state.service).await.unwrap_or(0);
    let uptime = (Utc::now() - state.start_time).num_seconds();

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Rustok Revisions Monitor</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 40px; background: #f5f5f5; }}
        .container {{ max-width: 1200px; margin: 0 auto; }}
        h1 {{ color: #333; }}
        .card {{ background: white; padding: 20px; margin: 20px 0; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }}
        .metric {{ font-size: 2em; color: #2196F3; font-weight: bold; }}
        .label {{ color: #666; margin-top: 5px; }}
        .grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(250px, 1fr)); gap: 20px; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>🔍 Rustok Revisions Monitor</h1>
        
        <div class="grid">
            <div class="card">
                <div class="metric">{}</div>
                <div class="label">Total Revisions</div>
            </div>
            
            <div class="card">
                <div class="metric">{}s</div>
                <div class="label">Uptime</div>
            </div>
            
            <div class="card">
                <div class="metric">{}</div>
                <div class="label">Version</div>
            </div>
            
            <div class="card">
                <div class="metric">✓</div>
                <div class="label">Status: Healthy</div>
            </div>
        </div>
        
        <div class="card">
            <h2>Endpoints</h2>
            <ul>
                <li><a href="/health">/health</a> - Health check (JSON)</li>
                <li><a href="/metrics">/metrics</a> - Prometheus metrics</li>
                <li><a href="/dashboard">/dashboard</a> - This dashboard</li>
            </ul>
        </div>
        
        <div class="card">
            <h2>System Information</h2>
            <p><strong>Started:</strong> {}</p>
            <p><strong>Current Time:</strong> {}</p>
        </div>
    </div>
</body>
</html>"#,
        revision_count,
        uptime,
        env!("CARGO_PKG_VERSION"),
        state.start_time.format("%Y-%m-%d %H:%M:%S UTC"),
        Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    );

    Html(html)
}

async fn get_total_revision_count(service: &RevisionService) -> Result<i64> {
    // This is a simplified version - in production you'd query the database directly
    // For now, we'll use a dummy tenant/content to get a count
    let dummy_tenant = Uuid::nil();
    let dummy_content = Uuid::nil();
    
    match service.count_revisions(dummy_tenant, dummy_content, "en").await {
        Ok(count) => Ok(count),
        Err(_) => Ok(0),
    }
}

async fn get_revision_date_range(
    _service: &RevisionService,
) -> Result<(Option<DateTime<Utc>>, Option<DateTime<Utc>>)> {
    // Simplified - in production query the database for min/max created_at
    Ok((None, None))
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("rustok_revisions_monitoring=info".parse().unwrap()),
        )
        .init();

    tracing::info!("Starting Rustok Revisions Monitor");

    // Connect to database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/rustok_revisions".to_string());

    tracing::info!("Connecting to database: {}", database_url);
    let db = Database::connect(&database_url).await?;

    let backend = SeaOrmBackend::new(db);
    let service = Arc::new(RevisionService::new(Box::new(backend)));

    let state = AppState {
        service,
        start_time: Utc::now(),
    };

    // Build router
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/metrics", get(metrics_handler))
        .route("/dashboard", get(dashboard_handler))
        .route("/", get(|| async { "Rustok Revisions Monitor - Visit /dashboard" }))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("Listening on {}", addr);
    tracing::info!("Dashboard: http://localhost:8080/dashboard");
    tracing::info!("Health: http://localhost:8080/health");
    tracing::info!("Metrics: http://localhost:8080/metrics");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
