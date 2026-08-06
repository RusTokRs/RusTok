use rustok_core::ModuleRuntimeExtensions;

pub(crate) fn register(extensions: &mut ModuleRuntimeExtensions) -> rustok_core::Result<()> {
    crate::product_index::graph::register_variant(extensions)
}
