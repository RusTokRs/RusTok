mod support;
use support::contains;

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
        assert!(contains(marker), "missing {marker}");
    }
}

#[test]
fn transport_options_expose_only_abort_signal() {
    assert!(contains("IntentTransportOptions"));
    assert!(contains("signal?: AbortSignal"));
    assert!(!contains("abort?: IntentAbortMetadata"));
    assert!(!contains("transport.abort"));
}
