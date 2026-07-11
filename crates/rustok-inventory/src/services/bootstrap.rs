use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use std::collections::HashMap;
use uuid::Uuid;

use rustok_api::PLATFORM_FALLBACK_LOCALE;
use rustok_commerce_foundation::{
    entities,
    error::{CommerceError, CommerceResult},
};
use rustok_core::generate_id;

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
    ) -> CommerceResult<entities::stock_location::Model>
    where
        C: ConnectionTrait,
    {
        if let Some(location) = entities::stock_location::Entity::find()
            .filter(entities::stock_location::Column::TenantId.eq(tenant_id))
            .filter(entities::stock_location::Column::DeletedAt.is_null())
            .one(conn)
            .await?
        {
            return Ok(location);
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
        .await
        .map_err(CommerceError::from)?;

        entities::stock_location_translation::ActiveModel {
            id: Set(generate_id()),
            stock_location_id: Set(location.id),
            locale: Set(PLATFORM_FALLBACK_LOCALE.to_owned()),
            name: Set("Default".to_owned()),
        }
        .insert(conn)
        .await
        .map_err(CommerceError::from)?;

        Ok(location)
    }

    pub async fn create_initial_records_in_tx<C>(
        conn: &C,
        location: &entities::stock_location::Model,
        input: InitialInventory,
    ) -> CommerceResult<()>
    where
        C: ConnectionTrait,
    {
        let now = Utc::now();
        let inventory_item = entities::inventory_item::ActiveModel {
            id: Set(generate_id()),
            variant_id: Set(input.variant_id),
            sku: Set(input.sku),
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

        Ok(())
    }

    pub async fn load_available_quantities<C>(
        conn: &C,
        variant_ids: &[Uuid],
    ) -> CommerceResult<HashMap<Uuid, i32>>
    where
        C: ConnectionTrait,
    {
        if variant_ids.is_empty() {
            return Ok(HashMap::new());
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

        let mut available_by_variant = HashMap::new();
        for level in levels {
            if let Some(variant_id) = item_to_variant.get(&level.inventory_item_id) {
                *available_by_variant.entry(*variant_id).or_insert(0) +=
                    level.stocked_quantity - level.reserved_quantity;
            }
        }

        Ok(available_by_variant)
    }

    pub async fn delete_records_for_variants_in_tx<C>(
        conn: &C,
        variant_ids: &[Uuid],
    ) -> CommerceResult<()>
    where
        C: ConnectionTrait,
    {
        if variant_ids.is_empty() {
            return Ok(());
        }

        let inventory_item_ids: Vec<Uuid> = entities::inventory_item::Entity::find()
            .filter(entities::inventory_item::Column::VariantId.is_in(variant_ids.iter().copied()))
            .all(conn)
            .await?
            .into_iter()
            .map(|item| item.id)
            .collect();
        if inventory_item_ids.is_empty() {
            return Ok(());
        }

        entities::reservation_item::Entity::delete_many()
            .filter(
                entities::reservation_item::Column::InventoryItemId
                    .is_in(inventory_item_ids.clone()),
            )
            .exec(conn)
            .await?;
        entities::inventory_level::Entity::delete_many()
            .filter(
                entities::inventory_level::Column::InventoryItemId
                    .is_in(inventory_item_ids.clone()),
            )
            .exec(conn)
            .await?;
        entities::inventory_item::Entity::delete_many()
            .filter(entities::inventory_item::Column::Id.is_in(inventory_item_ids))
            .exec(conn)
            .await?;

        Ok(())
    }
}
