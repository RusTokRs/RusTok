use utoipa::OpenApi;
#[derive(OpenApi)]
#[openapi(paths(crate::controllers::get_menu, crate::controllers::get_active_menu,
    crate::controllers::create_menu, crate::controllers::bind_active_menu),
    components(schemas(crate::CreateMenuInput, crate::BindActiveMenuInput, crate::ActiveMenuBindingResponse,
        crate::MenuTranslationInput, crate::MenuItemInput, crate::MenuItemTranslationInput,
        crate::MenuResponse, crate::MenuItemResponse, crate::MenuLocation,
        crate::controllers::GetMenuParams, crate::controllers::CreateMenuParams)),
    tags((name = "navigation", description = "Localized navigation menu endpoints")))]
pub struct NavigationApiDoc;
pub fn openapi_document() -> utoipa::openapi::OpenApi { NavigationApiDoc::openapi() }
