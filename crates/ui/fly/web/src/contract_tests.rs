use super::*;

#[test]
fn test_package_metadata_contract() {
    assert_eq!(env!("CARGO_PKG_NAME"), "fly-web");
    assert!(!env!("CARGO_PKG_VERSION").is_empty());
    assert!(!env!("CARGO_PKG_DESCRIPTION").is_empty());
}

#[test]
fn test_geometric_primitives_contract() {
    let pt = BrowserPoint { x: 50.0, y: 75.0 };
    assert!(pt.is_finite());
    assert!((pt.x - 50.0).abs() < f64::EPSILON);
    assert!((pt.y - 75.0).abs() < f64::EPSILON);

    let rect = BrowserRect {
        left: 20.0,
        top: 30.0,
        width: 100.0,
        height: 120.0,
    };
    assert!(rect.is_finite());
    assert!(rect.contains(pt));

    let outside_pt = BrowserPoint { x: 10.0, y: 10.0 };
    assert!(!rect.contains(outside_pt));

    let nan_pt = BrowserPoint {
        x: f64::NAN,
        y: 0.0,
    };
    assert!(!nan_pt.is_finite());
}

#[test]
fn test_origin_sanitization_contract() {
    assert_eq!(FLY_IFRAME_PROTOCOL, "fly_iframe");
    assert_eq!(OPAQUE_ORIGIN, "null");

    let normalized = normalize_expected_origin("  https://rustok.dev  ");
    assert_eq!(normalized.as_deref(), Some("https://rustok.dev"));

    let wildcard = normalize_expected_origin("*");
    assert_eq!(wildcard, None);

    let empty = normalize_expected_origin("   ");
    assert_eq!(empty, None);

    // Outbound target origin for opaque origin must map to wildcard
    assert_eq!(outbound_target_origin(OPAQUE_ORIGIN), "*");
    assert_eq!(
        outbound_target_origin("https://rustok.dev"),
        "https://rustok.dev"
    );
}

#[test]
fn test_cleanup_registry_contract() {
    let mut cleanup = CleanupRegistry::default();
    assert!(cleanup.is_empty());

    cleanup.register(|| {});
    cleanup.register(|| {});
    assert_eq!(cleanup.len(), 2);
    assert!(!cleanup.is_empty());

    cleanup.cleanup();
    assert!(cleanup.is_empty());
}
