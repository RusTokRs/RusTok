use fly_browser::FLY_BROWSER_ADAPTER_JS;

#[test]
fn public_bundle_bounds_and_aborts_pending_intents() {
    assert!(FLY_BROWSER_ADAPTER_JS.contains("DEFAULT_MAX_PENDING_INTENT_REQUESTS"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("maxPendingIntentRequests"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("PENDING_INTENT_LIMIT"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("fly:browser-intent-rejected"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("new AbortController()"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("controller.abort()"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("signal: controller.signal"));
}
