use rustok_core::ModuleRuntimeExtensions;

pub(crate) use crate::product_index::graph::PRODUCT_VARIANT_INDEX_SOURCE;

pub(crate) fn register(extensions: &mut ModuleRuntimeExtensions) -> rustok_core::Result<()> {
    crate::product_index::graph::register_variant(extensions)
}
