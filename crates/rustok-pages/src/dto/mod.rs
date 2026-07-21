// DTOs for pages-related requests/responses.
// Temporary diagnostic trigger; removed before merge.
pub mod menu;
pub mod page;

pub use menu::{
    CreateMenuInput, MenuItemInput, MenuItemResponse, MenuItemTranslationInput, MenuLocation,
    MenuResponse, MenuTranslationInput,
};
pub use page::{
    CreatePageInput, ListPagesFilter, PageBodyInput, PageBodyResponse, PageBodyRevisionInput,
    PageListItem, PageResponse, PageTranslationInput, PageTranslationResponse,
    PatchPageMetadataInput, PublishPageInput, PublishPageResult, ReviewedPagePublishRuntimeInput,
    SavePageDocumentInput,
};
