const MODEL: &str = include_str!("../src/model.rs");
const TRANSPORT: &str = include_str!("../src/transport/mod.rs");
const NATIVE: &str = include_str!("../src/transport/native_server_adapter.rs");
const UI: &str = include_str!("../src/ui/menu.rs");
#[test]
fn navigation_owns_active_menu_transport_and_slot_views() {
    for marker in [
        "StorefrontMenuLocation",
        "fetch_active_menu",
        "navigation/active-menu",
        "MenuBindingService::new",
        "NavigationHeaderMenu",
        "NavigationView",
    ] {
        assert!(
            MODEL.contains(marker)
                || TRANSPORT.contains(marker)
                || NATIVE.contains(marker)
                || UI.contains(marker),
            "missing `{marker}`"
        );
    }
    assert!(!NATIVE.contains("rustok_pages"));
    assert!(!MODEL.contains("page_id"));
}
