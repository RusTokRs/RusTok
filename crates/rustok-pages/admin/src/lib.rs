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
mod metadata_properties;
mod model;
mod rollback_control;
mod transport;

use builder::PagesBuilderSaveSnapshot;
use composition::PagesAdmin as PagesWorkspace;
use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::use_route_query_value;
use metadata_properties::pages_metadata_property_runtime;
use rollback_control::PagesRollbackControl;
use rustok_ui_core::{AdminQueryKey, UiRouteContext};

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
    PAGES_LANDING_BLOCK_IDS, PAGES_LANDING_BLOCKS_CONTRIBUTION_ID, PAGES_METADATA_CAPABILITIES,
    PAGES_METADATA_COMPONENT_TYPE, PAGES_METADATA_CONTRIBUTION_ID,
    PAGES_METADATA_PROPERTY_EDITOR_ID, PAGES_MODULE_ID, PAGES_OWNER_PROVIDER,
    build_pages_admin_contribution_registry, pages_admin_contribution_policy,
    pages_contribution_manifest, pages_landing_blocks_contribution, pages_metadata_contribution,
    pages_metadata_property_schema,
};
pub use fly_browser::BrowserIntentEnvelope;

#[component]
pub fn PagesAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let default_locale = route_context.locale.unwrap_or_else(|| "en".to_string());
    let selected_page = use_route_query_value(AdminQueryKey::PageId.as_str());
    let token = use_token();
    let tenant = use_tenant();
    let refresh_generation = RwSignal::new(0_u64);

    let metadata_page = selected_page;
    let metadata_token = token;
    let metadata_tenant = tenant;
    let metadata_default_locale = default_locale.clone();
    let metadata_refresh = refresh_generation;
    let metadata_runtime = pages_metadata_property_runtime(
        move || PagesBuilderSaveSnapshot {
            token: metadata_token.get_untracked(),
            tenant_slug: metadata_tenant.get_untracked(),
            page_id: metadata_page.get_untracked().unwrap_or_default(),
            default_locale: metadata_default_locale.clone(),
        },
        move |_page| {
            metadata_refresh.update(|generation| *generation = generation.wrapping_add(1));
        },
    );
    provide_context(metadata_runtime);

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
