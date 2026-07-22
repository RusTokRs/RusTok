mod core;
mod generated_ui_codegen {
    include!(concat!(env!("OUT_DIR"), "/module_ui_codegen.rs"));
}
mod registry;
mod search_composition;

use std::sync::OnceLock;

pub use registry::{
    StorefrontComponentRegistration, StorefrontPageLookup, StorefrontPageRegistration,
    StorefrontSlot, components_for_slot, page_for_route_segment, register_component, register_page,
};
pub use search_composition::SearchStorefrontComposition;

pub fn init_modules() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        core::register_components();
        generated_ui_codegen::register_generated_components();
    });
}
