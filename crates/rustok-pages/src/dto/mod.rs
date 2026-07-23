// DTOs for pages-related requests/responses.
pub mod page;

pub use page::{
    CreatePageInput, ListPagesFilter, PageBodyInput, PageBodyResponse, PageBodyRevisionInput,
    PageListItem, PageResponse, PageTranslationInput, PageTranslationResponse,
    PatchPageMetadataInput, PublishPageInput, PublishPageResult, ReviewedPagePublishRuntimeInput,
    RollbackPageInput, RollbackPageResult, SavePageDocumentInput,
};
