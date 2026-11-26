//! Host binding for record owners whose public visibility is tenant-wide.

use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{PortError, SharedStaticModuleSettingsReader};

/// Current owner visibility. Search supplies a bounded candidate batch after
/// validating request context and matching the registered source and entity kind.
#[async_trait]
pub trait TenantSearchPublication: Send + Sync {
    async fn public_discovery_allowed(&self, tenant_id: Uuid) -> Result<bool, PortError>;

    /// Return only input IDs that are both currently published and allowed by
    /// current tenant policy. Authenticated storefront calls also use this gate.
    async fn filter_published_ids(
        &self,
        tenant_id: Uuid,
        ids: Vec<Uuid>,
    ) -> Result<Vec<Uuid>, PortError>;
}

pub struct TenantSearchPublicationSource {
    pub module_slug: &'static str,
    pub entity_type: &'static str,
    pub build: fn(
        DatabaseConnection,
        SharedStaticModuleSettingsReader,
    ) -> Arc<dyn TenantSearchPublication>,
}

/// Native module contributions, collected before the host publishes its typed
/// settings reader. The Search audience registry validates corpus declarations.
#[derive(Default)]
pub struct TenantSearchPublicationSources(Vec<TenantSearchPublicationSource>);

impl TenantSearchPublicationSources {
    pub fn register(&mut self, source: TenantSearchPublicationSource) {
        self.0.push(source);
    }

    pub fn sources(&self) -> &[TenantSearchPublicationSource] {
        &self.0
    }
}
