use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::controllers::get_page,
        crate::controllers::get_page_artifact,
        crate::controllers::create_page,
        crate::controllers::update_page,
        crate::controllers::delete_page,
        crate::controllers::create_block,
        crate::controllers::update_block,
        crate::controllers::delete_block,
        crate::controllers::reorder_blocks,
    ),
    components(
        schemas(
            crate::CreatePageInput,
            crate::UpdatePageInput,
            crate::CreateBlockInput,
            crate::UpdateBlockInput,
            crate::BlockResponse,
            crate::PageResponse,
            crate::controllers::GetPageParams,
            crate::controllers::GetPageArtifactParams,
            crate::controllers::ReorderBlocksInput,
        )
    ),
    tags((name = "pages", description = "Pages endpoints"))
)]
pub struct PagesApiDoc;

pub fn openapi_document() -> utoipa::openapi::OpenApi {
    PagesApiDoc::openapi()
}
