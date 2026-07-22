use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::controllers::get_page,
        crate::controllers::get_page_artifact,
        crate::controllers::create_page,
        crate::controllers::patch_page_metadata,
        crate::controllers::save_page_document,
        crate::http::publish_page,
        crate::http::rollback_page,
        crate::controllers::delete_page,
    ),
    components(
        schemas(
            crate::CreatePageInput,
            crate::PatchPageMetadataInput,
            crate::SavePageDocumentInput,
            crate::PublishPageInput,
            crate::PublishPageResult,
            crate::RollbackPageInput,
            crate::RollbackPageResult,
            crate::PageBodyRevisionInput,
            crate::ReviewedPagePublishRuntimeInput,
            crate::PageBodyInput,
            crate::PageResponse,
            crate::controllers::GetPageParams,
            crate::controllers::GetPageArtifactParams,
        )
    ),
    tags((name = "pages", description = "Pages and published artifact endpoints"))
)]
pub struct PagesApiDoc;

pub fn openapi_document() -> utoipa::openapi::OpenApi {
    PagesApiDoc::openapi()
}
