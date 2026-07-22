// DTOs for pages-related requests/responses.
pub mod menu;
pub mod page;

pub use menu::{
    ActiveMenuBindingResponse, BindActiveMenuInput, CreateMenuInput, MenuItemInput,
    MenuItemResponse, MenuItemTranslationInput, MenuLocation, MenuResponse, MenuTranslationInput,
};
pub use page::{
    CreatePageInput, ListPagesFilter, PageBodyInput, PageBodyResponse, PageBodyRevisionInput,
    PageListItem, PageResponse, PageTranslationInput, PageTranslationResponse,
    PatchPageMetadataInput, PublishPageInput, PublishPageResult, ReviewedPagePublishRuntimeInput,
    SavePageDocumentInput,
};
