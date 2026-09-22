use rustok_telemetry::{LogFormat, TelemetryConfig};

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[test]
#[ignore = "Integration test requires global telemetry setup"]
fn test_telemetry_init() -> TestResult<()> {
    let config = TelemetryConfig {
        service_name: "rustok-test".to_string(),
        log_format: LogFormat::Json,
        metrics: false,
        otel: None,
    };

    let _handles = rustok_telemetry::init(config)?;
    Ok(())
}
