from pathlib import Path


def read(path: str) -> str:
    return Path(path).read_text()


def write(path: str, text: str) -> None:
    Path(path).write_text(text)


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


# Shared HTTP DTO for the admin bind/replace command.
path = "crates/rustok-pages/src/dto/menu.rs"
text = read(path)
marker = '''#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ActiveMenuBindingResponse {
'''
insert = '''#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct BindActiveMenuInput {
    pub menu_id: Uuid,
}

'''
text = replace_once(text, marker, insert + marker, "BindActiveMenuInput insertion")
write(path, text)

path = "crates/rustok-pages/src/dto/mod.rs"
text = read(path)
text = replace_once(
    text,
    '''    ActiveMenuBindingResponse, CreateMenuInput, MenuItemInput, MenuItemResponse,
    MenuItemTranslationInput, MenuLocation, MenuResponse, MenuTranslationInput,
''',
    '''    ActiveMenuBindingResponse, BindActiveMenuInput, CreateMenuInput, MenuItemInput,
    MenuItemResponse, MenuItemTranslationInput, MenuLocation, MenuResponse, MenuTranslationInput,
''',
    "menu DTO exports",
)
write(path, text)


# GraphQL types: current-scope bind input and binding response.
path = "crates/rustok-pages/src/graphql/types.rs"
text = read(path)
marker = '''#[derive(Clone, Debug, SimpleObject)]
pub struct GqlMenuItem {
'''
insert = '''#[derive(Clone, Debug, SimpleObject)]
pub struct GqlActiveMenuBinding {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub channel_id: Uuid,
    pub location: GqlMenuLocation,
    pub menu_id: Uuid,
}

'''
text = replace_once(text, marker, insert + marker, "GraphQL binding output")

marker = '''#[derive(InputObject)]
pub struct GqlMenuTranslationInput {
'''
insert = '''#[derive(InputObject)]
pub struct BindGqlActiveMenuInput {
    pub location: GqlMenuLocation,
    pub menu_id: Uuid,
}

'''
text = replace_once(text, marker, insert + marker, "GraphQL binding input")

marker = '''impl From<crate::MenuItemResponse> for GqlMenuItem {
'''
insert = '''impl From<crate::ActiveMenuBindingResponse> for GqlActiveMenuBinding {
    fn from(binding: crate::ActiveMenuBindingResponse) -> Self {
        Self {
            id: binding.id,
            tenant_id: binding.tenant_id,
            channel_id: binding.channel_id,
            location: binding.location.into(),
            menu_id: binding.menu_id,
        }
    }
}

'''
text = replace_once(text, marker, insert + marker, "GraphQL binding conversion")

marker = '''impl From<crate::MenuLocation> for GqlMenuLocation {
'''
insert = '''impl From<GqlMenuLocation> for crate::MenuLocation {
    fn from(location: GqlMenuLocation) -> Self {
        match location {
            GqlMenuLocation::Header => Self::Header,
            GqlMenuLocation::Footer => Self::Footer,
            GqlMenuLocation::Sidebar => Self::Sidebar,
            GqlMenuLocation::Mobile => Self::Mobile,
        }
    }
}

'''
text = replace_once(text, marker, insert + marker, "GraphQL location input conversion")
write(path, text)


# GraphQL public active lookup: exact locale, current tenant and current channel only.
path = "crates/rustok-pages/src/graphql/query.rs"
text = read(path)
text = replace_once(
    text,
    '''    MENU_LOCALE_NOT_FOUND_ERROR_CODE, MENU_TRANSLATION_INTEGRITY_ERROR_CODE, MenuService,
''',
    '''    MENU_LOCALE_NOT_FOUND_ERROR_CODE, MENU_TRANSLATION_INTEGRITY_ERROR_CODE,
    MenuBindingService, MenuService,
''',
    "query service imports",
)
marker = '''    async fn page_by_slug(
'''
insert = '''    async fn active_menu(
        &self,
        ctx: &Context<'_>,
        location: GqlMenuLocation,
        locale: Option<String>,
    ) -> Result<Option<GqlMenu>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_public_pages_channel_enabled(ctx).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let Some(channel_id) = ctx
            .data_opt::<RequestContext>()
            .and_then(|request_context| request_context.channel_id)
        else {
            return Ok(None);
        };
        let effective_locale = resolve_graphql_locale(ctx, locale.as_deref());

        match MenuBindingService::new(db.clone(), event_bus.clone())
            .get_active(
                tenant.id,
                request_security_context(ctx),
                channel_id,
                location.into(),
                &effective_locale,
            )
            .await
        {
            Ok(menu) => Ok(menu.map(Into::into)),
            Err(crate::PagesError::MenuNotFound(_)) => Ok(None),
            Err(crate::PagesError::Rich(rich))
                if rich.error_code.as_deref() == Some(MENU_LOCALE_NOT_FOUND_ERROR_CODE) =>
            {
                Ok(None)
            }
            Err(error) => Err(map_menu_query_error(error)),
        }
    }

'''
text = replace_once(text, marker, insert + marker, "activeMenu query")
write(path, text)


# GraphQL admin bind/replace: no tenant/channel arguments, both come from context.
path = "crates/rustok-pages/src/graphql/mutation.rs"
text = read(path)
text = replace_once(
    text,
    '''    Action, AuthContext, Permission, Resource, TenantContext,
''',
    '''    Action, AuthContext, Permission, RequestContext, Resource, TenantContext,
''',
    "mutation RequestContext import",
)
text = replace_once(
    text,
    '''    CANNOT_DELETE_PUBLISHED_ERROR_CODE, CreateMenuInput, CreatePageInput, MenuItemInput,
    MenuItemTranslationInput, MenuLocation, MenuService, MenuTranslationInput, PageBodyInput,
''',
    '''    CANNOT_DELETE_PUBLISHED_ERROR_CODE, CreateMenuInput, CreatePageInput, MenuBindingService,
    MenuItemInput, MenuItemTranslationInput, MenuLocation, MenuService, MenuTranslationInput,
    PageBodyInput,
''',
    "mutation binding service import",
)
text = replace_once(
    text,
    '''const PAGE_CREATE_PUBLISH_REQUIRES_REVIEWED_COMMAND: &str =
    "PAGE_CREATE_PUBLISH_REQUIRES_REVIEWED_COMMAND";
''',
    '''const PAGE_CREATE_PUBLISH_REQUIRES_REVIEWED_COMMAND: &str =
    "PAGE_CREATE_PUBLISH_REQUIRES_REVIEWED_COMMAND";
const CHANNEL_CONTEXT_REQUIRED: &str = "CHANNEL_CONTEXT_REQUIRED";
''',
    "channel context error code",
)
marker = '''    async fn patch_page_metadata(
'''
insert = '''    async fn bind_active_menu(
        &self,
        ctx: &Context<'_>,
        input: BindGqlActiveMenuInput,
    ) -> Result<GqlActiveMenuBinding> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        let db = ctx.data::<DatabaseConnection>()?;
        let event_bus = ctx.data::<TransactionalEventBus>()?;
        let auth = require_pages_permission(ctx, Permission::PAGES_UPDATE)?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = mutation_tenant_id(tenant, &auth, None)?;
        let channel_id = current_channel_id(ctx)?;

        MenuBindingService::new(db.clone(), event_bus.clone())
            .bind(
                tenant_id,
                page_security(&auth),
                channel_id,
                input.location.into(),
                input.menu_id,
            )
            .await
            .map(Into::into)
            .map_err(map_pages_error)
    }

'''
text = replace_once(text, marker, insert + marker, "bindActiveMenu mutation")
marker = '''fn create_publish_bypass_error() -> async_graphql::Error {
'''
insert = '''fn current_channel_id(ctx: &Context<'_>) -> Result<Uuid> {
    ctx.data_opt::<RequestContext>()
        .and_then(|request_context| request_context.channel_id)
        .ok_or_else(|| {
            async_graphql::Error::new(
                "Active menu binding requires a resolved current channel",
            )
            .extend_with(|_, extensions| {
                extensions.set("code", CHANNEL_CONTEXT_REQUIRED);
            })
        })
}

'''
text = replace_once(text, marker, insert + marker, "current GraphQL channel helper")
write(path, text)


# HTTP current-scope active lookup and idempotent admin bind/replace.
path = "crates/rustok-pages/src/controllers/mod.rs"
text = read(path)
text = replace_once(
    text,
    '''    CANNOT_DELETE_PUBLISHED_ERROR_CODE, CreateMenuInput, CreatePageInput, MenuResponse,
    MenuService, PAGE_DOCUMENT_REVISION_CONFLICT, PAGE_PUBLISHED_DOCUMENT_IMMUTABLE,
''',
    '''    ActiveMenuBindingResponse, BindActiveMenuInput, CANNOT_DELETE_PUBLISHED_ERROR_CODE,
    CreateMenuInput, CreatePageInput, MenuBindingService, MenuLocation, MenuResponse, MenuService,
    PAGE_DOCUMENT_REVISION_CONFLICT, PAGE_PUBLISHED_DOCUMENT_IMMUTABLE,
''',
    "HTTP active menu imports",
)
marker = '''#[utoipa::path(
    get,
    path = "/api/pages/{id}/artifact",
'''
insert = '''#[utoipa::path(
    get,
    path = "/api/menus/active/{location}",
    tag = "pages",
    params(
        ("location" = MenuLocation, Path, description = "Current channel menu location"),
        GetMenuParams
    ),
    responses(
        (status = 200, description = "Exact-locale active menu for the current tenant and channel", body = MenuResponse),
        (status = 404, description = "Active menu, current channel, or localized menu copy not found"),
        (status = 500, description = "Menu translation integrity failure")
    )
)]
pub async fn get_active_menu(
    State(runtime): State<PagesHttpRuntime>,
    tenant: TenantContext,
    request_context: RequestContext,
    Path(location): Path<MenuLocation>,
    Query(params): Query<GetMenuParams>,
) -> HttpResult<Json<MenuResponse>> {
    let channel_id = current_public_channel_id(&request_context)?;
    ensure_menu_module_enabled_for_channel(&runtime, &request_context).await?;
    let effective_locale = params
        .locale
        .unwrap_or_else(|| request_context.locale.clone());

    MenuBindingService::new(runtime.db_clone(), runtime.event_bus())
        .get_active(
            tenant.id,
            rustok_core::SecurityContext::public_read(),
            channel_id,
            location,
            &effective_locale,
        )
        .await
        .map_err(map_pages_error)?
        .map(Json)
        .ok_or_else(|| {
            HttpError::not_found("active_menu_not_found", "Active menu was not found")
        })
}

'''
text = replace_once(text, marker, insert + marker, "HTTP active menu GET")

marker = '''#[utoipa::path(
    patch,
    path = "/api/admin/pages/{id}/metadata",
'''
insert = '''#[utoipa::path(
    put,
    path = "/api/admin/menus/active/{location}",
    tag = "pages",
    params(("location" = MenuLocation, Path, description = "Current channel menu location")),
    request_body = BindActiveMenuInput,
    responses(
        (status = 200, description = "Active menu binding created or replaced", body = ActiveMenuBindingResponse),
        (status = 400, description = "Current channel is missing or binding input is invalid"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Menu not found")
    )
)]
pub async fn bind_active_menu(
    State(runtime): State<PagesHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(location): Path<MenuLocation>,
    Json(input): Json<BindActiveMenuInput>,
) -> HttpResult<Json<ActiveMenuBindingResponse>> {
    ensure_pages_permission(&auth, Permission::PAGES_UPDATE)?;
    let channel_id = current_admin_channel_id(&request_context)?;

    MenuBindingService::new(runtime.db_clone(), runtime.event_bus())
        .bind(
            tenant.id,
            page_security(&auth),
            channel_id,
            location,
            input.menu_id,
        )
        .await
        .map(Json)
        .map_err(map_pages_error)
}

'''
text = replace_once(text, marker, insert + marker, "HTTP active menu PUT")

text = replace_once(
    text,
    '''        .route("/api/menus/{id}", axum::routing::get(get_menu))
''',
    '''        .route("/api/menus/{id}", axum::routing::get(get_menu))
        .route(
            "/api/menus/active/{location}",
            axum::routing::get(get_active_menu),
        )
''',
    "public active menu route",
)
text = replace_once(
    text,
    '''        .route("/api/admin/menus", axum::routing::post(create_menu))
''',
    '''        .route("/api/admin/menus", axum::routing::post(create_menu))
        .route(
            "/api/admin/menus/active/{location}",
            axum::routing::put(bind_active_menu),
        )
''',
    "admin active menu route",
)
marker = '''async fn ensure_menu_module_enabled_for_channel(
'''
insert = '''fn current_public_channel_id(request_context: &RequestContext) -> HttpResult<Uuid> {
    request_context.channel_id.ok_or_else(|| {
        HttpError::not_found("active_menu_not_found", "Active menu was not found")
    })
}

fn current_admin_channel_id(request_context: &RequestContext) -> HttpResult<Uuid> {
    request_context.channel_id.ok_or_else(|| {
        HttpError::bad_request(
            "channel_context_required",
            "Active menu binding requires a resolved current channel",
        )
    })
}

'''
text = replace_once(text, marker, insert + marker, "HTTP current channel helpers")
write(path, text)


# OpenAPI must expose both paths and both shared DTOs.
path = "crates/rustok-pages/src/openapi.rs"
text = read(path)
text = replace_once(
    text,
    '''        crate::controllers::get_menu,
        crate::controllers::get_page_artifact,
''',
    '''        crate::controllers::get_menu,
        crate::controllers::get_active_menu,
        crate::controllers::get_page_artifact,
''',
    "OpenAPI public active menu path",
)
text = replace_once(
    text,
    '''        crate::controllers::create_menu,
        crate::controllers::patch_page_metadata,
''',
    '''        crate::controllers::create_menu,
        crate::controllers::bind_active_menu,
        crate::controllers::patch_page_metadata,
''',
    "OpenAPI admin active menu path",
)
text = replace_once(
    text,
    '''            crate::CreateMenuInput,
            crate::MenuTranslationInput,
''',
    '''            crate::CreateMenuInput,
            crate::BindActiveMenuInput,
            crate::ActiveMenuBindingResponse,
            crate::MenuTranslationInput,
''',
    "OpenAPI active menu schemas",
)
write(path, text)


# Source guard: transports use current context and stay synchronized.
path = "crates/rustok-pages/tests/menu_transport_contract.rs"
text = read(path)
text = replace_once(
    text,
    '''const MENU_BINDING_MIGRATION: &str =
    include_str!("../src/migrations/m20260721_000008_create_active_menu_bindings.rs");
''',
    '''const MENU_BINDING_MIGRATION: &str =
    include_str!("../src/migrations/m20260721_000008_create_active_menu_bindings.rs");
const MENU_DTO: &str = include_str!("../src/dto/menu.rs");
''',
    "menu DTO source guard constant",
)
marker = '''#[test]
fn active_menu_binding_is_deterministic_and_tenant_safe() {
'''
insert = '''#[test]
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

'''
text = replace_once(text, marker, insert + marker, "active menu transport source guard")
write(path, text)
