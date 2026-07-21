use fly_browser::FLY_BROWSER_ADAPTER_JS;

#[test]
fn public_bundle_orders_intent_responses_and_invalidates_on_stop() {
    assert!(FLY_BROWSER_ADAPTER_JS.contains("this.intentRequestGeneration = 0"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("this.latestIntentRequestGeneration = requestGeneration"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("requestGeneration === this.latestIntentRequestGeneration"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("if (current && response.ok && isObject(result))"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("requestGeneration, current"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("event.detail?.current === false"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("this.latestIntentRequestGeneration = ++this.intentRequestGeneration"));
}
