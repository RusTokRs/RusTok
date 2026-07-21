use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_core::ModuleRuntimeExtensions;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::keys::{NotificationAudienceCursor, NotificationSourceSlug, NotificationTypeKey};
use crate::model::{
    NotificationAudiencePage, NotificationOpenAuthorization, NotificationSemanticDescriptor,
    NotificationSourceEventRef, NotificationTargetRef, MAX_NOTIFICATION_AUDIENCE_PAGE_SIZE,
};

pub type NotificationProviderResult<T> = Result<T, NotificationProviderError>;

#[derive(Debug, Error, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum NotificationProviderError {
    #[error("notification source capability is unavailable")]
    CapabilityUnavailable { retryable: bool },
    #[error("notification source event is invalid")]
    InvalidEvent,
    #[error("notification source provider rejected the request")]
    Rejected,
    #[error("notification source provider failed")]
    Internal { retryable: bool },
}

impl NotificationProviderError {
    pub const fn is_retryable(self) -> bool {
        match self {
            Self::CapabilityUnavailable { retryable } | Self::Internal { retryable } => retryable,
            Self::InvalidEvent | Self::Rejected => false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DescribeNotificationRequest {
    pub event: NotificationSourceEventRef,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResolveNotificationAudienceRequest {
    pub event: NotificationSourceEventRef,
    pub descriptor: NotificationSemanticDescriptor,
    pub cursor: Option<NotificationAudienceCursor>,
    pub limit: u16,
}

impl ResolveNotificationAudienceRequest {
    pub fn bounded_limit(&self) -> usize {
        usize::from(self.limit).min(MAX_NOTIFICATION_AUDIENCE_PAGE_SIZE)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AuthorizeNotificationTargetRequest {
    pub tenant_id: Uuid,
    pub recipient_id: Uuid,
    pub target: NotificationTargetRef,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationSourceRegistryEntry {
    pub slug: NotificationSourceSlug,
    pub display_name: String,
    pub supported_types: Vec<NotificationTypeKey>,
}

#[async_trait]
pub trait NotificationSourceProvider: Send + Sync {
    fn slug(&self) -> NotificationSourceSlug;

    fn display_name(&self) -> &'static str;

    fn supported_types(&self) -> Vec<NotificationTypeKey>;

    async fn describe_event(
        &self,
        request: DescribeNotificationRequest,
    ) -> NotificationProviderResult<Option<NotificationSemanticDescriptor>>;

    async fn resolve_audience(
        &self,
        request: ResolveNotificationAudienceRequest,
    ) -> NotificationProviderResult<NotificationAudiencePage>;

    async fn authorize_target_open(
        &self,
        request: AuthorizeNotificationTargetRequest,
    ) -> NotificationProviderResult<NotificationOpenAuthorization>;
}

#[derive(Debug, Error, Clone, Eq, PartialEq)]
pub enum NotificationSourceRegistryError {
    #[error("notification source `{0}` is already registered")]
    DuplicateSource(NotificationSourceSlug),
}

#[derive(Clone, Default)]
pub struct NotificationSourceRegistry {
    providers: BTreeMap<NotificationSourceSlug, Arc<dyn NotificationSourceProvider>>,
}

impl NotificationSourceRegistry {
    pub fn register<P>(&mut self, provider: P) -> Result<(), NotificationSourceRegistryError>
    where
        P: NotificationSourceProvider + 'static,
    {
        self.register_arc(Arc::new(provider))
    }

    pub fn register_arc(
        &mut self,
        provider: Arc<dyn NotificationSourceProvider>,
    ) -> Result<(), NotificationSourceRegistryError> {
        let slug = provider.slug();
        if self.providers.contains_key(&slug) {
            return Err(NotificationSourceRegistryError::DuplicateSource(slug));
        }
        self.providers.insert(slug, provider);
        Ok(())
    }

    pub fn get(
        &self,
        slug: &NotificationSourceSlug,
    ) -> Option<Arc<dyn NotificationSourceProvider>> {
        self.providers.get(slug).cloned()
    }

    pub fn get_by_str(&self, slug: &str) -> Option<Arc<dyn NotificationSourceProvider>> {
        NotificationSourceSlug::new(slug)
            .ok()
            .and_then(|slug| self.get(&slug))
    }

    pub fn entries(&self) -> Vec<NotificationSourceRegistryEntry> {
        self.providers
            .iter()
            .map(|(slug, provider)| {
                let mut supported_types = provider.supported_types();
                supported_types.sort();
                supported_types.dedup();
                NotificationSourceRegistryEntry {
                    slug: slug.clone(),
                    display_name: provider.display_name().to_string(),
                    supported_types,
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

pub fn ensure_notification_source_registry(
    extensions: &mut ModuleRuntimeExtensions,
) -> Arc<NotificationSourceRegistry> {
    extensions
        .get_or_insert_with::<Arc<NotificationSourceRegistry>, _>(|| {
            Arc::new(NotificationSourceRegistry::default())
        })
        .clone()
}

pub fn register_notification_source_provider<P>(
    extensions: &mut ModuleRuntimeExtensions,
    provider: P,
) -> Result<(), NotificationSourceRegistryError>
where
    P: NotificationSourceProvider + 'static,
{
    let registry = extensions
        .get_or_insert_with::<Arc<NotificationSourceRegistry>, _>(|| {
            Arc::new(NotificationSourceRegistry::default())
        });
    Arc::make_mut(registry).register(provider)
}

pub fn notification_source_registry_from_extensions(
    extensions: &ModuleRuntimeExtensions,
) -> Option<Arc<NotificationSourceRegistry>> {
    extensions
        .get::<Arc<NotificationSourceRegistry>>()
        .cloned()
}
