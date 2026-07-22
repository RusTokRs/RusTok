from pathlib import Path
import subprocess

PRODUCT_BRANCH = "origin/agent/pages-active-menu-transport-v2"
COPY_PATHS = [
    "crates/rustok-pages/src/dto/menu.rs",
    "crates/rustok-pages/src/dto/mod.rs",
    "crates/rustok-pages/src/graphql/mutation.rs",
    "crates/rustok-pages/src/graphql/query.rs",
    "crates/rustok-pages/src/graphql/types.rs",
    "crates/rustok-pages/src/openapi.rs",
    "crates/rustok-pages/tests/menu_transport_contract.rs",
]


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


for path in COPY_PATHS:
    content = subprocess.check_output(
        ["git", "show", f"{PRODUCT_BRANCH}:{path}"], text=True
    )
    target = Path(path)
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content)

path = Path("crates/rustok-pages/src/controllers/mod.rs")
text = path.read_text()

if "ActiveMenuBindingResponse" not in text:
    text = replace_once(
        text,
        '''use crate::{
    CANNOT_DELETE_PUBLISHED_ERROR_CODE, CreateMenuInput, CreatePageInput, MenuResponse,
    MenuService, PAGE_DOCUMENT_REVISION_CONFLICT, PAGE_PUBLISHED_DOCUMENT_IMMUTABLE,
    PageBuilderArtifactService, PageCacheScope, PageResponse, PageService, PagesCacheReadRuntime,
    PagesError, PatchPageMetadataInput, PublishedLandingArtifact, SavePageDocumentInput,
    page_cache_key,
};
''',
        '''use crate::{
    ActiveMenuBindingResponse, BindActiveMenuInput, CANNOT_DELETE_PUBLISHED_ERROR_CODE,
    CreateMenuInput, CreatePageInput, MenuBindingService, MenuLocation, MenuResponse, MenuService,
    PAGE_DOCUMENT_REVISION_CONFLICT, PAGE_PUBLISHED_DOCUMENT_IMMUTABLE, PageBuilderArtifactService,
    PageCacheScope, PageResponse, PageService, PagesCacheReadRuntime, PagesError,
    PatchPageMetadataInput, PublishedLandingArtifact, SavePageDocumentInput, page_cache_key,
};
''',
        "controller imports",
    )

if "pub async fn get_active_menu(" not in text:
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
    text = replace_once(text, marker, insert + marker, "public active menu handler")

if "pub async fn bind_active_menu(" not in text:
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
    text = replace_once(text, marker, insert + marker, "admin active menu handler")

if '"/api/menus/active/{location}"' not in text.split("pub fn axum_router", 1)[1]:
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

if '"/api/admin/menus/active/{location}"' not in text.split("pub fn axum_router", 1)[1]:
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

if "fn current_public_channel_id(" not in text:
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
    text = replace_once(text, marker, insert + marker, "current channel helpers")

path.write_text(text)
