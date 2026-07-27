const APP: &str = include_str!("../src/app/mod.rs");
const BUILD: &str = include_str!("../build.rs");
const REGISTRY: &str = include_str!("../src/modules/registry.rs");
const HEADER: &str = include_str!("../src/widgets/header/mod.rs");
const FOOTER: &str = include_str!("../src/widgets/footer/mod.rs");

#[test]
fn storefront_host_places_generic_header_and_footer_contributions() {
    for marker in [
        "HeaderNavigation",
        "HeaderActions",
        "FooterNavigation",
        "components_for_slot(StorefrontSlot::HeaderNavigation",
        "components_for_slot(StorefrontSlot::HeaderActions",
        "components_for_slot(StorefrontSlot::FooterNavigation",
    ] {
        assert!(
            REGISTRY.contains(marker) || APP.contains(marker),
            "storefront host slot contract must contain `{marker}`"
        );
    }

    for marker in [
        "components: Vec<StorefrontUiComponentContract>",
        "storefront_component_render_fn_name",
        "header_navigation",
        "header_actions",
        "footer_navigation",
    ] {
        assert!(
            BUILD.contains(marker),
            "storefront codegen must contain `{marker}`"
        );
    }

    assert!(HEADER.contains("navigation_views: Vec<AnyView>"));
    assert!(HEADER.contains("action_views: Vec<AnyView>"));
    assert!(HEADER.contains("data-storefront-header-actions"));
    assert!(FOOTER.contains("navigation_views: Vec<AnyView>"));
    assert!(!APP.contains("rustok_pages_storefront::PagesHeaderMenu"));
    assert!(!APP.contains("rustok_pages_storefront::PagesFooterMenu"));
    assert!(!APP.contains("rustok_notifications_storefront::NotificationNavigation"));
}
