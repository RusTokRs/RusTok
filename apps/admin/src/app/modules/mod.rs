mod core;
mod generated_ui_codegen {
    include!(concat!(env!("OUT_DIR"), "/module_registry_codegen.rs"));
}
mod registry;

use std::cell::Cell;

pub use generated_ui_codegen::core_module_slugs;
pub use registry::{
    components_for_slot, page_for_route_segment, register_component, register_page,
    AdminChildPageRegistration, AdminComponentRegistration, AdminPageRegistration, AdminSlot,
};

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
