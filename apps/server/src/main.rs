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
    let args: Vec<String> = std::env::args().skip(1).collect();
    if is_validate_jwt_keyring_command(&args) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        runtime.block_on(rustok_server::host::validate_jwt_keyring_bootstrap())?;
        println!("JWT keyring document validated");
        return Ok(());
    }
    if !args.is_empty() {
        return Err(eyre::eyre!("Unknown rustok-server command"));
    }

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
    if result.is_err() {
        tracing::error!(
            failure_code = "server_host_terminated",
            "RusTok Axum host terminated with fatal error"
        );
    }
    if has_otel {
        runtime.block_on(rustok_telemetry::otel::shutdown());
    }
    result.map_err(Into::into)
}

fn is_validate_jwt_keyring_command(args: &[String]) -> bool {
    args.len() == 1 && args[0] == "validate-jwt-keyring"
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
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .or_else(|| {
            rustok_name
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty())
        })
        .unwrap_or("rustok-server")
        .to_string()
}

fn parse_otel_enabled(raw: Option<&str>) -> bool {
    matches!(
        raw.map(|s| s.trim().to_ascii_lowercase()).as_deref(),
        Some("true" | "1" | "yes" | "on")
    )
}

fn resolve_otel_config(
    otel_enabled_raw: Option<&str>,
    service_name: &str,
    build_base: impl FnOnce() -> rustok_telemetry::otel::OtelConfig,
) -> Option<rustok_telemetry::otel::OtelConfig> {
    if !parse_otel_enabled(otel_enabled_raw) {
        return None;
    }

    let mut config = build_base();
    config.enabled = true;
    config.service_name = service_name.to_string();
    Some(config)
}

fn telemetry_config() -> TelemetryConfig {
    let log_format = parse_log_format(std::env::var("RUSTOK_LOG_FORMAT").ok().as_deref());
    let metrics = parse_metrics_enabled(std::env::var("RUSTOK_METRICS").ok().as_deref());
    let service_name = resolve_service_name(
        std::env::var("OTEL_SERVICE_NAME").ok(),
        std::env::var("RUSTOK_SERVICE_NAME").ok(),
    );
    let otel_enabled_raw = std::env::var("OTEL_ENABLED").ok();
    let otel = resolve_otel_config(
        otel_enabled_raw.as_deref(),
        &service_name,
        rustok_telemetry::otel::OtelConfig::from_env,
    );

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
    fn keyring_validation_command_requires_exact_invocation() {
        assert!(is_validate_jwt_keyring_command(&["validate-jwt-keyring".to_string()]));
        assert!(!is_validate_jwt_keyring_command(&[]));
        assert!(!is_validate_jwt_keyring_command(&[
            "validate-jwt-keyring".to_string(), "extra".to_string(),
        ]));
        assert!(!is_validate_jwt_keyring_command(&["validate".to_string()]));
    }

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
        // Exact lower and upper inclusive bounds are accepted
        assert_eq!(parse_thread_stack_size(Some("2097152")), MIN_STACK_SIZE);
        assert_eq!(parse_thread_stack_size(Some("67108864")), MAX_STACK_SIZE);
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
        assert_eq!(
            resolve_service_name(
                Some("   ".to_string()),
                Some("  valid-fallback  ".to_string())
            ),
            "valid-fallback"
        );
        assert_eq!(
            resolve_service_name(Some("   ".to_string()), Some("   ".to_string())),
            "rustok-server"
        );
        assert_eq!(resolve_service_name(None, None), "rustok-server");
    }

    #[test]
    fn otel_enabled_parsing_normalizes_truthy_values() {
        assert!(parse_otel_enabled(Some("true")));
        assert!(parse_otel_enabled(Some("  TRUE  ")));
        assert!(parse_otel_enabled(Some("1")));
        assert!(parse_otel_enabled(Some("yes")));
        assert!(parse_otel_enabled(Some("on")));

        assert!(!parse_otel_enabled(None));
        assert!(!parse_otel_enabled(Some("")));
        assert!(!parse_otel_enabled(Some("false")));
        assert!(!parse_otel_enabled(Some("0")));
        assert!(!parse_otel_enabled(Some("no")));
    }

    #[test]
    fn resolve_otel_config_propagates_enabled_and_resolved_service_name() {
        assert!(resolve_otel_config(None, "custom-svc", || {
            panic!("base builder must not run when OTEL is disabled");
        })
        .is_none());

        let resolved = resolve_otel_config(Some("yes"), "custom-svc", || {
            rustok_telemetry::otel::OtelConfig {
                service_name: "stale-default".to_string(),
                enabled: false,
                ..Default::default()
            }
        })
        .expect("OTEL should be enabled for 'yes'");

        assert!(resolved.enabled);
        assert_eq!(resolved.service_name, "custom-svc");
    }
}
