use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::controllers::get_page,
        crate::controllers::get_page_artifact,
        crate::controllers::create_page,
        crate::controllers::patch_page_metadata,
        crate::controllers::save_page_document,
        crate::controllers::page_body_revision_history,
        crate::controllers::restore_page_body_revision,
        crate::controllers::duplicate_page,
        crate::controllers::page_publish_schedule,
        crate::controllers::schedule_page_publish,
        crate::controllers::cancel_page_publish,
        crate::http::publish_page,
        crate::http::rollback_page,
        crate::http::audit_page_artifacts,
        crate::http::rebuild_page_artifact,
        crate::http::activate_rebuilt_page_artifact,
        crate::controllers::delete_page,
    ),
    components(
        schemas(
            crate::CreatePageInput,
            crate::PatchPageMetadataInput,
            crate::SavePageDocumentInput,
            crate::RestorePageBodyRevisionInput,
            crate::PublishPageInput,
            crate::PublishPageResult,
            crate::RollbackPageInput,
            crate::RollbackPageResult,
            crate::AuditPageArtifactsInput,
            crate::PageArtifactIntegrityAuditResult,
            crate::PageArtifactIntegrityFinding,
            crate::RebuildPageArtifactInput,
            crate::RebuildPageArtifactTransportResult,
            crate::ReplacePageArtifactBindingInput,
            crate::ActivateRebuiltPageArtifactTransportResult,
            crate::PageBodyRevisionInput,
            crate::PageBodyRevisionResponse,
            crate::PageBodyRevisionSource,
            crate::PageBodyState,
            crate::PagePublishJobState,
            crate::PagePublishScheduleResponse,
            crate::SchedulePagePublishInput,
            crate::ReviewedPagePublishRuntimeInput,
            crate::PageBodyInput,
            crate::PageResponse,
            crate::controllers::GetPageParams,
            crate::controllers::GetPageArtifactParams,
            crate::controllers::PageBodyHistoryQuery,
        )
    ),
    tags((name = "pages", description = "Pages and published artifact endpoints"))
)]
pub struct PagesApiDoc;

pub fn openapi_document() -> utoipa::openapi::OpenApi {
    PagesApiDoc::openapi()
}
