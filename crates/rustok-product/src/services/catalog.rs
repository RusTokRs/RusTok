mod admin_queries;
mod attribute_filters;
mod commands;
pub mod helpers;
mod projection;
mod queries;
mod storefront_localization;
mod tags;
pub mod types;

pub use types::{
    AdminProductList, AdminProductListItem, AdminProductListQuery,
    MAX_STOREFRONT_PRODUCT_SEARCH_BYTES, ProductAttributeFilter, ProductTagState,
    StorefrontProductList, StorefrontProductListItem, StorefrontProductListQuery,
    StorefrontProductSortBy, StorefrontProductSortDirection,
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

use super::write_transaction::ProductWriteTransaction;
use helpers::*;
use storefront_localization::localize_product_response;

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
