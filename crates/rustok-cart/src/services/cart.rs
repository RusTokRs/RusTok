use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait,
};
use std::collections::{BTreeMap, BTreeSet};
use tracing::instrument;
use uuid::Uuid;
use validator::Validate;

use rustok_core::generate_id;

use crate::dto::{
    AddCartLineItemInput, CartDeliveryGroupResponse, CartLineItemResponse, CartResponse,
    CreateCartInput, UpdateCartContextInput,
};
use crate::entities;
use crate::error::{CartError, CartResult};

const STATUS_ACTIVE: &str = "active";
const STATUS_CHECKING_OUT: &str = "checking_out";
const STATUS_COMPLETED: &str = "completed";
const STATUS_ABANDONED: &str = "abandoned";
const DEFAULT_SHIPPING_PROFILE_SLUG: &str = "default";

pub struct CartService {
    db: DatabaseConnection,
}

impl CartService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id))]
    pub async fn create_cart(
        &self,
        tenant_id: Uuid,
        input: CreateCartInput,
    ) -> CartResult<CartResponse> {
        self.create_cart_with_channel(tenant_id, input, None, None)
            .await
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, channel_id = ?channel_id, channel_slug = ?channel_slug))]
    pub async fn create_cart_with_channel(
        &self,
        tenant_id: Uuid,
        input: CreateCartInput,
        channel_id: Option<Uuid>,
        channel_slug: Option<String>,
    ) -> CartResult<CartResponse> {
        input
            .validate()
            .map_err(|error| CartError::Validation(error.to_string()))?;

        let currency_code = input.currency_code.trim().to_ascii_uppercase();
        if currency_code.len() != 3 {
            return Err(CartError::Validation(
                "currency_code must be a 3-letter code".to_string(),
            ));
        }
        let country_code = input
            .country_code
            .as_deref()
            .map(normalize_country_code)
            .transpose()?;
        let locale_code = input
            .locale_code
            .as_deref()
            .map(normalize_locale_code)
            .transpose()?;
        let channel_slug = channel_slug
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let cart_id = generate_id();
        let now = Utc::now();

        entities::cart::ActiveModel {
            id: Set(cart_id),
            tenant_id: Set(tenant_id),
            channel_id: Set(channel_id),
            channel_slug: Set(channel_slug),
            customer_id: Set(input.customer_id),
            email: Set(input.email),
            region_id: Set(input.region_id),
            country_code: Set(country_code),
            locale_code: Set(locale_code),
            selected_shipping_option_id: Set(input.selected_shipping_option_id),
            status: Set(STATUS_ACTIVE.to_string()),
            currency_code: Set(currency_code),
            total_amount: Set(Decimal::ZERO),
            metadata: Set(input.metadata),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            completed_at: Set(None),
        }
        .insert(&self.db)
        .await?;

        self.get_cart(tenant_id, cart_id).await
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id, cart_id = %cart_id))]
    pub async fn get_cart(&self, tenant_id: Uuid, cart_id: Uuid) -> CartResult<CartResponse> {
        let cart = self.load_cart(tenant_id, cart_id).await?;
        self.build_response(cart).await
    }

    pub async fn add_line_item(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        input: AddCartLineItemInput,
    ) -> CartResult<CartResponse> {
        input
            .validate()
            .map_err(|error| CartError::Validation(error.to_string()))?;
        if input.unit_price < Decimal::ZERO {
            return Err(CartError::Validation(
                "unit_price cannot be negative".to_string(),
            ));
        }

        let txn = self.db.begin().await?;
        let cart = self.load_cart_in_tx(&txn, tenant_id, cart_id).await?;
        ensure_active(&cart.status, "add_line_item")?;
        let now = Utc::now();

        entities::cart_line_item::ActiveModel {
            id: Set(generate_id()),
            cart_id: Set(cart_id),
            product_id: Set(input.product_id),
            variant_id: Set(input.variant_id),
            shipping_profile_slug: Set(normalize_shipping_profile_slug(
                input.shipping_profile_slug.as_deref(),
            )),
            sku: Set(input.sku),
            title: Set(input.title),
            quantity: Set(input.quantity),
            unit_price: Set(input.unit_price),
            total_price: Set(input.unit_price * Decimal::from(input.quantity)),
            currency_code: Set(cart.currency_code.clone()),
            metadata: Set(input.metadata),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        self.recalculate_totals(&txn, cart).await?;
        self.reconcile_cart_shipping_state(&txn, cart_id).await?;
        txn.commit().await?;
        self.get_cart(tenant_id, cart_id).await
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, cart_id = %cart_id))]
    pub async fn update_context(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        input: UpdateCartContextInput,
    ) -> CartResult<CartResponse> {
        input
            .validate()
            .map_err(|error| CartError::Validation(error.to_string()))?;

        let txn = self.db.begin().await?;
        let cart = self.load_cart_in_tx(&txn, tenant_id, cart_id).await?;
        ensure_active(&cart.status, "update_context")?;
        let shipping_patch_input = input.clone();

        let country_code = input
            .country_code
            .as_deref()
            .map(normalize_country_code)
            .transpose()?;
        let locale_code = input
            .locale_code
            .as_deref()
            .map(normalize_locale_code)
            .transpose()?;

        let mut active: entities::cart::ActiveModel = cart.clone().into();
        active.email = Set(input.email);
        active.region_id = Set(input.region_id);
        active.country_code = Set(country_code);
        active.locale_code = Set(locale_code);
        active.updated_at = Set(Utc::now().into());
        active.update(&txn).await?;
        self.apply_shipping_selection_patch(&txn, &cart, &shipping_patch_input)
            .await?;

        txn.commit().await?;
        self.get_cart(tenant_id, cart_id).await
    }

    pub async fn update_line_item_quantity(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        line_item_id: Uuid,
        quantity: i32,
    ) -> CartResult<CartResponse> {
        if quantity < 1 {
            return Err(CartError::Validation(
                "quantity must be at least 1".to_string(),
            ));
        }

        let txn = self.db.begin().await?;
        let cart = self.load_cart_in_tx(&txn, tenant_id, cart_id).await?;
        ensure_active(&cart.status, "update_line_item_quantity")?;

        let line_item = entities::cart_line_item::Entity::find_by_id(line_item_id)
            .filter(entities::cart_line_item::Column::CartId.eq(cart_id))
            .one(&txn)
            .await?
            .ok_or(CartError::CartLineItemNotFound(line_item_id))?;

        let mut active: entities::cart_line_item::ActiveModel = line_item.into();
        let now = Utc::now();
        let unit_price = active.unit_price.clone().take().unwrap_or(Decimal::ZERO);
        active.quantity = Set(quantity);
        active.total_price = Set(unit_price * Decimal::from(quantity));
        active.updated_at = Set(now.into());
        active.update(&txn).await?;

        self.recalculate_totals(&txn, cart).await?;
        self.reconcile_cart_shipping_state(&txn, cart_id).await?;
        txn.commit().await?;
        self.get_cart(tenant_id, cart_id).await
    }

    pub async fn remove_line_item(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        line_item_id: Uuid,
    ) -> CartResult<CartResponse> {
        let txn = self.db.begin().await?;
        let cart = self.load_cart_in_tx(&txn, tenant_id, cart_id).await?;
        ensure_active(&cart.status, "remove_line_item")?;

        let line_item = entities::cart_line_item::Entity::find_by_id(line_item_id)
            .filter(entities::cart_line_item::Column::CartId.eq(cart_id))
            .one(&txn)
            .await?
            .ok_or(CartError::CartLineItemNotFound(line_item_id))?;
        let active: entities::cart_line_item::ActiveModel = line_item.into();
        active.delete(&txn).await?;

        self.recalculate_totals(&txn, cart).await?;
        self.reconcile_cart_shipping_state(&txn, cart_id).await?;
        txn.commit().await?;
        self.get_cart(tenant_id, cart_id).await
    }

    pub async fn complete_cart(&self, tenant_id: Uuid, cart_id: Uuid) -> CartResult<CartResponse> {
        self.transition_cart_from_any(
            tenant_id,
            cart_id,
            &[STATUS_ACTIVE, STATUS_CHECKING_OUT],
            STATUS_COMPLETED,
            true,
        )
        .await
    }

    pub async fn abandon_cart(&self, tenant_id: Uuid, cart_id: Uuid) -> CartResult<CartResponse> {
        self.transition_cart(tenant_id, cart_id, STATUS_ACTIVE, STATUS_ABANDONED, false)
            .await
    }

    pub async fn begin_checkout(&self, tenant_id: Uuid, cart_id: Uuid) -> CartResult<CartResponse> {
        self.transition_cart(
            tenant_id,
            cart_id,
            STATUS_ACTIVE,
            STATUS_CHECKING_OUT,
            false,
        )
        .await
    }

    pub async fn release_checkout(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
    ) -> CartResult<CartResponse> {
        self.transition_cart(
            tenant_id,
            cart_id,
            STATUS_CHECKING_OUT,
            STATUS_ACTIVE,
            false,
        )
        .await
    }

    async fn transition_cart(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        expected_from: &str,
        next_status: &str,
        mark_completed: bool,
    ) -> CartResult<CartResponse> {
        let txn = self.db.begin().await?;
        let cart = self.load_cart_in_tx(&txn, tenant_id, cart_id).await?;
        if cart.status != expected_from {
            return Err(CartError::InvalidTransition {
                from: cart.status,
                to: next_status.to_string(),
            });
        }

        let mut active: entities::cart::ActiveModel = cart.into();
        let now = Utc::now();
        active.status = Set(next_status.to_string());
        active.updated_at = Set(now.into());
        active.completed_at = Set(if mark_completed {
            Some(now.into())
        } else {
            None
        });
        active.update(&txn).await?;
        txn.commit().await?;
        self.get_cart(tenant_id, cart_id).await
    }

    async fn transition_cart_from_any(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        expected_from: &[&str],
        next_status: &str,
        mark_completed: bool,
    ) -> CartResult<CartResponse> {
        let txn = self.db.begin().await?;
        let cart = self.load_cart_in_tx(&txn, tenant_id, cart_id).await?;
        if !expected_from.contains(&cart.status.as_str()) {
            return Err(CartError::InvalidTransition {
                from: cart.status,
                to: next_status.to_string(),
            });
        }

        let mut active: entities::cart::ActiveModel = cart.into();
        let now = Utc::now();
        active.status = Set(next_status.to_string());
        active.updated_at = Set(now.into());
        active.completed_at = Set(if mark_completed {
            Some(now.into())
        } else {
            None
        });
        active.update(&txn).await?;
        txn.commit().await?;
        self.get_cart(tenant_id, cart_id).await
    }

    async fn recalculate_totals<C>(&self, conn: &C, cart: entities::cart::Model) -> CartResult<()>
    where
        C: sea_orm::ConnectionTrait,
    {
        let line_items = entities::cart_line_item::Entity::find()
            .filter(entities::cart_line_item::Column::CartId.eq(cart.id))
            .all(conn)
            .await?;
        let total_amount = line_items
            .into_iter()
            .fold(Decimal::ZERO, |acc, item| acc + item.total_price);

        let mut active: entities::cart::ActiveModel = cart.into();
        active.total_amount = Set(total_amount);
        active.updated_at = Set(Utc::now().into());
        active.update(conn).await?;
        Ok(())
    }

    async fn load_cart(&self, tenant_id: Uuid, cart_id: Uuid) -> CartResult<entities::cart::Model> {
        self.load_cart_in_tx(&self.db, tenant_id, cart_id).await
    }

    async fn load_cart_in_tx<C>(
        &self,
        conn: &C,
        tenant_id: Uuid,
        cart_id: Uuid,
    ) -> CartResult<entities::cart::Model>
    where
        C: sea_orm::ConnectionTrait,
    {
        entities::cart::Entity::find_by_id(cart_id)
            .filter(entities::cart::Column::TenantId.eq(tenant_id))
            .one(conn)
            .await?
            .ok_or(CartError::CartNotFound(cart_id))
    }

    async fn build_response(&self, cart: entities::cart::Model) -> CartResult<CartResponse> {
        let line_items = entities::cart_line_item::Entity::find()
            .filter(entities::cart_line_item::Column::CartId.eq(cart.id))
            .order_by_asc(entities::cart_line_item::Column::CreatedAt)
            .all(&self.db)
            .await?;
        let shipping_selections = entities::cart_shipping_selection::Entity::find()
            .filter(entities::cart_shipping_selection::Column::CartId.eq(cart.id))
            .all(&self.db)
            .await?;
        let selection_map = shipping_selections
            .into_iter()
            .map(|selection| {
                (
                    selection.shipping_profile_slug,
                    selection.selected_shipping_option_id,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let delivery_groups = build_delivery_groups(&line_items, &selection_map);
        let selected_shipping_option_id = if delivery_groups.len() == 1 {
            delivery_groups[0].selected_shipping_option_id
        } else {
            None
        };

        Ok(CartResponse {
            id: cart.id,
            tenant_id: cart.tenant_id,
            channel_id: cart.channel_id,
            channel_slug: cart.channel_slug,
            customer_id: cart.customer_id,
            email: cart.email,
            region_id: cart.region_id,
            country_code: cart.country_code,
            locale_code: cart.locale_code,
            selected_shipping_option_id,
            status: cart.status,
            currency_code: cart.currency_code,
            total_amount: cart.total_amount,
            metadata: cart.metadata,
            created_at: cart.created_at.with_timezone(&Utc),
            updated_at: cart.updated_at.with_timezone(&Utc),
            completed_at: cart.completed_at.map(|value| value.with_timezone(&Utc)),
            line_items: line_items
                .into_iter()
                .map(|item| CartLineItemResponse {
                    id: item.id,
                    cart_id: item.cart_id,
                    product_id: item.product_id,
                    variant_id: item.variant_id,
                    shipping_profile_slug: item.shipping_profile_slug,
                    sku: item.sku,
                    title: item.title,
                    quantity: item.quantity,
                    unit_price: item.unit_price,
                    total_price: item.total_price,
                    currency_code: item.currency_code,
                    metadata: item.metadata,
                    created_at: item.created_at.with_timezone(&Utc),
                    updated_at: item.updated_at.with_timezone(&Utc),
                })
                .collect(),
            delivery_groups,
        })
    }

    async fn apply_shipping_selection_patch<C>(
        &self,
        conn: &C,
        cart: &entities::cart::Model,
        input: &UpdateCartContextInput,
    ) -> CartResult<()>
    where
        C: sea_orm::ConnectionTrait,
    {
        let line_items = entities::cart_line_item::Entity::find()
            .filter(entities::cart_line_item::Column::CartId.eq(cart.id))
            .all(conn)
            .await?;
        let available_profiles = collect_shipping_profiles(&line_items);
        let existing = entities::cart_shipping_selection::Entity::find()
            .filter(entities::cart_shipping_selection::Column::CartId.eq(cart.id))
            .all(conn)
            .await?;
        let mut desired = existing
            .into_iter()
            .filter_map(|selection| {
                available_profiles
                    .contains(selection.shipping_profile_slug.as_str())
                    .then_some((
                        selection.shipping_profile_slug,
                        selection.selected_shipping_option_id,
                    ))
            })
            .collect::<BTreeMap<_, _>>();

        if let Some(shipping_selections) = &input.shipping_selections {
            desired.clear();
            for selection in shipping_selections {
                let normalized =
                    normalize_shipping_profile_slug(Some(selection.shipping_profile_slug.as_str()));
                if available_profiles.contains(normalized.as_str()) {
                    desired.insert(normalized, selection.selected_shipping_option_id);
                }
            }
        } else if available_profiles.len() <= 1 {
            if let Some(profile_slug) = available_profiles.iter().next() {
                desired.insert(profile_slug.clone(), input.selected_shipping_option_id);
            } else {
                desired.clear();
            }
        } else if input.selected_shipping_option_id != cart.selected_shipping_option_id
            && input.selected_shipping_option_id.is_some()
        {
            return Err(CartError::Validation(
                "selected_shipping_option_id can only be used for carts with a single delivery group"
                    .to_string(),
            ));
        }

        self.store_shipping_selections(conn, cart.id, desired)
            .await?;
        self.reconcile_cart_shipping_state(conn, cart.id).await
    }

    async fn store_shipping_selections<C>(
        &self,
        conn: &C,
        cart_id: Uuid,
        desired: BTreeMap<String, Option<Uuid>>,
    ) -> CartResult<()>
    where
        C: sea_orm::ConnectionTrait,
    {
        let existing = entities::cart_shipping_selection::Entity::find()
            .filter(entities::cart_shipping_selection::Column::CartId.eq(cart_id))
            .all(conn)
            .await?;
        let existing_map = existing
            .into_iter()
            .map(|selection| (selection.shipping_profile_slug.clone(), selection))
            .collect::<BTreeMap<_, _>>();
        let now = Utc::now();

        for (shipping_profile_slug, selected_shipping_option_id) in &desired {
            if let Some(current) = existing_map.get(shipping_profile_slug) {
                let mut active: entities::cart_shipping_selection::ActiveModel =
                    current.clone().into();
                active.selected_shipping_option_id = Set(*selected_shipping_option_id);
                active.updated_at = Set(now.into());
                active.update(conn).await?;
            } else {
                entities::cart_shipping_selection::ActiveModel {
                    id: Set(generate_id()),
                    cart_id: Set(cart_id),
                    shipping_profile_slug: Set(shipping_profile_slug.clone()),
                    selected_shipping_option_id: Set(*selected_shipping_option_id),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(conn)
                .await?;
            }
        }

        for (shipping_profile_slug, current) in existing_map {
            if !desired.contains_key(&shipping_profile_slug) {
                let active: entities::cart_shipping_selection::ActiveModel = current.into();
                active.delete(conn).await?;
            }
        }

        Ok(())
    }

    async fn reconcile_cart_shipping_state<C>(&self, conn: &C, cart_id: Uuid) -> CartResult<()>
    where
        C: sea_orm::ConnectionTrait,
    {
        let cart = entities::cart::Entity::find_by_id(cart_id)
            .one(conn)
            .await?
            .ok_or(CartError::CartNotFound(cart_id))?;
        let line_items = entities::cart_line_item::Entity::find()
            .filter(entities::cart_line_item::Column::CartId.eq(cart_id))
            .order_by_asc(entities::cart_line_item::Column::CreatedAt)
            .all(conn)
            .await?;
        let profiles = collect_shipping_profiles(&line_items);
        let mut desired = entities::cart_shipping_selection::Entity::find()
            .filter(entities::cart_shipping_selection::Column::CartId.eq(cart_id))
            .all(conn)
            .await?
            .into_iter()
            .filter_map(|selection| {
                profiles
                    .contains(selection.shipping_profile_slug.as_str())
                    .then_some((
                        selection.shipping_profile_slug,
                        selection.selected_shipping_option_id,
                    ))
            })
            .collect::<BTreeMap<_, _>>();

        if profiles.len() == 1
            && desired.is_empty()
            && cart.selected_shipping_option_id.is_some()
            && !line_items.is_empty()
        {
            let profile_slug = profiles.iter().next().cloned().unwrap_or_default();
            desired.insert(profile_slug, cart.selected_shipping_option_id);
        }

        self.store_shipping_selections(conn, cart_id, desired.clone())
            .await?;

        let legacy_selected_shipping_option_id = if profiles.len() == 1 {
            profiles
                .iter()
                .next()
                .and_then(|profile_slug| desired.get(profile_slug).copied().flatten())
        } else {
            None
        };
        let mut active: entities::cart::ActiveModel = cart.into();
        active.selected_shipping_option_id = Set(legacy_selected_shipping_option_id);
        active.updated_at = Set(Utc::now().into());
        active.update(conn).await?;
        Ok(())
    }
}

fn ensure_active(status: &str, action: &str) -> CartResult<()> {
    if status == STATUS_ACTIVE {
        Ok(())
    } else {
        Err(CartError::InvalidTransition {
            from: status.to_string(),
            to: action.to_string(),
        })
    }
}

fn normalize_country_code(value: &str) -> CartResult<String> {
    let normalized = value.trim().to_ascii_uppercase();
    if normalized.len() == 2 && normalized.chars().all(|ch| ch.is_ascii_alphabetic()) {
        Ok(normalized)
    } else {
        Err(CartError::Validation(format!(
            "country_code `{value}` is invalid"
        )))
    }
}

fn normalize_locale_code(value: &str) -> CartResult<String> {
    let normalized = value.trim().replace('_', "-").to_ascii_lowercase();
    if (2..=10).contains(&normalized.len()) {
        Ok(normalized)
    } else {
        Err(CartError::Validation(format!(
            "locale_code `{value}` is invalid"
        )))
    }
}

fn normalize_shipping_profile_slug(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_else(|| DEFAULT_SHIPPING_PROFILE_SLUG.to_string())
}

fn collect_shipping_profiles(line_items: &[entities::cart_line_item::Model]) -> BTreeSet<String> {
    line_items
        .iter()
        .map(|item| normalize_shipping_profile_slug(Some(item.shipping_profile_slug.as_str())))
        .collect()
}

fn build_delivery_groups(
    line_items: &[entities::cart_line_item::Model],
    selection_map: &BTreeMap<String, Option<Uuid>>,
) -> Vec<CartDeliveryGroupResponse> {
    let mut groups = BTreeMap::<String, Vec<Uuid>>::new();
    for item in line_items {
        groups
            .entry(normalize_shipping_profile_slug(Some(
                item.shipping_profile_slug.as_str(),
            )))
            .or_default()
            .push(item.id);
    }

    groups
        .into_iter()
        .map(
            |(shipping_profile_slug, line_item_ids)| CartDeliveryGroupResponse {
                selected_shipping_option_id: selection_map
                    .get(&shipping_profile_slug)
                    .copied()
                    .flatten(),
                shipping_profile_slug,
                line_item_ids,
                available_shipping_options: Vec::new(),
            },
        )
        .collect()
}
