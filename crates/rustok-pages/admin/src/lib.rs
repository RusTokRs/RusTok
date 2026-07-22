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
mod rollback_control;
mod transport;

use composition::PagesAdmin as PagesWorkspace;
use leptos::prelude::*;
use rollback_control::PagesRollbackControl;

pub use access::{
    pages_editor_capability_policy, pages_editor_capability_policy_for_role,
    pages_editor_permissions_for_role, pages_editor_provider_state,
};
pub use browser_intent::{
    PagesBrowserIntentError, PagesBrowserIntentResponse, pages_browser_draft_store,
};
pub use browser_problem::PagesBrowserIntentProblem;
pub use builder::PagesBuilderSaveSnapshot;
pub use contribution_browser_intent::{
    PagesBrowserIntentAccessError, dispatch_pages_browser_intent,
    dispatch_pages_browser_intent_with_capabilities, dispatch_pages_browser_intent_with_store,
    dispatch_pages_browser_intent_with_store_and_capabilities, pages_palette_block_access,
};
pub use contributions::{
    FLY_BUILTIN_PROVIDER, PAGES_BUILDER_CAPABILITIES, PAGES_LANDING_BLOCK_CAPABILITIES,
    PAGES_LANDING_BLOCK_IDS, PAGES_LANDING_BLOCKS_CONTRIBUTION_ID, PAGES_MODULE_ID,
    PAGES_OWNER_PROVIDER, build_pages_admin_contribution_registry, pages_admin_contribution_policy,
    pages_contribution_manifest, pages_landing_blocks_contribution,
};
pub use fly_browser::BrowserIntentEnvelope;

#[component]
pub fn PagesAdmin() -> impl IntoView {
    let refresh_generation = RwSignal::new(0_u64);
    let on_rolled_back = Callback::new(move |()| {
        refresh_generation.update(|generation| *generation = generation.wrapping_add(1));
    });

    view! {
        <div class="space-y-4">
            <PagesRollbackControl on_rolled_back />
            {move || {
                let generation = refresh_generation.get();
                view! {
                    <div data-pages-rollback-generation=generation>
                        <PagesWorkspace />
                    </div>
                }
            }}
        </div>
    }
}
