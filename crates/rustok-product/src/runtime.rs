use std::sync::Arc;

use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;

use crate::{
    CatalogService, ProductCatalogCommandPort, ProductCatalogReadPort, ProductCatalogSchemaReadPort,
    ProductCatalogSchemaService, ProductStorefrontTagReadPort,
};

/// Host-selected execution profile for the Product catalog read boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductCatalogReadProfile {
    EmbeddedNative,
    External,
}

impl ProductCatalogReadProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EmbeddedNative => "embedded_native",
            Self::External => "external",
        }
    }
}

/// Canonical host-composed Product catalog read capability.
///
/// Consumers receive this wrapper rather than constructing `CatalogService` directly. A host can
/// therefore replace the embedded provider with a remote adapter without changing consumer code. Optional
/// owner capabilities remain explicit and fail closed when an external profile has not selected them.
#[derive(Clone)]
pub struct ProductCatalogReadRuntime {
    read_port: Arc<dyn ProductCatalogReadPort>,
    schema_read_port: Option<Arc<dyn ProductCatalogSchemaReadPort>>,
    storefront_tag_read_port: Option<Arc<dyn ProductStorefrontTagReadPort>>,
    profile: ProductCatalogReadProfile,
}

impl ProductCatalogReadRuntime {
    pub fn new(
        read_port: Arc<dyn ProductCatalogReadPort>,
        profile: ProductCatalogReadProfile,
    ) -> Self {
        Self {
            read_port,
            schema_read_port: None,
            storefront_tag_read_port: None,
            profile,
        }
    }

    pub fn in_process(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        let catalog = Arc::new(CatalogService::new(db.clone(), event_bus.clone()));
        Self::new(catalog.clone(), ProductCatalogReadProfile::EmbeddedNative)
            .with_schema_read_port(Arc::new(ProductCatalogSchemaService::new(db, event_bus)))
            .with_storefront_tag_read_port(catalog)
    }

    pub fn external(read_port: Arc<dyn ProductCatalogReadPort>) -> Self {
        Self::new(read_port, ProductCatalogReadProfile::External)
    }

    pub fn with_schema_read_port(
        mut self,
        schema_read_port: Arc<dyn ProductCatalogSchemaReadPort>,
    ) -> Self {
        self.schema_read_port = Some(schema_read_port);
        self
    }

    pub fn with_storefront_tag_read_port(
        mut self,
        storefront_tag_read_port: Arc<dyn ProductStorefrontTagReadPort>,
    ) -> Self {
        self.storefront_tag_read_port = Some(storefront_tag_read_port);
        self
    }

    pub fn read_port(&self) -> Arc<dyn ProductCatalogReadPort> {
        self.read_port.clone()
    }

    pub fn schema_read_port(&self) -> Option<Arc<dyn ProductCatalogSchemaReadPort>> {
        self.schema_read_port.clone()
    }

    pub fn storefront_tag_read_port(&self) -> Option<Arc<dyn ProductStorefrontTagReadPort>> {
        self.storefront_tag_read_port.clone()
    }

    pub const fn profile(&self) -> ProductCatalogReadProfile {
        self.profile
    }
}

/// Host-selected execution profile for the Product catalog command boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductCatalogCommandProfile {
    EmbeddedNative,
    External,
}

impl ProductCatalogCommandProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EmbeddedNative => "embedded_native",
            Self::External => "external",
        }
    }
}

/// Canonical host-composed Product catalog lifecycle command capability.
#[derive(Clone)]
pub struct ProductCatalogCommandRuntime {
    command_port: Arc<dyn ProductCatalogCommandPort>,
    profile: ProductCatalogCommandProfile,
}

impl ProductCatalogCommandRuntime {
    pub fn new(
        command_port: Arc<dyn ProductCatalogCommandPort>,
        profile: ProductCatalogCommandProfile,
    ) -> Self {
        Self {
            command_port,
            profile,
        }
    }

    pub fn in_process(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self::new(
            Arc::new(CatalogService::new(db, event_bus)),
            ProductCatalogCommandProfile::EmbeddedNative,
        )
    }

    pub fn external(command_port: Arc<dyn ProductCatalogCommandPort>) -> Self {
        Self::new(command_port, ProductCatalogCommandProfile::External)
    }

    pub fn command_port(&self) -> Arc<dyn ProductCatalogCommandPort> {
        self.command_port.clone()
    }

    pub const fn profile(&self) -> ProductCatalogCommandProfile {
        self.profile
    }
}
