const MODEL: &str = include_str!("../src/model.rs");
const GRAPHQL: &str = include_str!("../src/transport/graphql_adapter.rs");
const NATIVE: &str = include_str!("../src/transport/native_server_adapter.rs");
const TRANSPORT: &str = include_str!("../src/transport/mod.rs");
const MENU_UI: &str = include_str!("../src/ui/menu.rs");
const MANIFEST: &str = include_str!("../../rustok-module.toml");

#[test]
fn active_menus_use_dedicated_current_channel_layout_transport() {
    for marker in [
        "query StorefrontActiveMenu",
        "activeMenu(location: $location",
        "StorefrontActiveMenuVariables",
        "pub async fn fetch_active_menu(",
    ] {
        assert!(
            GRAPHQL.contains(marker) || TRANSPORT.contains(marker),
            "dedicated active-menu transport must contain `{marker}`"
        );
    }

    for marker in [
        "endpoint = "pages/active-menu"",
        "async fn active_menu_native(",
        "MenuBindingService::new",
        ".get_active(",
        "request_context.as_ref().and_then(|ctx| ctx.channel_id)",
        "MENU_LOCALE_NOT_FOUND_ERROR_CODE",
    ] {
        assert!(
            NATIVE.contains(marker),
            "native active-menu transport must contain `{marker}`"
        );
    }

    for marker in [
        "pub fn PagesHeaderMenu()",
        "pub fn PagesFooterMenu()",
        "StorefrontMenuLocation::Header",
        "StorefrontMenuLocation::Footer",
    ] {
        assert!(MENU_UI.contains(marker), "menu UI must contain `{marker}`");
    }

    for marker in [
        "component = "PagesHeaderMenu"",
        "slot = "header_navigation"",
        "component = "PagesFooterMenu"",
        "slot = "footer_navigation"",
    ] {
        assert!(MANIFEST.contains(marker), "manifest must contain `{marker}`");
    }

    assert!(!GRAPHQL.contains("activeHeaderMenu: activeMenu"));
    assert!(!GRAPHQL.contains("activeFooterMenu: activeMenu"));
    assert!(!MODEL.contains("active_header_menu:"));
    assert!(!MODEL.contains("active_footer_menu:"));
    assert!(!NATIVE.contains("menu::Column::Location"));
    assert!(!NATIVE.contains(".first()"));
}
