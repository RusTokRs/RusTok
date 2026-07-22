use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::controllers::get_page,
        crate::controllers::get_menu,
        crate::controllers::get_active_menu,
        crate::controllers::get_page_artifact,
        crate::controllers::create_page,
        crate::controllers::create_menu,
        crate::controllers::bind_active_menu,
        crate::controllers::patch_page_metadata,
        crate::controllers::save_page_document,
        crate::http::publish_page,
        crate::controllers::delete_page,
    ),
    components(
        schemas(
            crate::CreatePageInput,
            crate::PatchPageMetadataInput,
            crate::SavePageDocumentInput,
            crate::PublishPageInput,
            crate::PublishPageResult,
            crate::PageBodyRevisionInput,
            crate::ReviewedPagePublishRuntimeInput,
            crate::PageBodyInput,
            crate::PageResponse,
            crate::CreateMenuInput,
            crate::BindActiveMenuInput,
            crate::ActiveMenuBindingResponse,
            crate::MenuTranslationInput,
            crate::MenuItemInput,
            crate::MenuItemTranslationInput,
            crate::MenuResponse,
            crate::MenuItemResponse,
            crate::MenuLocation,
            crate::controllers::GetPageParams,
            crate::controllers::GetMenuParams,
            crate::controllers::CreateMenuParams,
            crate::controllers::GetPageArtifactParams,
        )
    ),
    tags((name = "pages", description = "Pages and localized menu endpoints"))
)]
pub struct PagesApiDoc;

pub fn openapi_document() -> utoipa::openapi::OpenApi {
    PagesApiDoc::openapi()
}
