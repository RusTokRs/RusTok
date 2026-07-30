mod product;
#[path = "../product_variant_index.rs"]
mod variant;

pub(crate) use product::PRODUCT_INDEX_SOURCE;
pub(crate) use variant::PRODUCT_VARIANT_INDEX_SOURCE;

pub(crate) fn register(
    extensions: &mut rustok_core::ModuleRuntimeExtensions,
) -> rustok_core::Result<()> {
    product::register(extensions)?;
    variant::register(extensions)
}
