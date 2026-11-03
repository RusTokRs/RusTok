#[test]
fn crate_api_defines_public_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
        "## Public modules",
        "## Primary public types and functions",
        "## Contract invariants",
        "## Errors",
    ] {
        assert!(
            api.contains(marker),
            "CRATE_API.md must contain section: {marker}"
        );
    }
}

#[test]
fn metrics_handle_render_and_default() {
    let handle = crate::MetricsHandle::default();
    let rendered = handle.render();
    assert!(rendered.is_empty() || !rendered.contains("Failed to encode metrics"));

    let debug = format!("{handle:?}");
    assert!(debug.contains("MetricsHandle"));
}

#[test]
fn log_format_traits_and_telemetry_handles_debug() {
    assert_eq!(crate::LogFormat::Json, crate::LogFormat::Json);
    assert_ne!(crate::LogFormat::Json, crate::LogFormat::Pretty);

    let handles = crate::TelemetryHandles { metrics: None };
    assert_eq!(
        format!("{handles:?}"),
        "TelemetryHandles { metrics: false }"
    );
}
