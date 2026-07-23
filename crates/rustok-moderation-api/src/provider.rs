use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{HostRuntimeContext, PortContext, PortError};
use rustok_core::ModuleRuntimeExtensions;
use thiserror::Error;

use crate::{
    ApplyModerationDecisionCommand, ModerationDecisionApplication, ModerationSubjectKind,
};

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ModerationSubjectAdapterKey {
    pub module: String,
    pub kind: ModerationSubjectKind,
}

impl ModerationSubjectAdapterKey {
    pub fn new(
        module: impl Into<String>,
        kind: ModerationSubjectKind,
    ) -> Result<Self, ModerationSubjectAdapterRegistryError> {
        let module = module.into();
        let valid = !module.is_empty()
            && module.len() <= 100
            && module.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'_' | b'-')
            })
            && !module.starts_with('_')
            && !module.starts_with('-')
            && !module.ends_with('_')
            && !module.ends_with('-');
        if !valid {
            return Err(ModerationSubjectAdapterRegistryError::InvalidModule);
        }
        Ok(Self { module, kind })
    }
}

#[async_trait]
pub trait ModerationSubjectCommandPort: Send + Sync {
    async fn apply_moderation_decision(
        &self,
        context: PortContext,
        command: ApplyModerationDecisionCommand,
    ) -> Result<ModerationDecisionApplication, PortError>;
}

pub trait ModerationSubjectAdapterFactory: Send + Sync {
    fn key(&self) -> ModerationSubjectAdapterKey;

    fn build(
        &self,
        host: &HostRuntimeContext,
    ) -> Result<Arc<dyn ModerationSubjectCommandPort>, ModerationSubjectAdapterBuildError>;
}

#[derive(Debug, Error, Clone, Eq, PartialEq)]
pub enum ModerationSubjectAdapterBuildError {
    #[error("moderation subject adapter capability is unavailable")]
    CapabilityUnavailable { retryable: bool },
    #[error("moderation subject adapter configuration is invalid")]
    InvalidConfiguration,
    #[error("moderation subject adapter failed to initialize")]
    Internal { retryable: bool },
}

impl ModerationSubjectAdapterBuildError {
    pub const fn is_retryable(&self) -> bool {
        match self {
            Self::CapabilityUnavailable { retryable } | Self::Internal { retryable } => *retryable,
            Self::InvalidConfiguration => false,
        }
    }
}

#[derive(Debug, Error, Clone, Eq, PartialEq)]
pub enum ModerationSubjectAdapterRegistryError {
    #[error("moderation subject adapter module is invalid")]
    InvalidModule,
    #[error("moderation subject adapter `{module}/{kind}` is already registered")]
    DuplicateAdapter { module: String, kind: &'static str },
    #[error("moderation subject adapter factory `{module}/{kind}` is already registered")]
    DuplicateFactory { module: String, kind: &'static str },
    #[error("moderation subject adapter factory key does not match the built adapter key")]
    FactoryKeyMismatch,
    #[error("moderation subject adapter factory failed for `{module}/{kind}`: {error}")]
    FactoryBuild {
        module: String,
        kind: &'static str,
        error: ModerationSubjectAdapterBuildError,
    },
}

#[derive(Clone, Default)]
pub struct ModerationSubjectAdapterRegistry {
    adapters: BTreeMap<ModerationSubjectAdapterKey, Arc<dyn ModerationSubjectCommandPort>>,
}

impl ModerationSubjectAdapterRegistry {
    pub fn register_arc(
        &mut self,
        key: ModerationSubjectAdapterKey,
        adapter: Arc<dyn ModerationSubjectCommandPort>,
    ) -> Result<(), ModerationSubjectAdapterRegistryError> {
        if self.adapters.contains_key(&key) {
            return Err(ModerationSubjectAdapterRegistryError::DuplicateAdapter {
                module: key.module,
                kind: key.kind.as_str(),
            });
        }
        self.adapters.insert(key, adapter);
        Ok(())
    }

    pub fn get(
        &self,
        module: &str,
        kind: ModerationSubjectKind,
    ) -> Option<Arc<dyn ModerationSubjectCommandPort>> {
        let key = ModerationSubjectAdapterKey::new(module, kind).ok()?;
        self.adapters.get(&key).cloned()
    }

    pub fn contains(&self, module: &str, kind: ModerationSubjectKind) -> bool {
        self.get(module, kind).is_some()
    }

    pub fn len(&self) -> usize {
        self.adapters.len()
    }

    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }
}

#[derive(Clone, Default)]
pub struct ModerationSubjectAdapterFactoryRegistry {
    factories: BTreeMap<ModerationSubjectAdapterKey, Arc<dyn ModerationSubjectAdapterFactory>>,
}

impl ModerationSubjectAdapterFactoryRegistry {
    pub fn register<F>(&mut self, factory: F) -> Result<(), ModerationSubjectAdapterRegistryError>
    where
        F: ModerationSubjectAdapterFactory + 'static,
    {
        let key = factory.key();
        if self.factories.contains_key(&key) {
            return Err(ModerationSubjectAdapterRegistryError::DuplicateFactory {
                module: key.module,
                kind: key.kind.as_str(),
            });
        }
        self.factories.insert(key, Arc::new(factory));
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.factories.len()
    }

    pub fn is_empty(&self) -> bool {
        self.factories.is_empty()
    }
}

pub fn register_moderation_subject_adapter_factory<F>(
    extensions: &mut ModuleRuntimeExtensions,
    factory: F,
) -> Result<(), ModerationSubjectAdapterRegistryError>
where
    F: ModerationSubjectAdapterFactory + 'static,
{
    let registry = extensions
        .get_or_insert_with::<Arc<ModerationSubjectAdapterFactoryRegistry>, _>(|| {
            Arc::new(ModerationSubjectAdapterFactoryRegistry::default())
        });
    Arc::make_mut(registry).register(factory)
}

pub fn materialize_moderation_subject_adapter_registry(
    extensions: &mut ModuleRuntimeExtensions,
    host: &HostRuntimeContext,
) -> Result<Arc<ModerationSubjectAdapterRegistry>, ModerationSubjectAdapterRegistryError> {
    let factories = extensions
        .get::<Arc<ModerationSubjectAdapterFactoryRegistry>>()
        .cloned()
        .unwrap_or_else(|| Arc::new(ModerationSubjectAdapterFactoryRegistry::default()));
    let mut adapters = ModerationSubjectAdapterRegistry::default();
    for (declared, factory) in &factories.factories {
        let built = factory.build(host).map_err(|error| {
            ModerationSubjectAdapterRegistryError::FactoryBuild {
                module: declared.module.clone(),
                kind: declared.kind.as_str(),
                error,
            }
        })?;
        if factory.key() != *declared {
            return Err(ModerationSubjectAdapterRegistryError::FactoryKeyMismatch);
        }
        adapters.register_arc(declared.clone(), built)?;
    }
    let adapters = Arc::new(adapters);
    extensions.insert(adapters.clone());
    Ok(adapters)
}

pub fn moderation_subject_adapter_registry_from_extensions(
    extensions: &ModuleRuntimeExtensions,
) -> Option<Arc<ModerationSubjectAdapterRegistry>> {
    extensions
        .get::<Arc<ModerationSubjectAdapterRegistry>>()
        .cloned()
}
