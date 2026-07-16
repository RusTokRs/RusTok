#![recursion_limit = "256"]

#[cfg(test)]
mod context_contract;

pub mod browser_intent;
pub mod editor;
mod i18n;
mod model;
pub mod transport;
pub mod ui;

pub use browser_intent::{
    dispatch_browser_intent, BrowserIntentDispatchError, BrowserIntentDispatchResult,
    BrowserIntentEffect,
};
pub use model::{AdminCanvasController, AdminCanvasEffect, AdminCanvasError};
pub use transport::{
    PageBuilderAdminFacade, PageBuilderAdminFacadeError, PageBuilderAdminFacadeFuture,
};
pub use ui::leptos::{
    PageBuilderAdmin, PageBuilderAdminHostContext, PageBuilderAdminWithController,
};
