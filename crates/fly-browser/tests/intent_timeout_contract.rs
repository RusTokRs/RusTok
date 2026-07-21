use fly_browser::FLY_BROWSER_ADAPTER_JS;

#[test]
fn public_bundle_bounds_hung_intent_requests() {
    assert!(FLY_BROWSER_ADAPTER_JS.contains("DEFAULT_INTENT_REQUEST_TIMEOUT_MS"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("intentRequestTimeoutMs"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("flyIntentRequestTimeoutMs"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("INTENT_REQUEST_TIMEOUT"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("fly:browser-intent-timeout"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("clearTimeout"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("controller.abort()"));
}
