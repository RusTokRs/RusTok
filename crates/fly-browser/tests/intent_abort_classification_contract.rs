use fly_browser::FLY_BROWSER_ADAPTER_JS;

#[test]
fn bundled_adapter_types_transport_options_and_abort_classification() {
    assert!(FLY_BROWSER_ADAPTER_JS.contains("IntentTransportOptions"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("normalizedTransportOptions"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("fly:browser-intent-aborted"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("INTENT_REQUEST_ABORTED"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("adapter_stop"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("NETWORK_ERROR"));
}
