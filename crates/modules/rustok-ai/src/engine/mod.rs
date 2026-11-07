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
pub(crate) use inference::{assistant_choice, extract_usage, map_message, map_rig_messages};
#[cfg(feature = "server")]
pub use vectors::{
    EmbeddingRequest, EmbeddingResponse, RerankItem, RerankRequest, RerankResponse, embed, rerank,
};

#[cfg(feature = "server")]
async fn resolve_primary_credential(
    config: &crate::AiProviderConfig,
    secrets: &rustok_secrets::SecretResolverRegistry,
) -> crate::AiResult<Option<secrecy::SecretString>> {
    use crate::AiError;

    if !matches!(config.target_auth, ProviderTargetAuth::SecretRefs) {
        return Ok(None);
    }
    let descriptor = provider_catalog_entry(&config.provider_slug).ok_or_else(|| {
        AiError::InvalidConfig(format!("unknown provider `{}`", config.provider_slug))
    })?;
    let Some(field) = descriptor.credentials.first() else {
        return Ok(None);
    };
    let reference = config.credential_refs.get(field.key).ok_or_else(|| {
        AiError::InvalidConfig(format!("credential reference `{}` is required", field.key))
    })?;
    secrets
        .resolve_for_tenant(config.tenant_id, reference)
        .await
        .map(Some)
        .map_err(|error| AiError::InvalidConfig(error.to_string()))
}
