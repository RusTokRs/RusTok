mod catalog;
#[cfg(feature = "server")]
mod inference;

pub use catalog::{
    provider_catalog, provider_catalog_entry, ProviderCatalogEntry, ProviderConfigField,
    ProviderFeature, ProviderFieldKind, ProviderSlug,
};
#[cfg(feature = "server")]
pub(crate) use inference::{assistant_choice, map_message, map_rig_message};
#[cfg(feature = "server")]
pub use inference::{inference_for_slug, InferenceEngine};
