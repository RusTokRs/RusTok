// DTOs for pages-related requests/responses.
pub mod menu;
pub mod page;

pub use menu::{CreateMenuInput, MenuItemInput, MenuItemResponse, MenuLocation, MenuResponse};
pub use page::{
    CreatePageInput, ListPagesFilter, PageBodyInput, PageBodyResponse, PageListItem, PageResponse,
    PageTranslationInput, PageTranslationResponse, UpdatePageInput,
};
