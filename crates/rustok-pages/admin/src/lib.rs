mod access;
mod browser_intent;
mod browser_problem;
mod builder;
#[cfg(test)]
mod builder_contract;
mod composition;
mod contribution_browser_intent;
mod contributions;
mod core;
mod i18n;
mod model;
mod transport;
pub mod ui;

pub use access::{
    pages_editor_capability_policy, pages_editor_capability_policy_for_role,
    pages_editor_permissions_for_role, pages_editor_provider_state,
};
pub use browser_intent::{
    pages_browser_draft_store, PagesBrowserIntentError, PagesBrowserIntentResponse,
};
pub use browser_problem::PagesBrowserIntentProblem;
pub use builder::PagesBuilderSaveSnapshot;
pub use composition::PagesAdmin;
pub use contribution_browser_intent::{
    dispatch_pages_browser_intent, dispatch_pages_browser_intent_with_capabilities,
    dispatch_pages_browser_intent_with_store,
    dispatch_pages_browser_intent_with_store_and_capabilities, pages_palette_block_access,
    PagesBrowserIntentAccessError,
};
pub use contributions::{
    build_pages_admin_contribution_registry, pages_admin_contribution_policy,
    pages_contribution_manifest, pages_landing_blocks_contribution, FLY_BUILTIN_PROVIDER,
    FLY_BUILTIN_PROVIDER_VERSION, PAGES_BUILDER_CAPABILITIES,
    PAGES_LANDING_BLOCK_CAPABILITIES, PAGES_LANDING_BLOCK_IDS,
    PAGES_LANDING_BLOCKS_CONTRIBUTION_ID, PAGES_MODULE_ID, PAGES_OWNER_PROVIDER,
};
pub use fly_browser::BrowserIntentEnvelope;
