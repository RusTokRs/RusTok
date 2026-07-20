use fly_browser::FLY_BROWSER_ADAPTER_JS;

#[test]
fn public_bundle_reports_typed_accessible_browser_problems() {
    assert!(FLY_BROWSER_ADAPTER_JS.contains("fly:browser-problem"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("flyBrowserProblem"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("NETWORK_ERROR"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("role\", \"alert"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("aria-live\", \"assertive"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("normalizedProblem"));
}
