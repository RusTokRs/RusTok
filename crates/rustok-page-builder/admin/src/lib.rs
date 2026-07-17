#![recursion_limit = "256"]

#[cfg(test)]
mod context_contract;
#[cfg(test)]
mod ssr_actions_forms_browser_tests;

pub mod browser_intent;
pub mod draft_session;
pub mod editor;
mod i18n;
mod model;
pub mod transport;
pub mod ui;

pub use browser_intent::{
    dispatch_browser_intent, BrowserIntentDispatchError, BrowserIntentDispatchResult,
    BrowserIntentEffect,
};
pub use draft_session::{
    InMemorySsrDraftSessionStore, SsrDraftSessionError, SsrDraftSessionSnapshot,
    SsrDraftSessionStore,
};
pub use model::{AdminCanvasController, AdminCanvasEffect, AdminCanvasError};
pub use transport::{
    PageBuilderAdminFacade, PageBuilderAdminFacadeError, PageBuilderAdminFacadeFuture,
};
pub use ui::leptos::{
    PageBuilderAdmin, PageBuilderAdminHostContext, PageBuilderAdminWithController,
};
