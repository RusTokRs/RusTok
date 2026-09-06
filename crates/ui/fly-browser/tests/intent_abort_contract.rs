use fly_browser::FLY_BROWSER_ADAPTER_JS;

#[test]
fn bundled_adapter_uses_request_scoped_abort_signals() {
    assert!(FLY_BROWSER_ADAPTER_JS.contains("async postIntent(input, requestOptions = {})"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("signal: requestOptions?.signal"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("signal: controller.signal"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("forwardAbortSignal"));
    assert!(!FLY_BROWSER_ADAPTER_JS.contains("globalThis.fetch ="));
}
