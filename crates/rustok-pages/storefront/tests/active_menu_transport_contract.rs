const MODEL: &str = include_str!("../src/model.rs");
const GRAPHQL: &str = include_str!("../src/transport/graphql_adapter.rs");
const NATIVE: &str = include_str!("../src/transport/native_server_adapter.rs");

#[test]
fn storefront_resolves_current_channel_active_menus_without_menu_ids() {
    for marker in [
        "activeHeaderMenu: activeMenu(location: HEADER",
        "activeFooterMenu: activeMenu(location: FOOTER",
        "active_header_menu: Option<StorefrontMenu>",
        "active_footer_menu: Option<StorefrontMenu>",
    ] {
        assert!(
            GRAPHQL.contains(marker) || MODEL.contains(marker),
            "storefront GraphQL/model contract must contain `{marker}`"
        );
    }

    for marker in [
        "MenuBindingService::new",
        ".get_active(",
        "request_context.as_ref().and_then(|ctx| ctx.channel_id)",
        "MenuLocation::Header",
        "MenuLocation::Footer",
        "MENU_LOCALE_NOT_FOUND_ERROR_CODE",
    ] {
        assert!(
            NATIVE.contains(marker),
            "native storefront contract must contain `{marker}`"
        );
    }

    assert!(!GRAPHQL.contains("menuId"));
    assert!(!GRAPHQL.contains("menu_id"));
    assert!(!NATIVE.contains("menu::Column::Location"));
    assert!(!NATIVE.contains(".first()"));
}
