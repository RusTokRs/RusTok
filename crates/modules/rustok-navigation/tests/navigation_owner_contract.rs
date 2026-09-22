const NAV: &str = include_str!("../src/lib.rs");
const SERVICE: &str = include_str!("../src/services/menu_binding.rs");
const PAGES: &str = include_str!("../../rustok-pages/src/lib.rs");
#[test]
fn navigation_is_the_only_current_menu_owner() {
    for marker in [
        "NavigationModule",
        "Resource::Navigation",
        "MenuBindingService",
        "MenuService",
    ] {
        assert!(
            NAV.contains(marker) || SERVICE.contains(marker),
            "missing `{marker}`"
        );
    }
    assert!(!PAGES.contains("MenuBindingService"));
    assert!(!PAGES.contains("MenuService"));
    assert!(!SERVICE.contains("rustok_pages"));
}
