use loco_rs::cli;
use migration::Migrator;
use rustok_server::app::App;
use rustok_telemetry::{LogFormat, TelemetryConfig};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let telemetry_cfg = telemetry_config();
    let has_otel = telemetry_cfg.otel.is_some();
    let _telemetry = if has_otel {
        rustok_telemetry::init(telemetry_cfg)?
    } else {
        // Loco owns the global tracing subscriber for CLI commands.
        rustok_telemetry::init_metrics(telemetry_cfg.metrics)?
    };
    let args = std::env::args().collect::<Vec<_>>();
    if rustok_server::installer_cli::try_handle(&args).await? {
        if has_otel {
            rustok_telemetry::otel::shutdown().await;
        }
        return Ok(());
    }
    let result = cli::main::<App, Migrator>().await;
    if has_otel {
        rustok_telemetry::otel::shutdown().await;
    }
    Ok(result?)
}

fn telemetry_config() -> TelemetryConfig {
    let log_format = match std::env::var("RUSTOK_LOG_FORMAT").as_deref() {
        Ok("json") => LogFormat::Json,
        _ => LogFormat::Pretty,
    };
    let metrics = std::env::var("RUSTOK_METRICS")
        .map(|value| value != "0")
        .unwrap_or(true);

    // Check if OpenTelemetry is enabled
    let otel = if std::env::var("OTEL_ENABLED")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false)
    {
        Some(rustok_telemetry::otel::OtelConfig::from_env())
    } else {
        None
    };

    TelemetryConfig {
        service_name: "rustok-server".to_string(),
        log_format,
        metrics,
        otel,
    }
}
