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
#[cfg(feature = "server")]
pub use catalog::{AiProviderTarget, AiProviderTargetCatalog, ProviderEgressPolicy};
pub use catalog::{
    ProviderCatalogEntry, ProviderConfigField, ProviderDefaultSetting, ProviderFeature,
    ProviderFieldKind, ProviderSlug, ProviderTargetAuth, ProviderTargetId, provider_catalog,
    provider_catalog_entry,
};
#[cfg(feature = "server")]
pub use inference::{InferenceEngine, inference_for_slug};
#[cfg(feature = "server")]
pub(crate) use inference::{assistant_choice, map_message, map_rig_message};
#[cfg(feature = "server")]
pub use vectors::{
    EmbeddingRequest, EmbeddingResponse, RerankItem, RerankRequest, RerankResponse, embed, rerank,
};
