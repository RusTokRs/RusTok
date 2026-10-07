use std::sync::Arc;

use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;

use crate::{
    CatalogService, ProductCatalogCommandPort, ProductCatalogReadPort,
    ProductCatalogSchemaReadPort, ProductCatalogSchemaService, ProductCatalogSchemaWritePort,
    ProductMediaAssetReadPort, ProductMediaReferencePolicy, ProductMediaValidatedCommandPort,
    ProductStorefrontHttpReadPort, ProductStorefrontTagReadPort,
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
    storefront_http_read_port: Option<Arc<dyn ProductStorefrontHttpReadPort>>,
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
            storefront_http_read_port: None,
            storefront_tag_read_port: None,
            profile,
        }
    }

    pub fn in_process(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        let catalog = Arc::new(CatalogService::new(db.clone(), event_bus.clone()));
        Self::new(catalog.clone(), ProductCatalogReadProfile::EmbeddedNative)
            .with_schema_read_port(Arc::new(ProductCatalogSchemaService::new(db, event_bus)))
            .with_storefront_http_read_port(catalog.clone())
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

    pub fn with_storefront_http_read_port(
        mut self,
        storefront_http_read_port: Arc<dyn ProductStorefrontHttpReadPort>,
    ) -> Self {
        self.storefront_http_read_port = Some(storefront_http_read_port);
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

    pub fn storefront_http_read_port(&self) -> Option<Arc<dyn ProductStorefrontHttpReadPort>> {
        self.storefront_http_read_port.clone()
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
///
/// Product schema writes are an explicit optional secondary capability. External command profiles
/// therefore fail closed until a host deliberately composes a schema-write provider.
#[derive(Clone)]
pub struct ProductCatalogCommandRuntime {
    command_port: Arc<dyn ProductCatalogCommandPort>,
    schema_write_port: Option<Arc<dyn ProductCatalogSchemaWritePort>>,
    media_asset_read_port: Option<Arc<dyn ProductMediaAssetReadPort>>,
    profile: ProductCatalogCommandProfile,
}

impl ProductCatalogCommandRuntime {
    pub fn new(
        command_port: Arc<dyn ProductCatalogCommandPort>,
        profile: ProductCatalogCommandProfile,
    ) -> Self {
        Self {
            command_port,
            schema_write_port: None,
            media_asset_read_port: None,
            profile,
        }
    }

    pub fn in_process(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self::new(
            Arc::new(CatalogService::new(db.clone(), event_bus.clone())),
            ProductCatalogCommandProfile::EmbeddedNative,
        )
        .with_schema_write_port(Arc::new(ProductCatalogSchemaService::new(db, event_bus)))
    }

    pub fn external(command_port: Arc<dyn ProductCatalogCommandPort>) -> Self {
        Self::new(command_port, ProductCatalogCommandProfile::External)
    }

    pub fn with_schema_write_port(
        mut self,
        schema_write_port: Arc<dyn ProductCatalogSchemaWritePort>,
    ) -> Self {
        self.schema_write_port = Some(schema_write_port);
        self
    }

    pub fn command_port(&self) -> Arc<dyn ProductCatalogCommandPort> {
        self.command_port.clone()
    }

    pub fn schema_write_port(&self) -> Option<Arc<dyn ProductCatalogSchemaWritePort>> {
        self.schema_write_port.clone()
    }

    /// Composes the Product-owned Media asset validation boundary over the current command port.
    ///
    /// Image writes validate the canonical Media UUID before the owner transaction runs, so an
    /// unknown or cross-tenant asset can never reach `product_images`. Hosts call this after a
    /// Media provider is available; without it Product keeps
    /// [`ProductMediaReferencePolicy::OpaqueReferences`].
    pub fn with_media_asset_read_port(
        mut self,
        media_asset_read_port: Arc<dyn ProductMediaAssetReadPort>,
    ) -> Self {
        self.command_port = Arc::new(ProductMediaValidatedCommandPort::new(
            self.command_port.clone(),
            media_asset_read_port.clone(),
        ));
        self.media_asset_read_port = Some(media_asset_read_port);
        self
    }

    pub fn media_asset_read_port(&self) -> Option<Arc<dyn ProductMediaAssetReadPort>> {
        self.media_asset_read_port.clone()
    }

    pub const fn media_reference_policy(&self) -> ProductMediaReferencePolicy {
        if self.media_asset_read_port.is_some() {
            ProductMediaReferencePolicy::ValidatedByMediaOwner
        } else {
            ProductMediaReferencePolicy::OpaqueReferences
        }
    }

    pub const fn profile(&self) -> ProductCatalogCommandProfile {
        self.profile
    }
}
