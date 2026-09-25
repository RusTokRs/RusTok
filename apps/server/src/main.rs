/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use rustok_telemetry::{LogFormat, TelemetryConfig};

fn main() -> eyre::Result<()> {
    let telemetry_cfg = telemetry_config();
    let has_otel = telemetry_cfg.otel.is_some();
    let _telemetry = if has_otel {
        rustok_telemetry::init(telemetry_cfg)?
    } else {
        rustok_telemetry::init_metrics(telemetry_cfg.metrics)?
    };
    let stack_size = std::env::var("RUSTOK_THREAD_STACK_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8 * 1024 * 1024);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(stack_size)
        .build()?;
    let result = runtime.block_on(rustok_server::host::run());
    if has_otel {
        runtime.block_on(rustok_telemetry::otel::shutdown());
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
