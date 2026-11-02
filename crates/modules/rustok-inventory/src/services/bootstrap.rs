mod deletion;

pub use deletion::{CatalogDeletionBlocker, CatalogDeletionError, MAX_CATALOG_DELETE_VARIANTS};

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
    Statement,
};
use std::collections::HashMap;
use uuid::Uuid;

use rustok_api::PLATFORM_FALLBACK_LOCALE;
use rustok_commerce_foundation::entities;
use rustok_core::generate_id;

use crate::translation_changes::{
    StockLocationTranslationChangeLifecycle, record_stock_location_translation_change_in_tx,
};

use super::stock_location_translation::resource_revision as stock_location_translation_resource_revision;

/// Input for creating inventory state for a newly persisted product variant.
#[derive(Debug, Clone)]
pub struct InitialInventory {
    pub variant_id: Uuid,
    pub sku: Option<String>,
    pub available_quantity: i32,
}

/// Owner-owned, transaction-aware inventory bootstrap operations.
///
/// Product creation uses this native contract because the variant and its first
/// inventory records must commit or roll back together. It is intentionally not
/// a transport surface; no GraphQL or REST bootstrap contract exists yet.
pub struct BootstrapService;

impl BootstrapService {
    pub async fn ensure_default_location_in_tx<C>(
        conn: &C,
        tenant_id: Uuid,
    ) -> Result<entities::stock_location::Model, sea_orm::DbErr>
    where
        C: ConnectionTrait,
    {
        if tenant_id.is_nil() {
            return Err(sea_orm::DbErr::Custom(
                "tenant_id cannot be nil".to_string(),
            ));
        }
        if let Some(location) = Self::find_default_location_in_tx(conn, tenant_id).await? {
            return Ok(location);
        }
        if conn.get_database_backend() == sea_orm::DatabaseBackend::Postgres {
            let lock_key = format!("inventory:stock_location_bootstrap:{tenant_id}");
            conn.execute_raw(Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Postgres,
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
                [lock_key.into()],
            ))
            .await?;
            if let Some(location) = Self::find_default_location_in_tx(conn, tenant_id).await? {
                return Ok(location);
            }
        }

        let now = Utc::now();
        let location = entities::stock_location::ActiveModel {
            id: Set(generate_id()),
            tenant_id: Set(tenant_id),
            code: Set(Some("default".to_owned())),
            address_line1: Set(None),
            address_line2: Set(None),
            city: Set(None),
            province: Set(None),
            postal_code: Set(None),
            country_code: Set(None),
            phone: Set(None),
            metadata: Set(serde_json::json!({ "source": "inventory_bootstrap" })),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            deleted_at: Set(None),
        }
        .insert(conn)
        .await?;

        let translation = entities::stock_location_translation::ActiveModel {
            id: Set(generate_id()),
            stock_location_id: Set(location.id),
            locale: Set(PLATFORM_FALLBACK_LOCALE.to_owned()),
            name: Set("Default".to_owned()),
        }
        .insert(conn)
        .await?;

        let resource_revision = stock_location_translation_resource_revision(
            &location,
            std::slice::from_ref(&translation),
        );
        record_stock_location_translation_change_in_tx(
            conn,
            tenant_id,
            location.id,
            generate_id(),
            &resource_revision,
            StockLocationTranslationChangeLifecycle::Active,
        )
        .await
        .map_err(|error| match error {
            crate::StockLocationTranslationExactLocaleError::Database(error) => error,
            other => sea_orm::DbErr::Custom(format!(
                "Inventory bootstrap translation change journal write failed: {other}"
            )),
        })?;

        Ok(location)
    }

    pub async fn create_initial_records_in_tx(
        conn: &sea_orm::DatabaseTransaction,
        location: &entities::stock_location::Model,
        actor_id: Uuid,
        input: InitialInventory,
    ) -> Result<(), sea_orm::DbErr> {
        if location.id.is_nil() {
            return Err(sea_orm::DbErr::Custom(
                "location.id cannot be nil".to_string(),
            ));
        }
        if actor_id.is_nil() {
            return Err(sea_orm::DbErr::Custom("actor_id cannot be nil".to_owned()));
        }
        if input.variant_id.is_nil() {
            return Err(sea_orm::DbErr::Custom(
                "variant_id cannot be nil".to_string(),
            ));
        }
        if input.available_quantity < 0 {
            return Err(sea_orm::DbErr::Custom(
                "available_quantity cannot be negative".to_string(),
            ));
        }
        let sku = input.sku.and_then(|sku| {
            let trimmed = sku.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });
        let now = Utc::now();
        let inventory_item = entities::inventory_item::ActiveModel {
            id: Set(generate_id()),
            variant_id: Set(input.variant_id),
            sku: Set(sku),
            requires_shipping: Set(true),
            metadata: Set(serde_json::json!({ "source": "inventory_bootstrap" })),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(conn)
        .await?;

        entities::inventory_level::ActiveModel {
            id: Set(generate_id()),
            inventory_item_id: Set(inventory_item.id),
            location_id: Set(location.id),
            stocked_quantity: Set(input.available_quantity),
            reserved_quantity: Set(0),
            incoming_quantity: Set(0),
            low_stock_threshold: Set(None),
            updated_at: Set(now.into()),
        }
        .insert(conn)
        .await?;

        crate::source_history::record_created(
            conn,
            location.tenant_id,
            input.variant_id,
            Some(actor_id),
        )
        .await?;
        Ok(())
    }

    /// Read-only owner selector shared with quantity planning; creation is a separate operation.
    pub(super) async fn find_default_location_in_tx<C>(
        conn: &C,
        tenant_id: Uuid,
    ) -> Result<Option<entities::stock_location::Model>, sea_orm::DbErr>
    where
        C: ConnectionTrait,
    {
        if let Some(location) = entities::stock_location::Entity::find()
            .filter(entities::stock_location::Column::TenantId.eq(tenant_id))
            .filter(entities::stock_location::Column::Code.eq("default"))
            .filter(entities::stock_location::Column::DeletedAt.is_null())
            .order_by_asc(entities::stock_location::Column::CreatedAt)
            .order_by_asc(entities::stock_location::Column::Id)
            .one(conn)
            .await?
        {
            return Ok(Some(location));
        }
        entities::stock_location::Entity::find()
            .filter(entities::stock_location::Column::TenantId.eq(tenant_id))
            .filter(entities::stock_location::Column::DeletedAt.is_null())
            .order_by_asc(entities::stock_location::Column::CreatedAt)
            .order_by_asc(entities::stock_location::Column::Id)
            .one(conn)
            .await
    }

    /// Caller owns commit/rollback, including canonical stock notification and Product effects.
    pub async fn update_initial_quantity_in_tx(
        txn: &sea_orm::DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        quantity: i32,
    ) -> Result<(), super::QuantityWriteError> {
        if quantity < 0 {
            return Err(super::quantity::QuantityWriteBlocker::InvalidRequest.into());
        }
        super::quantity::write_quantity(
            txn,
            tenant_id,
            actor_id,
            variant_id,
            super::quantity::Change::Set(quantity),
            None,
        )
        .await?;
        Ok(())
    }

    pub async fn load_available_quantities<C>(
        conn: &C,
        variant_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, i32>, sea_orm::DbErr>
    where
        C: ConnectionTrait,
    {
        if variant_ids.is_empty() {
            return Ok(HashMap::new());
        }

        if variant_ids.iter().any(|id| id.is_nil()) {
            return Err(sea_orm::DbErr::Custom(
                "variant_ids must not contain a nil UUID".to_string(),
            ));
        }

        let inventory_items = entities::inventory_item::Entity::find()
            .filter(entities::inventory_item::Column::VariantId.is_in(variant_ids.iter().copied()))
            .all(conn)
            .await?;
        if inventory_items.is_empty() {
            return Ok(HashMap::new());
        }

        let item_to_variant: HashMap<Uuid, Uuid> = inventory_items
            .iter()
            .map(|item| (item.id, item.variant_id))
            .collect();
        let levels = entities::inventory_level::Entity::find()
            .filter(
                entities::inventory_level::Column::InventoryItemId
                    .is_in(item_to_variant.keys().copied()),
            )
            .all(conn)
            .await?;

        super::quantity::available_by_variant(&item_to_variant, &levels)
    }

    /// Remove only proved empty bookkeeping; never purge stock or retained history.
    /// The caller must propagate errors and roll back the enclosing transaction.
    pub async fn delete_records_for_variants_in_tx(
        txn: &sea_orm::DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_ids: &[Uuid],
    ) -> Result<(), CatalogDeletionError> {
        deletion::delete_records(txn, tenant_id, actor_id, variant_ids).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{Database, TransactionTrait};

    #[test]
    fn initial_inventory_debug_derives_correctly() {
        let input = InitialInventory {
            variant_id: Uuid::new_v4(),
            sku: Some("SKU-123".to_string()),
            available_quantity: 10,
        };
        let formatted = format!("{input:?}");
        assert!(formatted.contains("SKU-123"));
        assert!(formatted.contains("10"));
    }

    #[tokio::test]
    async fn test_bootstrap_service_nil_location_guard() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let loc = entities::stock_location::Model {
            id: Uuid::nil(),
            tenant_id: Uuid::new_v4(),
            code: Some("default".to_string()),
            address_line1: None,
            address_line2: None,
            city: None,
            province: None,
            postal_code: None,
            country_code: None,
            phone: None,
            metadata: serde_json::json!({}),
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
            deleted_at: None,
        };
        let input = InitialInventory {
            variant_id: Uuid::new_v4(),
            sku: None,
            available_quantity: 10,
        };
        let err = BootstrapService::create_initial_records_in_tx(
            &db.begin().await.unwrap(),
            &loc,
            Uuid::new_v4(),
            input,
        )
        .await
        .expect_err("nil location.id must be rejected");
        assert!(err.to_string().contains("location.id cannot be nil"));
    }

    #[tokio::test]
    async fn test_bootstrap_service_nil_variant_and_negative_quantity_guards() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let loc = entities::stock_location::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            code: Some("default".to_string()),
            address_line1: None,
            address_line2: None,
            city: None,
            province: None,
            postal_code: None,
            country_code: None,
            phone: None,
            metadata: serde_json::json!({}),
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
            deleted_at: None,
        };
        let nil_variant_input = InitialInventory {
            variant_id: Uuid::nil(),
            sku: None,
            available_quantity: 10,
        };
        let err = BootstrapService::create_initial_records_in_tx(
            &db.begin().await.unwrap(),
            &loc,
            Uuid::new_v4(),
            nil_variant_input,
        )
        .await
        .expect_err("nil variant_id must be rejected");
        assert!(err.to_string().contains("variant_id cannot be nil"));

        let negative_qty_input = InitialInventory {
            variant_id: Uuid::new_v4(),
            sku: None,
            available_quantity: -5,
        };
        let err = BootstrapService::create_initial_records_in_tx(
            &db.begin().await.unwrap(),
            &loc,
            Uuid::new_v4(),
            negative_qty_input,
        )
        .await
        .expect_err("negative available_quantity must be rejected");
        assert!(
            err.to_string()
                .contains("available_quantity cannot be negative")
        );
    }

    #[tokio::test]
    async fn initial_inventory_requires_current_actor_before_any_bookkeeping_write() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let location = entities::stock_location::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            code: Some("default".into()),
            address_line1: None,
            address_line2: None,
            city: None,
            province: None,
            postal_code: None,
            country_code: None,
            phone: None,
            metadata: serde_json::json!({}),
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
            deleted_at: None,
        };
        let txn = db.begin().await.unwrap();
        let error = BootstrapService::create_initial_records_in_tx(
            &txn,
            &location,
            Uuid::nil(),
            InitialInventory {
                variant_id: Uuid::new_v4(),
                sku: None,
                available_quantity: 0,
            },
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("actor_id cannot be nil"));
        txn.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn test_bootstrap_service_update_initial_quantity_guards() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let txn = db.begin().await.unwrap();
        let tenant = Uuid::new_v4();
        let actor = Uuid::new_v4();
        for (variant, quantity) in [(Uuid::nil(), 5), (Uuid::new_v4(), -1)] {
            let error = BootstrapService::update_initial_quantity_in_tx(
                &txn, tenant, actor, variant, quantity,
            )
            .await
            .expect_err("invalid quantity input must fail before querying missing tables");
            assert!(matches!(
                error,
                super::super::quantity::QuantityWriteError::Rejected(
                    super::super::quantity::QuantityWriteBlocker::InvalidRequest
                )
            ));
        }
        txn.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn test_bootstrap_service_nil_slice_guards() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let err = BootstrapService::load_available_quantities(&db, &[Uuid::nil()])
            .await
            .expect_err("nil variant_id in slice must be rejected");
        assert!(
            err.to_string()
                .contains("variant_ids must not contain a nil UUID")
        );

        let txn = db.begin().await.unwrap();
        let err = BootstrapService::delete_records_for_variants_in_tx(
            &txn,
            Uuid::new_v4(),
            Uuid::new_v4(),
            &[Uuid::new_v4(), Uuid::nil()],
        )
        .await
        .expect_err("nil variant_id in delete slice must be rejected");
        assert!(matches!(
            err,
            CatalogDeletionError::Rejected(CatalogDeletionBlocker::InvalidRequest)
        ));
        txn.rollback().await.unwrap();
    }
}
