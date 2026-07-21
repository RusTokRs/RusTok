const GRAPHQL_TYPES: &str = include_str!("../src/graphql/types.rs");
const GRAPHQL_QUERY: &str = include_str!("../src/graphql/query.rs");
const GRAPHQL_MUTATION: &str = include_str!("../src/graphql/mutation.rs");
const HTTP: &str = include_str!("../src/controllers/mod.rs");
const OPENAPI: &str = include_str!("../src/openapi.rs");

#[test]
fn menu_graphql_transport_uses_exact_effective_locale() {
    let query_runtime = GRAPHQL_QUERY
        .split("#[cfg(test)]")
        .next()
        .expect("query runtime source should exist");

    for marker in [
        "async fn menu(",
        "resolve_graphql_locale(ctx, locale.as_deref())",
        "MenuService::new",
        "MENU_LOCALE_NOT_FOUND_ERROR_CODE",
        "MENU_TRANSLATION_INTEGRITY_ERROR_CODE",
    ] {
        assert!(
            query_runtime.contains(marker),
            "GraphQL menu query must contain `{marker}`"
        );
    }

    assert!(!query_runtime.contains("PLATFORM_FALLBACK_LOCALE"));
    assert!(!query_runtime.contains(".first()"));
    assert!(!query_runtime.contains("menu_by_location"));
}

#[test]
fn menu_graphql_write_contract_has_translation_sets_only() {
    for marker in [
        "pub struct CreateGqlMenuInput",
        "pub translations: Vec<GqlMenuTranslationInput>",
        "pub struct GqlMenuItemInput",
        "pub translations: Vec<GqlMenuItemTranslationInput>",
        "async fn create_menu(",
        "resolve_graphql_locale(ctx, locale.as_deref())",
    ] {
        assert!(
            GRAPHQL_TYPES.contains(marker) || GRAPHQL_MUTATION.contains(marker),
            "GraphQL menu write contract must contain `{marker}`"
        );
    }

    let menu_item_output = GRAPHQL_TYPES
        .split("pub struct GqlMenuItem {")
        .nth(1)
        .and_then(|tail| tail.split("#[derive(InputObject)]").next())
        .expect("GqlMenuItem output contract should exist");
    assert!(menu_item_output.contains("pub title: String"));
    assert!(!menu_item_output.contains("pub title: Option<String>"));
    assert!(!GRAPHQL_MUTATION.contains("PLATFORM_FALLBACK_LOCALE"));
}

#[test]
fn menu_http_and_openapi_surfaces_stay_synchronized() {
    for marker in [
        "path = \"/api/menus/{id}\"",
        "path = \"/api/admin/menus\"",
        ".route(\"/api/menus/{id}\", axum::routing::get(get_menu))",
        ".route(\"/api/admin/menus\", axum::routing::post(create_menu))",
        "request_context.locale.clone()",
    ] {
        assert!(
            HTTP.contains(marker),
            "HTTP menu transport must contain `{marker}`"
        );
    }

    for marker in [
        "crate::controllers::get_menu",
        "crate::controllers::create_menu",
        "crate::CreateMenuInput",
        "crate::MenuResponse",
    ] {
        assert!(
            OPENAPI.contains(marker),
            "OpenAPI menu contract must contain `{marker}`"
        );
    }
}
