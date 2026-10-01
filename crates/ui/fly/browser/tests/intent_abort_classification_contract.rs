use fly_browser::FLY_BROWSER_ADAPTER_JS;

#[test]
fn bundled_adapter_owns_typed_abort_classification() {
    assert!(FLY_BROWSER_ADAPTER_JS.contains("IntentTransportOptions"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("IntentAbortMetadata"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("normalizedTransportOptions"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("signal?: AbortSignal"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("newAbortMetadata"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("fly:browser-intent-aborted"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("INTENT_REQUEST_ABORTED"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("adapter_stop"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("NETWORK_ERROR"));
    assert!(!FLY_BROWSER_ADAPTER_JS.contains("abort?: IntentAbortMetadata"));
    assert!(!FLY_BROWSER_ADAPTER_JS.contains("transport.abort"));
    assert!(!FLY_BROWSER_ADAPTER_JS.contains("pendingIntentRecordForGeneration"));
    assert!(!FLY_BROWSER_ADAPTER_JS.contains("reportIntentAborted"));
}
