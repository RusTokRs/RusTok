use rustok_core::ModuleRuntimeExtensions;

pub(crate) fn register(extensions: &mut ModuleRuntimeExtensions) -> rustok_core::Result<()> {
    super::graph::register_product(extensions)
}
