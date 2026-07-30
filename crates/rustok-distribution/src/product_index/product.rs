use rustok_core::ModuleRuntimeExtensions;

pub(crate) use super::graph::PRODUCT_INDEX_SOURCE;

pub(crate) fn register(extensions: &mut ModuleRuntimeExtensions) -> rustok_core::Result<()> {
    super::graph::register_product(extensions)
}
