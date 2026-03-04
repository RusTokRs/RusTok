/// OpenTelemetry Integration Tests
///
/// These tests verify OpenTelemetry initialization and configuration.
/// Note: Full tracing tests require a running OTLP collector (Jaeger/Tempo).
use rustok_telemetry::otel::{init_tracing, OtelConfig};
use std::sync::{Mutex, MutexGuard, OnceLock};

fn env_lock() -> MutexGuard<'static, ()> {
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    ENV_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("env lock poisoned")
}

#[test]
fn test_otel_config_builder() {
    let config = OtelConfig {
        service_name: "test-service".to_string(),
        service_version: "1.0.0".to_string(),
        otlp_endpoint: "http://localhost:4318".to_string(),
        environment: "test".to_string(),
        sampling_rate: 0.5,
        enabled: true,
    };

    assert_eq!(config.service_name, "test-service");
    assert_eq!(config.service_version, "1.0.0");
    assert_eq!(config.otlp_endpoint, "http://localhost:4318");
    assert_eq!(config.environment, "test");
    assert_eq!(config.sampling_rate, 0.5);
    assert!(config.enabled);
}

#[tokio::test]
async fn test_init_tracing_disabled() {
    let config = OtelConfig {
        enabled: false,
        ..Default::default()
    };

    let result = init_tracing(config).await;
    assert!(result.is_ok());
}

#[test]
fn test_config_from_env_defaults() {
    let _guard = env_lock();

    // Clear any existing env vars
    std::env::remove_var("OTEL_SERVICE_NAME");
    std::env::remove_var("OTEL_EXPORTER_OTLP_ENDPOINT");
    std::env::remove_var("OTEL_SAMPLING_RATE");
    std::env::remove_var("RUST_ENV");
    std::env::remove_var("OTEL_ENABLED");

    let config = OtelConfig::from_env();

    // Should use defaults
    assert_eq!(config.service_name, "rustok-server");
    assert_eq!(config.otlp_endpoint, "http://localhost:4317");
    assert_eq!(config.sampling_rate, 1.0);
    assert!(config.enabled);
}

#[test]
fn test_config_from_env_custom() {
    let _guard = env_lock();

    std::env::set_var("OTEL_SERVICE_NAME", "custom-service");
    std::env::set_var("OTEL_SERVICE_VERSION", "2.0.0");
    std::env::set_var("OTEL_EXPORTER_OTLP_ENDPOINT", "http://custom:4317");
    std::env::set_var("RUST_ENV", "production");
    std::env::set_var("OTEL_SAMPLING_RATE", "0.1");
    std::env::set_var("OTEL_ENABLED", "true");

    let config = OtelConfig::from_env();

    assert_eq!(config.service_name, "custom-service");
    assert_eq!(config.service_version, "2.0.0");
    assert_eq!(config.otlp_endpoint, "http://custom:4317");
    assert_eq!(config.environment, "production");
    assert_eq!(config.sampling_rate, 0.1);
    assert!(config.enabled);

    // Cleanup
    std::env::remove_var("OTEL_SERVICE_NAME");
    std::env::remove_var("OTEL_SERVICE_VERSION");
    std::env::remove_var("OTEL_EXPORTER_OTLP_ENDPOINT");
    std::env::remove_var("RUST_ENV");
    std::env::remove_var("OTEL_SAMPLING_RATE");
    std::env::remove_var("OTEL_ENABLED");
}

#[test]
fn test_config_disabled_via_env() {
    let _guard = env_lock();

    std::env::set_var("OTEL_ENABLED", "false");
    let config = OtelConfig::from_env();
    assert!(!config.enabled);
    std::env::remove_var("OTEL_ENABLED");

    std::env::set_var("OTEL_ENABLED", "0");
    let config = OtelConfig::from_env();
    assert!(!config.enabled);
    std::env::remove_var("OTEL_ENABLED");
}

#[test]
fn test_sampling_rate_bounds() {
    let config = OtelConfig {
        sampling_rate: 0.0,
        ..Default::default()
    };
    assert_eq!(config.sampling_rate, 0.0);

    let config = OtelConfig {
        sampling_rate: 1.0,
        ..Default::default()
    };
    assert_eq!(config.sampling_rate, 1.0);
}

// Integration test with actual OTLP collector
// This requires Jaeger running on localhost:4317
#[tokio::test]
#[ignore] // Ignored by default - run with `cargo test -- --ignored`
async fn test_init_tracing_with_collector() {
    let config = OtelConfig {
        service_name: "rustok-test".to_string(),
        service_version: "0.0.1".to_string(),
        otlp_endpoint: "http://localhost:4317".to_string(),
        environment: "test".to_string(),
        sampling_rate: 1.0,
        enabled: true,
    };

    let result = init_tracing(config).await;

    // This will fail if Jaeger is not running, but that's expected
    match result {
        Ok(_) => {
            // Success - collector is running
            tracing::info!("Successfully connected to OTLP collector");

            // Create a test span
            let span = tracing::info_span!("test_span");
            let _guard = span.enter();
            tracing::info!("Test log inside span");

            // Shutdown gracefully
            rustok_telemetry::otel::shutdown().await;
        }
        Err(e) => {
            // Expected if collector is not running
            eprintln!("Note: OTLP collector not available (expected): {}", e);
        }
    }
}
