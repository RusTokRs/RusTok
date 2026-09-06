use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{HostRuntimeContext, PortContext, PortError};
use rustok_core::ModuleRuntimeExtensions;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    ApplyReactionCommand, ReactionCatalog, ReactionContractError, ReactionReadRequest,
    ReactionSnapshot, ReactionSourceSlug, ReactionSubjectKind, ReactionSubjectRef,
    ReactionWriteReceipt,
};

pub const MAX_REACTION_PROVIDER_KINDS: usize = 32;

#[derive(Debug, Error, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum ReactionProviderError {
    #[error("reaction subject capability is unavailable")]
    CapabilityUnavailable { retryable: bool },
    #[error("reaction subject request is invalid")]
    InvalidRequest,
    #[error("reaction subject is unavailable")]
    Unavailable,
    #[error("reaction subject changed concurrently")]
    Conflict,
    #[error("reaction subject provider failed")]
    Internal { retryable: bool },
}

impl ReactionProviderError {
    pub const fn is_retryable(self) -> bool {
        match self {
            Self::CapabilityUnavailable { retryable } | Self::Internal { retryable } => retryable,
            Self::Conflict => true,
            Self::InvalidRequest | Self::Unavailable => false,
        }
    }
}

pub type ReactionProviderResult<T> = Result<T, ReactionProviderError>;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum ReactionSubjectAccess {
    Read { actor_id: Option<Uuid> },
    Apply { command: ApplyReactionCommand },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReactionSubjectRequest {
    pub subject: ReactionSubjectRef,
    pub access: ReactionSubjectAccess,
}

impl ReactionSubjectRequest {
    pub fn validate(&self) -> Result<(), ReactionContractError> {
        match &self.access {
            ReactionSubjectAccess::Read { actor_id } => {
                if actor_id.is_some_and(|actor_id| actor_id.is_nil()) {
                    return Err(ReactionContractError::NilIdentity);
                }
            }
            ReactionSubjectAccess::Apply { command } => {
                if &self.subject != command.subject() {
                    return Err(ReactionContractError::ProviderSubjectMismatch);
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum ReactionSubjectAuthorization {
    Allowed {
        canonical_subject: ReactionSubjectRef,
        catalog: ReactionCatalog,
    },
    Unavailable,
}

impl ReactionSubjectAuthorization {
    pub fn validate_for(
        &self,
        request: &ReactionSubjectRequest,
    ) -> Result<(), ReactionContractError> {
        if let Self::Allowed {
            canonical_subject, ..
        } = self
            && canonical_subject != &request.subject
        {
            return Err(ReactionContractError::ProviderSubjectMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReactionSubjectRegistryEntry {
    pub source: ReactionSourceSlug,
    pub display_name: String,
    pub supported_kinds: Vec<ReactionSubjectKind>,
}

#[async_trait]
pub trait ReactionSubjectProvider: Send + Sync {
    fn source(&self) -> ReactionSourceSlug;

    fn display_name(&self) -> &'static str;

    fn supported_kinds(&self) -> Vec<ReactionSubjectKind>;

    async fn authorize(
        &self,
        context: PortContext,
        request: ReactionSubjectRequest,
    ) -> ReactionProviderResult<ReactionSubjectAuthorization>;
}

pub trait ReactionSubjectProviderFactory: Send + Sync {
    fn source(&self) -> ReactionSourceSlug;

    fn build(
        &self,
        host: &HostRuntimeContext,
    ) -> ReactionProviderResult<Arc<dyn ReactionSubjectProvider>>;
}

#[async_trait]
pub trait ReactionReadPort: Send + Sync {
    async fn read_reactions(
        &self,
        context: PortContext,
        request: ReactionReadRequest,
    ) -> Result<ReactionSnapshot, PortError>;
}

#[async_trait]
pub trait ReactionWritePort: Send + Sync {
    async fn apply_reaction(
        &self,
        context: PortContext,
        command: ApplyReactionCommand,
    ) -> Result<ReactionWriteReceipt, PortError>;
}

#[derive(Debug, Error, Clone, Eq, PartialEq)]
pub enum ReactionSubjectRegistryError {
    #[error("reaction subject source `{0}` is already registered")]
    DuplicateSource(ReactionSourceSlug),
    #[error("reaction subject factory `{0}` is already registered")]
    DuplicateFactory(ReactionSourceSlug),
    #[error("reaction subject provider `{subject_source}` exposes an invalid kind set")]
    InvalidKinds { subject_source: ReactionSourceSlug },
    #[error("reaction subject factory `{declared}` built provider `{built}`")]
    FactorySourceMismatch {
        declared: ReactionSourceSlug,
        built: ReactionSourceSlug,
    },
    #[error("reaction subject factory `{source}` failed: {error}")]
    FactoryBuild {
        source: ReactionSourceSlug,
        #[source]
        error: ReactionProviderError,
    },
}

#[derive(Clone, Default)]
pub struct ReactionSubjectRegistry {
    providers: BTreeMap<ReactionSourceSlug, Arc<dyn ReactionSubjectProvider>>,
}

impl ReactionSubjectRegistry {
    pub fn register<P>(&mut self, provider: P) -> Result<(), ReactionSubjectRegistryError>
    where
        P: ReactionSubjectProvider + 'static,
    {
        self.register_arc(Arc::new(provider))
    }

    pub fn register_arc(
        &mut self,
        provider: Arc<dyn ReactionSubjectProvider>,
    ) -> Result<(), ReactionSubjectRegistryError> {
        let source = provider.source();
        validate_provider_kinds(&source, provider.supported_kinds())?;
        if self.providers.contains_key(&source) {
            return Err(ReactionSubjectRegistryError::DuplicateSource(source));
        }
        self.providers.insert(source, provider);
        Ok(())
    }

    pub fn get(&self, source: &ReactionSourceSlug) -> Option<Arc<dyn ReactionSubjectProvider>> {
        self.providers.get(source).cloned()
    }

    pub fn get_by_str(&self, source: &str) -> Option<Arc<dyn ReactionSubjectProvider>> {
        ReactionSourceSlug::new(source)
            .ok()
            .and_then(|source| self.get(&source))
    }

    pub fn entries(&self) -> Vec<ReactionSubjectRegistryEntry> {
        self.providers
            .iter()
            .map(|(source, provider)| {
                let mut supported_kinds = provider.supported_kinds();
                supported_kinds.sort();
                ReactionSubjectRegistryEntry {
                    source: source.clone(),
                    display_name: provider.display_name().to_string(),
                    supported_kinds,
                }
            })
            .collect()
    }

    pub fn len(&self) -> usize {
        self.providers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

fn validate_provider_kinds(
    source: &ReactionSourceSlug,
    kinds: Vec<ReactionSubjectKind>,
) -> Result<(), ReactionSubjectRegistryError> {
    if kinds.is_empty() || kinds.len() > MAX_REACTION_PROVIDER_KINDS {
        return Err(ReactionSubjectRegistryError::InvalidKinds {
            subject_source: source.clone(),
        });
    }
    if kinds.iter().collect::<BTreeSet<_>>().len() != kinds.len() {
        return Err(ReactionSubjectRegistryError::InvalidKinds {
            subject_source: source.clone(),
        });
    }
    Ok(())
}

#[derive(Clone, Default)]
pub struct ReactionSubjectFactoryRegistry {
    factories: BTreeMap<ReactionSourceSlug, Arc<dyn ReactionSubjectProviderFactory>>,
}

impl ReactionSubjectFactoryRegistry {
    pub fn register<F>(&mut self, factory: F) -> Result<(), ReactionSubjectRegistryError>
    where
        F: ReactionSubjectProviderFactory + 'static,
    {
        self.register_arc(Arc::new(factory))
    }

    pub fn register_arc(
        &mut self,
        factory: Arc<dyn ReactionSubjectProviderFactory>,
    ) -> Result<(), ReactionSubjectRegistryError> {
        let source = factory.source();
        if self.factories.contains_key(&source) {
            return Err(ReactionSubjectRegistryError::DuplicateFactory(source));
        }
        self.factories.insert(source, factory);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.factories.len()
    }

    pub fn is_empty(&self) -> bool {
        self.factories.is_empty()
    }
}

pub fn ensure_reaction_subject_registry(
    extensions: &mut ModuleRuntimeExtensions,
) -> Arc<ReactionSubjectRegistry> {
    extensions
        .get_or_insert_with::<Arc<ReactionSubjectRegistry>, _>(|| {
            Arc::new(ReactionSubjectRegistry::default())
        })
        .clone()
}

pub fn ensure_reaction_subject_factory_registry(
    extensions: &mut ModuleRuntimeExtensions,
) -> Arc<ReactionSubjectFactoryRegistry> {
    extensions
        .get_or_insert_with::<Arc<ReactionSubjectFactoryRegistry>, _>(|| {
            Arc::new(ReactionSubjectFactoryRegistry::default())
        })
        .clone()
}

pub fn register_reaction_subject_provider<P>(
    extensions: &mut ModuleRuntimeExtensions,
    provider: P,
) -> Result<(), ReactionSubjectRegistryError>
where
    P: ReactionSubjectProvider + 'static,
{
    let registry = extensions.get_or_insert_with::<Arc<ReactionSubjectRegistry>, _>(|| {
        Arc::new(ReactionSubjectRegistry::default())
    });
    Arc::make_mut(registry).register(provider)
}

pub fn register_reaction_subject_provider_factory<F>(
    extensions: &mut ModuleRuntimeExtensions,
    factory: F,
) -> Result<(), ReactionSubjectRegistryError>
where
    F: ReactionSubjectProviderFactory + 'static,
{
    let registry = extensions.get_or_insert_with::<Arc<ReactionSubjectFactoryRegistry>, _>(|| {
        Arc::new(ReactionSubjectFactoryRegistry::default())
    });
    Arc::make_mut(registry).register(factory)
}

pub fn materialize_reaction_subject_registry(
    extensions: &mut ModuleRuntimeExtensions,
    host: &HostRuntimeContext,
) -> Result<Arc<ReactionSubjectRegistry>, ReactionSubjectRegistryError> {
    let mut providers = reaction_subject_registry_from_extensions(extensions)
        .map(|registry| registry.as_ref().clone())
        .unwrap_or_default();
    let factories = reaction_subject_factory_registry_from_extensions(extensions)
        .unwrap_or_else(|| Arc::new(ReactionSubjectFactoryRegistry::default()));

    for (declared, factory) in &factories.factories {
        let provider =
            factory
                .build(host)
                .map_err(|error| ReactionSubjectRegistryError::FactoryBuild {
                    source: declared.clone(),
                    error,
                })?;
        let built = provider.source();
        if &built != declared {
            return Err(ReactionSubjectRegistryError::FactorySourceMismatch {
                declared: declared.clone(),
                built,
            });
        }
        providers.register_arc(provider)?;
    }

    let providers = Arc::new(providers);
    extensions.insert(providers.clone());
    Ok(providers)
}

pub fn reaction_subject_registry_from_extensions(
    extensions: &ModuleRuntimeExtensions,
) -> Option<Arc<ReactionSubjectRegistry>> {
    extensions.get::<Arc<ReactionSubjectRegistry>>().cloned()
}

pub fn reaction_subject_factory_registry_from_extensions(
    extensions: &ModuleRuntimeExtensions,
) -> Option<Arc<ReactionSubjectFactoryRegistry>> {
    extensions
        .get::<Arc<ReactionSubjectFactoryRegistry>>()
        .cloned()
}
