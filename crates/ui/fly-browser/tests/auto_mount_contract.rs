use fly_browser::{BrowserAdapterConfig, FLY_BROWSER_ADAPTER_JS};

#[test]
fn auto_mount_false_serializes_for_javascript() {
    let json = BrowserAdapterConfig {
        auto_mount: false,
        ..BrowserAdapterConfig::default()
    }
    .to_json()
    .expect("browser config");
    let value: serde_json::Value = serde_json::from_str(&json).expect("JSON");

    assert_eq!(value["autoMount"], false);
    assert!(value.get("auto_mount").is_none());
}

#[test]
fn public_bundle_separates_bootstrap_from_manual_mount() {
    assert!(FLY_BROWSER_ADAPTER_JS.contains("export function bootstrapFlyBrowsers"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("bootstrapConfig.autoMount !== false"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("bootstrap: bootstrapFlyBrowsers"));
    assert!(FLY_BROWSER_ADAPTER_JS.contains("mountAll: mountAllFlyBrowsers"));
}
