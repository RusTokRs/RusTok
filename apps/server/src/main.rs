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

const DEFAULT_STACK_SIZE: usize = 8 * 1024 * 1024;
const MIN_STACK_SIZE: usize = 2 * 1024 * 1024;
const MAX_STACK_SIZE: usize = 64 * 1024 * 1024;

fn main() -> eyre::Result<()> {
    let telemetry_cfg = telemetry_config();
    let has_otel = telemetry_cfg.otel.is_some();
    let _telemetry = rustok_telemetry::init(telemetry_cfg)?;
    let stack_size =
        parse_thread_stack_size(std::env::var("RUSTOK_THREAD_STACK_SIZE").ok().as_deref());
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(stack_size)
        .build()?;
    let result = runtime.block_on(rustok_server::host::run());
    if let Err(ref error) = result {
        tracing::error!(error = %error, "RusTok Axum host terminated with fatal error");
    }
    if has_otel {
        runtime.block_on(rustok_telemetry::otel::shutdown());
    }
    Ok(result?)
}

fn parse_thread_stack_size(raw: Option<&str>) -> usize {
    raw.and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|&size| (MIN_STACK_SIZE..=MAX_STACK_SIZE).contains(&size))
        .unwrap_or(DEFAULT_STACK_SIZE)
}

fn parse_log_format(raw: Option<&str>) -> LogFormat {
    match raw.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("json") => LogFormat::Json,
        _ => LogFormat::Pretty,
    }
}

fn parse_metrics_enabled(raw: Option<&str>) -> bool {
    !matches!(
        raw.map(|s| s.trim().to_ascii_lowercase()).as_deref(),
        Some("0" | "false" | "no" | "off")
    )
}

fn resolve_service_name(otel_name: Option<String>, rustok_name: Option<String>) -> String {
    otel_name
        .or(rustok_name)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "rustok-server".to_string())
}

fn telemetry_config() -> TelemetryConfig {
    let log_format = parse_log_format(std::env::var("RUSTOK_LOG_FORMAT").ok().as_deref());
    let metrics = parse_metrics_enabled(std::env::var("RUSTOK_METRICS").ok().as_deref());
    let service_name = resolve_service_name(
        std::env::var("OTEL_SERVICE_NAME").ok(),
        std::env::var("RUSTOK_SERVICE_NAME").ok(),
    );

    // Check if OpenTelemetry is enabled
    let otel = if std::env::var("OTEL_ENABLED")
        .map(|v| {
            let s = v.trim().to_ascii_lowercase();
            s == "true" || s == "1" || s == "yes" || s == "on"
        })
        .unwrap_or(false)
    {
        Some(rustok_telemetry::otel::OtelConfig::from_env())
    } else {
        None
    };

    TelemetryConfig {
        service_name,
        log_format,
        metrics,
        otel,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_stack_size_defaults_to_8mb() {
        assert_eq!(parse_thread_stack_size(None), DEFAULT_STACK_SIZE);
        assert_eq!(parse_thread_stack_size(Some("")), DEFAULT_STACK_SIZE);
        assert_eq!(parse_thread_stack_size(Some("invalid")), DEFAULT_STACK_SIZE);
    }

    #[test]
    fn thread_stack_size_enforces_bounds() {
        // Less than 2MB falls back to default
        assert_eq!(parse_thread_stack_size(Some("1048576")), DEFAULT_STACK_SIZE);
        // Between 2MB and 64MB is accepted
        assert_eq!(parse_thread_stack_size(Some("4194304")), 4 * 1024 * 1024);
        assert_eq!(
            parse_thread_stack_size(Some("  16777216  ")),
            16 * 1024 * 1024
        );
        // Exceeding 64MB ceiling falls back to default
        assert_eq!(
            parse_thread_stack_size(Some("134217728")),
            DEFAULT_STACK_SIZE
        );
    }

    #[test]
    fn log_format_parsing_normalizes_input() {
        assert!(matches!(parse_log_format(Some("json")), LogFormat::Json));
        assert!(matches!(
            parse_log_format(Some("  JSON  ")),
            LogFormat::Json
        ));
        assert!(matches!(
            parse_log_format(Some("pretty")),
            LogFormat::Pretty
        ));
        assert!(matches!(parse_log_format(None), LogFormat::Pretty));
        assert!(matches!(parse_log_format(Some("other")), LogFormat::Pretty));
    }

    #[test]
    fn metrics_enabled_parsing_normalizes_falsy_values() {
        assert!(!parse_metrics_enabled(Some("0")));
        assert!(!parse_metrics_enabled(Some("false")));
        assert!(!parse_metrics_enabled(Some("FALSE")));
        assert!(!parse_metrics_enabled(Some("no")));
        assert!(!parse_metrics_enabled(Some("off")));

        assert!(parse_metrics_enabled(Some("1")));
        assert!(parse_metrics_enabled(Some("true")));
        assert!(parse_metrics_enabled(None));
    }

    #[test]
    fn service_name_precedence_and_trimming() {
        assert_eq!(
            resolve_service_name(
                Some("  otel-svc  ".to_string()),
                Some("rustok-svc".to_string())
            ),
            "otel-svc"
        );
        assert_eq!(
            resolve_service_name(None, Some("  rustok-svc  ".to_string())),
            "rustok-svc"
        );
        assert_eq!(
            resolve_service_name(Some("   ".to_string()), None),
            "rustok-server"
        );
        assert_eq!(resolve_service_name(None, None), "rustok-server");
    }
}
