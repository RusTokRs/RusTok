const GRAPHQL_TYPES: &str = include_str!("../src/graphql/types.rs");
const GRAPHQL_QUERY: &str = include_str!("../src/graphql/query.rs");
const GRAPHQL_MUTATION: &str = include_str!("../src/graphql/mutation.rs");
const HTTP: &str = include_str!("../src/controllers/mod.rs");
const OPENAPI: &str = include_str!("../src/openapi.rs");
const MENU_BINDING_SERVICE: &str = include_str!("../src/services/menu_binding.rs");
const MENU_BINDING_MIGRATION: &str =
    include_str!("../src/migrations/m20260721_000008_create_active_menu_bindings.rs");
const MENU_DTO: &str = include_str!("../src/dto/menu.rs");
const MENU_MIGRATIONS: &str = include_str!("../src/migrations/mod.rs");

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

#[test]
fn active_menu_transport_is_current_scope_only() {
    for marker in [
        "async fn active_menu(",
        "MenuBindingService::new",
        "request_context.channel_id",
        "resolve_graphql_locale(ctx, locale.as_deref())",
        "async fn bind_active_menu(",
        "current_channel_id(ctx)?",
        "pub struct BindGqlActiveMenuInput",
        "pub location: GqlMenuLocation",
        "pub menu_id: Uuid",
    ] {
        assert!(
            GRAPHQL_QUERY.contains(marker)
                || GRAPHQL_MUTATION.contains(marker)
                || GRAPHQL_TYPES.contains(marker),
            "GraphQL active-menu transport must contain `{marker}`"
        );
    }

    let gql_bind_input = GRAPHQL_TYPES
        .split("pub struct BindGqlActiveMenuInput {")
        .nth(1)
        .and_then(|tail| tail.split('}').next())
        .expect("GraphQL active-menu bind input should exist");
    assert!(!gql_bind_input.contains("channel_id"));
    assert!(!gql_bind_input.contains("tenant_id"));

    for marker in [
        "path = \"/api/menus/active/{location}\"",
        "path = \"/api/admin/menus/active/{location}\"",
        ".route(\n            \"/api/menus/active/{location}\"",
        ".route(\n            \"/api/admin/menus/active/{location}\"",
        "current_public_channel_id(&request_context)?",
        "current_admin_channel_id(&request_context)?",
        "request_context.locale.clone()",
    ] {
        assert!(
            HTTP.contains(marker),
            "HTTP active-menu transport must contain `{marker}`"
        );
    }

    let http_bind_input = MENU_DTO
        .split("pub struct BindActiveMenuInput {")
        .nth(1)
        .and_then(|tail| tail.split('}').next())
        .expect("HTTP active-menu bind input should exist");
    assert!(http_bind_input.contains("pub menu_id: Uuid"));
    assert!(!http_bind_input.contains("channel_id"));
    assert!(!http_bind_input.contains("tenant_id"));

    for marker in [
        "crate::controllers::get_active_menu",
        "crate::controllers::bind_active_menu",
        "crate::BindActiveMenuInput",
        "crate::ActiveMenuBindingResponse",
    ] {
        assert!(
            OPENAPI.contains(marker),
            "OpenAPI active-menu contract must contain `{marker}`"
        );
    }

    assert!(!GRAPHQL_QUERY.contains("menu::Column::Location"));
    assert!(!HTTP.contains("menu_by_location"));
}

#[test]
fn active_menu_binding_is_deterministic_and_tenant_safe() {
    for marker in [
        "uq_menu_bindings_tenant_channel_location",
        "fk_menu_bindings_tenant_menu",
        ".from_col(MenuBindings::MenuId)",
        ".to_col(Menus::Id)",
        "fk_menu_bindings_channel",
        ".to(Channels::Table, Channels::Id)",
    ] {
        assert!(
            MENU_BINDING_MIGRATION.contains(marker),
            "active menu binding migration must contain `{marker}`"
        );
    }

    for marker in [
        ".filter(menu_binding::Column::TenantId.eq(tenant_id))",
        ".filter(menu_binding::Column::ChannelId.eq(channel_id))",
        ".filter(menu_binding::Column::Location.eq(",
        "ChannelService::new",
        "channel.tenant_id != tenant_id",
        "!channel.is_active",
        "MenuService::new",
    ] {
        assert!(
            MENU_BINDING_SERVICE.contains(marker),
            "active menu binding service must contain `{marker}`"
        );
    }

    for marker in [
        "m20260721_000008_create_active_menu_bindings",
        "m20260325_000001_create_channels",
    ] {
        assert!(
            MENU_MIGRATIONS.contains(marker),
            "menu migration ordering must contain `{marker}`"
        );
    }

    assert!(!MENU_BINDING_SERVICE.contains(".first()"));
    assert!(!MENU_BINDING_SERVICE.contains("menu::Column::Location"));
}
