mod core;
mod generated_ui_codegen {
    include!(concat!(env!("OUT_DIR"), "/module_registry_codegen.rs"));
}
mod registry;

use std::cell::Cell;

pub use generated_ui_codegen::core_module_slugs;
pub use generated_ui_codegen::module_runtime_metadata;
pub use registry::{
    components_for_slot, page_for_route_segment, register_component, register_page,
    AdminChildPageRegistration, AdminComponentRegistration, AdminPageRegistration, AdminSlot,
};

#[derive(Clone, Copy)]
pub struct GeneratedModuleRuntimeMetadata {
    pub ownership: &'static str,
    pub trust_level: &'static str,
    pub recommended_admin_surfaces: &'static [&'static str],
    pub showcase_admin_surfaces: &'static [&'static str],
}

thread_local! {
    static INIT: Cell<bool> = const { Cell::new(false) };
}

pub fn init_modules() {
    INIT.with(|flag| {
        if flag.get() {
            return;
        }
        flag.set(true);
        core::register_components();
        generated_ui_codegen::register_generated_components();
    });
}
