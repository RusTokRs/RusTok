mod builder;
mod core;
mod i18n;
#[cfg(feature = "inline-edit")]
mod inline_edit;
mod model;
mod transport;
mod ui;

pub use builder::{
    PAGE_BUILDER_DOCUMENT_FORMAT, PageBuilderPageBody, STATIC_LANDING_URL_BODY_FORMAT,
    decode_page_builder_body, is_page_builder_body,
};
#[cfg(feature = "inline-edit")]
pub use inline_edit::{
    PagesAuthenticatedInlineEditSurface, PagesInlineEditBootstrap, commit_pages_inline_edit,
    fetch_pages_inline_edit_bootstrap,
};
pub use transport::{
    StorefrontPageRouteDecision, StorefrontPageRouteDisposition, resolve_storefront_page_route,
};
pub use ui::leptos::PagesView;
