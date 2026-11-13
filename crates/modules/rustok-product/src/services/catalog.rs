mod admin_queries;
mod attribute_filters;
mod commands;
pub mod concurrency;
mod facets;
pub mod helpers;
mod image_translation;
mod image_translation_changes;
mod image_translation_progress;
mod projection;
mod queries;
mod storefront_attributes;
mod tags;
mod translation;
mod translation_changes;
mod translation_progress;
pub mod types;
pub use types::{AdminProductListQueryExt, StorefrontProductListQueryExt};
mod variant_translation;
mod variant_translation_changes;
mod variant_translation_progress;

pub(crate) use commands::variant_axes::reconfiguration_error_code;
pub(crate) use concurrency::revision_conflict_of;
pub use facets::{
    MAX_CATALOG_FACET_VALUES, MAX_CATALOG_FACETS, StorefrontCatalogFacet,
    StorefrontCatalogFacetValue,
};
pub use image_translation::{
    ProductImageTranslationExactLocaleApply, ProductImageTranslationExactLocaleApplyReceipt,
    ProductImageTranslationExactLocaleError, ProductImageTranslationExactLocaleRecord,
    ProductImageTranslationExactLocaleResult, ProductImageTranslationExactLocaleSnapshot,
};
pub(crate) use image_translation_changes::record_product_image_translation_changes_in_tx;
pub use image_translation_changes::{
    MAX_PRODUCT_IMAGE_TRANSLATION_CHANGE_PAGE, ProductImageTranslationChangeLifecycle,
    ProductImageTranslationChangeRecord,
};
pub use translation::{
    ProductTranslationExactLocaleApply, ProductTranslationExactLocaleApplyReceipt,
    ProductTranslationExactLocaleError, ProductTranslationExactLocaleRecord,
    ProductTranslationExactLocaleResult, ProductTranslationExactLocaleSnapshot,
    ProductTranslationExactResourcePage, ProductTranslationExactResourceSummary,
};
pub(crate) use translation_changes::record_product_translation_change_in_tx;
pub use translation_changes::{
    MAX_PRODUCT_TRANSLATION_CHANGE_PAGE, ProductTranslationChangeLifecycle,
    ProductTranslationChangeRecord,
};
pub use types::{
    AdminProductList, AdminProductListItem, AdminProductListQuery,
    MAX_STOREFRONT_PRODUCT_SEARCH_BYTES, ProductAttributeFilter, ProductAttributeFilterSelection,
    ProductTagState, StorefrontProductList, StorefrontProductListImage, StorefrontProductListItem,
    StorefrontProductListPrice, StorefrontProductListQuery, StorefrontProductSortBy,
    StorefrontProductSortDirection, group_product_attribute_filters,
};
pub use variant_translation::{
    ProductVariantTranslationExactLocaleApply, ProductVariantTranslationExactLocaleApplyReceipt,
    ProductVariantTranslationExactLocaleError, ProductVariantTranslationExactLocaleRecord,
    ProductVariantTranslationExactLocaleResult, ProductVariantTranslationExactLocaleSnapshot,
};
pub(crate) use variant_translation_changes::record_product_variant_translation_changes_in_tx;
pub use variant_translation_changes::{
    MAX_PRODUCT_VARIANT_TRANSLATION_CHANGE_PAGE, ProductVariantTranslationChangeLifecycle,
    ProductVariantTranslationChangeRecord,
};

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, Statement,
};
use std::collections::{HashMap, HashSet};
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;
use validator::Validate;

use rustok_core::generate_id;

use crate::dto::*;
use crate::entities;
use crate::error::{CommerceError, CommerceResult};
use crate::ports::pricing::{CatalogBasePriceSnapshot, InitialPrice, ProductPricingPort};
use rustok_api::PLATFORM_FALLBACK_LOCALE;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::ProductCatalogSchemaService;

use super::write_transaction::{
    ProductWriteTransaction, current_product_operation_id, record_product_operation_result,
};
use helpers::*;

fn map_product_unique_violation(
    error: sea_orm::DbErr,
    handle: &str,
    locale: &str,
    sku: Option<&str>,
) -> CommerceError {
    let Some(sea_orm::SqlErr::UniqueConstraintViolation(details)) = error.sql_err() else {
        return CommerceError::Database(error);
    };

    let constraint = &details;
    if constraint.contains("product_variants_tenant_sku") {
        return CommerceError::DuplicateSku(sku.unwrap_or_default().to_owned());
    }
    if constraint.contains("product_translations_tenant_locale_handle") {
        return CommerceError::DuplicateHandle {
            handle: handle.to_owned(),
            locale: locale.to_owned(),
        };
    }
    if constraint.contains("product_trans_unique")
        || constraint.contains("product_translations_product_id_locale_key")
    {
        return CommerceError::Validation(format!(
            "A translation for locale `{locale}` already exists for this product"
        ));
    }
    if constraint.contains("product_variants_combination") {
        return CommerceError::Validation(
            "A variant with this axis combination already exists for this product".to_owned(),
        );
    }
    if constraint.contains("product_variants_default") {
        return CommerceError::Validation(
            "A default variant already exists for this product".to_owned(),
        );
    }
    CommerceError::Database(error)
}

#[derive(Clone)]
pub struct CatalogService {
    pub(crate) db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    pub(crate) shipping_profile_port: Option<std::sync::Arc<dyn crate::ProductShippingProfilePort>>,
    pub(crate) media_reference_port: Option<std::sync::Arc<dyn crate::ProductMediaReferencePort>>,
    pub(crate) pricing_port: Option<std::sync::Arc<dyn ProductPricingPort>>,
    pub(crate) inventory_port: Option<std::sync::Arc<dyn crate::ports::ProductInventoryPort>>,
    pub(crate) taxonomy_port: Option<std::sync::Arc<dyn crate::ports::ProductTaxonomyPort>>,
}

impl CatalogService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            db,
            event_bus,
            media_reference_port: None,
            shipping_profile_port: None,
            pricing_port: None,
            inventory_port: None,
            taxonomy_port: None,
        }
    }

    pub fn with_media_reference_port(
        mut self,
        port: std::sync::Arc<dyn crate::ProductMediaReferencePort>,
    ) -> Self {
        self.media_reference_port = Some(crate::media_reference::bound_media_reference_port(port));
        self
    }

    pub fn with_pricing_port(mut self, port: std::sync::Arc<dyn ProductPricingPort>) -> Self {
        self.pricing_port = Some(port);
        self
    }

    pub fn with_inventory_port(
        mut self,
        port: std::sync::Arc<dyn crate::ports::ProductInventoryPort>,
    ) -> Self {
        self.inventory_port = Some(port);
        self
    }

    pub fn with_taxonomy_port(
        mut self,
        port: std::sync::Arc<dyn crate::ports::ProductTaxonomyPort>,
    ) -> Self {
        self.taxonomy_port = Some(port);
        self
    }

    pub fn with_inventory_bootstrap_port(
        self,
        port: std::sync::Arc<dyn crate::ports::ProductInventoryPort>,
    ) -> Self {
        self.with_inventory_port(port)
    }

    pub(crate) fn inventory(&self) -> std::sync::Arc<dyn crate::ports::ProductInventoryPort> {
        self.inventory_port
            .clone()
            .unwrap_or_else(|| std::sync::Arc::new(crate::ports::NoopProductInventoryPort))
    }

    pub(crate) fn taxonomy(&self) -> std::sync::Arc<dyn crate::ports::ProductTaxonomyPort> {
        self.taxonomy_port
            .clone()
            .unwrap_or_else(|| std::sync::Arc::new(crate::ports::NoopProductTaxonomyPort))
    }

    pub(crate) fn database(&self) -> &DatabaseConnection {
        &self.db
    }
}
