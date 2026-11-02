use super::*;

#[test]
fn test_package_metadata_contract() {
    assert_eq!(env!("CARGO_PKG_NAME"), "fly-leptos");
    assert!(!env!("CARGO_PKG_VERSION").is_empty());
    assert!(!env!("CARGO_PKG_DESCRIPTION").is_empty());
}

#[test]
fn test_reexported_web_geometry_contract() {
    let pt = BrowserPoint { x: 15.0, y: 25.0 };
    assert!(pt.is_finite());
    assert!((pt.x - 15.0).abs() < f64::EPSILON);
    assert!((pt.y - 25.0).abs() < f64::EPSILON);

    let rect = BrowserRect {
        left: 10.0,
        top: 20.0,
        width: 200.0,
        height: 100.0,
    };
    assert!(rect.is_finite());
    assert!((rect.left - 10.0).abs() < f64::EPSILON);
    assert!((rect.top - 20.0).abs() < f64::EPSILON);
    assert!((rect.width - 200.0).abs() < f64::EPSILON);
    assert!((rect.height - 100.0).abs() < f64::EPSILON);
    assert!(rect.contains(pt));
}

#[test]
fn test_iframe_protocol_constants_and_origin_contract() {
    assert_eq!(FLY_IFRAME_PROTOCOL, "fly_iframe");
    assert_eq!(OPAQUE_ORIGIN, "null");

    let normalized = normalize_expected_origin("  https://example.com  ");
    assert_eq!(normalized.as_deref(), Some("https://example.com"));

    let wildcard = normalize_expected_origin("*");
    assert_eq!(wildcard, None);

    let opaque_norm = normalize_expected_origin("null");
    assert_eq!(opaque_norm.as_deref(), Some("null"));
}

#[test]
fn test_cleanup_registry_contract() {
    let mut cleanup = CleanupRegistry::default();
    assert!(cleanup.is_empty());
    cleanup.register(|| {});
    assert!(!cleanup.is_empty());
    assert_eq!(cleanup.len(), 1);
    cleanup.cleanup();
    assert!(cleanup.is_empty());
}

