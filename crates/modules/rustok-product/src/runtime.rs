use std::sync::Arc;

use rustok_api::PortError;
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;

use crate::{
    CatalogService, ProductCatalogCommandPort, ProductCatalogReadPort,
    ProductCatalogSchemaReadPort, ProductCatalogSchemaService, ProductCatalogSchemaWritePort,
    ProductMediaMaintenancePort, ProductMediaReferencePolicy, ProductMediaReferencePort,
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
    media_reference_port: Option<Arc<dyn ProductMediaReferencePort>>,
    embedded_catalog: Option<CatalogService>,
    media_maintenance_port: Option<Arc<dyn ProductMediaMaintenancePort>>,
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
            media_reference_port: None,
            embedded_catalog: None,
            media_maintenance_port: None,
            profile,
        }
    }

    pub fn in_process(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        let catalog = CatalogService::new(db.clone(), event_bus.clone());
        let mut runtime = Self::new(
            Arc::new(catalog.clone()),
            ProductCatalogCommandProfile::EmbeddedNative,
        )
        .with_schema_write_port(Arc::new(ProductCatalogSchemaService::new(db, event_bus)));
        runtime.media_maintenance_port = Some(Arc::new(catalog.clone()));
        runtime.embedded_catalog = Some(catalog);
        runtime
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

    /// The consumer boundary is installed inside the embedded owner, never in a
    /// decorator in front of receipt replay. An external owner enforces its own policy.
    pub fn with_media_reference_port(
        mut self,
        port: Arc<dyn ProductMediaReferencePort>,
    ) -> Result<Self, PortError> {
        let catalog = self.embedded_catalog.take().ok_or_else(|| {
            PortError::validation(
                "product.media_owner_configuration_invalid",
                "Media retention must be configured inside the Product owner",
            )
        })?;
        let catalog = catalog.with_media_reference_port(port.clone());
        self.command_port = Arc::new(catalog.clone());
        self.media_maintenance_port = Some(Arc::new(catalog.clone()));
        self.media_reference_port = catalog.media_reference_port.clone();
        self.embedded_catalog = Some(catalog);
        Ok(self)
    }

    /// Installs policy inside the receipt-owning CatalogService, never around its port.
    /// A remote Product owner must compose its own Shipping Profile authority.
    pub fn with_shipping_profile_port(
        mut self,
        port: Arc<dyn crate::ProductShippingProfilePort>,
    ) -> Result<Self, PortError> {
        let catalog = self
            .embedded_catalog
            .take()
            .ok_or_else(|| {
                PortError::validation(
                    "product.shipping_owner_configuration_invalid",
                    "Shipping Profile admission must be configured inside the Product owner",
                )
            })?
            .with_shipping_profile_port(port);
        self.command_port = Arc::new(catalog.clone());
        self.media_maintenance_port = Some(Arc::new(catalog.clone()));
        self.embedded_catalog = Some(catalog);
        Ok(self)
    }
    pub fn shipping_profile_port(&self) -> Option<Arc<dyn crate::ProductShippingProfilePort>> {
        self.embedded_catalog
            .as_ref()
            .and_then(|catalog| catalog.shipping_profile_port.clone())
    }

    /// An external owner may publish its own maintenance adapter; the host must
    /// not substitute local persistence for that owner's capability.
    pub fn with_media_maintenance_port(
        mut self,
        port: Arc<dyn ProductMediaMaintenancePort>,
    ) -> Self {
        self.media_maintenance_port = Some(port);
        self
    }

    pub fn media_maintenance_port(&self) -> Option<Arc<dyn ProductMediaMaintenancePort>> {
        self.media_maintenance_port.clone()
    }

    pub fn media_reference_port(&self) -> Option<Arc<dyn ProductMediaReferencePort>> {
        self.media_reference_port.clone()
    }

    /// Trusted background delivery on the exact owner snapshot used for commands.
    /// Never combine a host-default database with a separately published Media port.
    /// External Product owners deliver their own obligations; there is no local fallback.
    pub async fn process_media_releases(
        &self,
        batch_limit: u64,
    ) -> Result<crate::ProductMediaReleaseReport, PortError> {
        let catalog = self
            .embedded_catalog
            .as_ref()
            .filter(|_| self.profile == ProductCatalogCommandProfile::EmbeddedNative)
            .ok_or_else(|| {
                PortError::unavailable(
                    "product.media_delivery_owner_unavailable",
                    "Product Media release delivery requires the embedded owner",
                )
            })?;
        catalog
            .process_media_releases(batch_limit)
            .await
            .map_err(crate::media_reference::maintenance_error)
    }

    pub const fn media_reference_policy(&self) -> ProductMediaReferencePolicy {
        if matches!(self.profile, ProductCatalogCommandProfile::External) {
            ProductMediaReferencePolicy::ExternalOwner
        } else if self.media_reference_port.is_some() {
            ProductMediaReferencePolicy::DurableRetention
        } else {
            ProductMediaReferencePolicy::Unavailable
        }
    }

    pub const fn profile(&self) -> ProductCatalogCommandProfile {
        self.profile
    }
}
