mod admin_queries;
mod attribute_filters;
mod commands;
pub mod helpers;
mod image_translation;
mod option_translation;
mod option_translation_changes;
mod option_translation_progress;
mod projection;
mod queries;
mod tags;
mod translation;
mod translation_changes;
mod translation_progress;
pub mod types;
mod variant_translation;
mod variant_translation_changes;
mod variant_translation_progress;

pub use image_translation::{
    ProductImageTranslationExactLocaleApply, ProductImageTranslationExactLocaleApplyReceipt,
    ProductImageTranslationExactLocaleError, ProductImageTranslationExactLocaleRecord,
    ProductImageTranslationExactLocaleResult, ProductImageTranslationExactLocaleSnapshot,
};
pub use option_translation::{
    ProductOptionTranslationExactLocaleApply, ProductOptionTranslationExactLocaleApplyReceipt,
    ProductOptionTranslationExactLocaleError, ProductOptionTranslationExactLocaleRecord,
    ProductOptionTranslationExactLocaleResult, ProductOptionTranslationExactLocaleSnapshot,
    ProductOptionTranslationExactLocaleValueApply, ProductOptionTranslationExactLocaleValueRecord,
};
pub(crate) use option_translation_changes::record_product_option_translation_changes_in_tx;
pub use option_translation_changes::{
    MAX_PRODUCT_OPTION_TRANSLATION_CHANGE_PAGE, ProductOptionTranslationChangeLifecycle,
    ProductOptionTranslationChangeRecord,
};
pub(crate) use translation_changes::record_product_translation_change_in_tx;
pub use translation_changes::{
    MAX_PRODUCT_TRANSLATION_CHANGE_PAGE, ProductTranslationChangeLifecycle,
    ProductTranslationChangeRecord,
};
pub use translation::{
    ProductTranslationExactLocaleApply, ProductTranslationExactLocaleApplyReceipt,
    ProductTranslationExactLocaleError, ProductTranslationExactLocaleRecord,
    ProductTranslationExactLocaleResult, ProductTranslationExactLocaleSnapshot,
    ProductTranslationExactResourcePage, ProductTranslationExactResourceSummary,
};
pub use types::{
    AdminProductList, AdminProductListItem, AdminProductListQuery,
    MAX_STOREFRONT_PRODUCT_SEARCH_BYTES, ProductAttributeFilter, ProductTagState,
    StorefrontProductList, StorefrontProductListItem, StorefrontProductListQuery,
    StorefrontProductSortBy, StorefrontProductSortDirection,
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
use rustok_api::PLATFORM_FALLBACK_LOCALE;
use rustok_events::DomainEvent;
use rustok_inventory::{BootstrapService, InitialInventory};
use rustok_outbox::TransactionalEventBus;
use rustok_pricing_persistence::{BootstrapService as PricingBootstrapService, InitialPrice};

use crate::ProductCatalogSchemaService;

use super::write_transaction::{
    ProductWriteTransaction, current_product_operation_id, record_product_operation_result,
};
use helpers::*;

const PRODUCT_SCOPE_VALUE: &str = "product";

fn map_product_unique_violation(
    error: sea_orm::DbErr,
    handle: &str,
    locale: &str,
    sku: Option<&str>,
) -> CommerceError {
    let message = error.to_string();
    if message.contains("uq_product_variants_tenant_sku") {
        return CommerceError::DuplicateSku(sku.unwrap_or_default().to_owned());
    }
    if message.contains("uq_product_translations_tenant_locale_handle") {
        return CommerceError::DuplicateHandle {
            handle: handle.to_owned(),
            locale: locale.to_owned(),
        };
    }
    CommerceError::Database(error)
}

pub struct CatalogService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl CatalogService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    pub(crate) fn database(&self) -> &DatabaseConnection {
        &self.db
    }
}
