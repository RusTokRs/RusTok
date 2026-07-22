#![recursion_limit = "256"]

#[cfg(test)]
mod context_contract;
#[cfg(test)]
mod ssr_actions_forms_browser_tests;
#[cfg(test)]
mod ssr_assets_browser_tests;

pub mod browser_intent;
mod capability_access;
pub mod draft_session;
pub mod editor;
mod i18n;
mod model;
mod palette_access;
pub mod publish_scenario_selection;
pub mod transport;
pub mod ui;

pub const BROWSER_CAPABILITY_DENIAL_CODE: &str = "FLY_CAPABILITY_DENIED";

pub use browser_intent::{
    BrowserIntentDispatchError, BrowserIntentDispatchResult, BrowserIntentEffect,
    dispatch_browser_intent,
};
pub use capability_access::{
    BrowserCapabilityAccessError, BrowserCapabilityDenial, CapabilityFailure,
    browser_capability_denial, validate_browser_capability_access,
};
pub use draft_session::{
    InMemorySsrDraftSessionStore, SsrDraftSessionError, SsrDraftSessionSnapshot,
    SsrDraftSessionStore,
};
pub use model::{AdminCanvasController, AdminCanvasEffect, AdminCanvasError};
pub use palette_access::{
    dispatch_browser_intent_with_palette_access, validate_browser_palette_access,
};
pub use publish_scenario_selection::{
    PAGE_BUILDER_PUBLISH_SCENARIO_SELECTION_FORMAT, PublishScenarioSelectionError,
    load_publish_scenario_selection, publish_scenario_selection_key, resolve_publish_scenario,
    save_publish_scenario_selection,
};
pub use transport::{
    PageBuilderAdminFacade, PageBuilderAdminFacadeError, PageBuilderAdminFacadeFuture,
};
pub use ui::leptos::{
    PageBuilderAdmin, PageBuilderAdminHostContext, PageBuilderAdminWithController,
};
