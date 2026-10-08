mod support;
use fly_browser::BrowserAdapterConfig;
use support::contains;

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
    assert!(contains("export function bootstrapFlyBrowsers"));
    assert!(contains("bootstrapConfig.autoMount !== false"));
    assert!(contains("bootstrap: bootstrapFlyBrowsers"));
    assert!(contains("mountAll: mountAllFlyBrowsers"));
}
