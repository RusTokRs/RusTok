#[cfg(feature = "server")]
mod agent_driver;
mod catalog;
#[cfg(feature = "server")]
mod inference;
#[cfg(feature = "server")]
mod vectors;

#[cfg(feature = "server")]
pub use agent_driver::RigAgentDriver;
#[cfg(feature = "server")]
pub use catalog::provider_factory_supports;
pub use catalog::{
    provider_catalog, provider_catalog_entry, ProviderCatalogEntry, ProviderConfigField,
    ProviderDefaultSetting, ProviderFeature, ProviderFieldKind, ProviderSlug, ProviderTargetAuth,
    ProviderTargetId,
};
#[cfg(feature = "server")]
pub use catalog::{AiProviderTarget, AiProviderTargetCatalog, ProviderEgressPolicy};
#[cfg(feature = "server")]
pub(crate) use inference::{assistant_choice, map_message, map_rig_message};
#[cfg(feature = "server")]
pub use inference::{inference_for_slug, InferenceEngine};
#[cfg(feature = "server")]
pub use vectors::{
    embed, rerank, EmbeddingRequest, EmbeddingResponse, RerankItem, RerankRequest, RerankResponse,
};
