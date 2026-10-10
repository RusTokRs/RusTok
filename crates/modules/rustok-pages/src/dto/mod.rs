// DTOs for pages-related requests/responses.
pub mod artifact_binding_replacement;
pub mod artifact_repair_transport;
pub mod page;

pub use artifact_binding_replacement::{
    ReplacePageArtifactBindingInput, ReplacePageArtifactBindingResult,
};
pub use artifact_repair_transport::{
    ActivateRebuiltPageArtifactTransportResult, RebuildPageArtifactTransportResult,
};
pub use page::{
    CreatePageInput, ListPagesFilter, PageBodyInput, PageBodyResponse,
    PageBodyRevisionInput, PageBodyRevisionResponse, PageBodyRevisionSource, PageBodyState,
    PageListItem, PagePublishJobState, PagePublishScheduleResponse, PageResponse,
    PageTranslationInput, PageTranslationResponse, PatchPageMetadataInput, PublishPageInput,
    PublishPageResult, RebuildPageArtifactInput, RebuildPageArtifactResult,
    RestorePageBodyRevisionInput, ReviewedPagePublishRuntimeInput, RollbackPageInput,
    RollbackPageResult, SavePageDocumentInput, SchedulePagePublishInput,
};
