use fly_browser::FLY_BROWSER_ADAPTER_JS;

#[test]
fn adapter_lifecycle_is_one_shot_and_idempotent() {
    for marker in [
        "const ADAPTER_LIFECYCLE = Object.freeze",
        "const ADAPTER_STOPPED_CODE = \"ADAPTER_STOPPED\"",
        "this.lifecycleState = ADAPTER_LIFECYCLE.CREATED",
        "if (this.lifecycleState === ADAPTER_LIFECYCLE.STARTED) return this",
        "if (this.lifecycleState === ADAPTER_LIFECYCLE.STOPPED) return this",
        "FlyBrowserLifecycleError",
    ] {
        assert!(FLY_BROWSER_ADAPTER_JS.contains(marker), "missing {marker}");
    }
}

#[test]
fn transport_options_expose_only_abort_signal() {
    assert!(FLY_BROWSER_ADAPTER_JS.contains("IntentTransportOptions"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("signal?: AbortSignal"));
    assert!(!FLY_BROWSER_ADAPTER_JS.contains("abort?: IntentAbortMetadata"));
    assert!(!FLY_BROWSER_ADAPTER_JS.contains("transport.abort"));
}
