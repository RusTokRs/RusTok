mod core;
#[cfg(feature = "ssr")]
mod forum_reactions_composition;
mod generated_ui_codegen {
    include!(concat!(env!("OUT_DIR"), "/module_ui_codegen.rs"));
}
mod registry;
// Client-only codegen registers no module surfaces, so it does not enable the
// optional Product/Search packages used by this server composition.
#[cfg(feature = "ssr")]
mod search_composition;

use std::sync::OnceLock;

#[cfg(feature = "ssr")]
pub use forum_reactions_composition::ForumStorefrontComposition;
pub use registry::{
    StorefrontComponentRegistration, StorefrontPageLookup, StorefrontPageRegistration,
    StorefrontSlot, components_for_slot, page_for_route_segment, register_component, register_page,
};
#[cfg(feature = "ssr")]
pub use search_composition::SearchStorefrontComposition;

pub fn init_modules() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        core::register_components();
        generated_ui_codegen::register_generated_components();
    });
}
