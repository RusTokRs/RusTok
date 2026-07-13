use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};
use serde_json::Value;
use uuid::Uuid;

use rustok_core::generate_id;

use crate::dto::CartResponse;
use crate::entities;
use crate::error::{CartError, CartResult};

use super::CartService;
use super::helpers::{
    CART_PROMOTION_SCOPE, LINE_ITEM_PROMOTION_SCOPE, PROMOTION_ADJUSTMENT_SOURCE_TYPE,
    SHIPPING_PROMOTION_SCOPE, ensure_active, load_cart, load_cart_in_tx,
    normalize_required_adjustment_source_id, promotion_metadata, recalculate_totals,
    reconcile_cart_shipping_state, resolve_promotion_base_amount,
    resolve_shipping_promotion_base_amount, sanitize_adjustment_metadata,
};
use super::types::{CartPromotionKind, CartPromotionPreview};

struct PromotionAdjustmentInput<'a> {
    line_item_id: Option<Uuid>,
    source_id: &'a str,
    amount: Decimal,
    scope: &'a str,
    metadata: Value,
}

impl CartService {
    pub async fn preview_percentage_promotion(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        line_item_id: Option<Uuid>,
        source_id: &str,
        discount_percent: Decimal,
    ) -> CartResult<CartPromotionPreview> {
        super::helpers::validate_promotion_percent(discount_percent)?;

        let cart = load_cart(&self.db, tenant_id, cart_id).await?;
        ensure_active(&cart.status, "preview_percentage_promotion")?;
        let line_items = entities::cart_line_item::Entity::find()
            .filter(entities::cart_line_item::Column::CartId.eq(cart_id))
            .all(&self.db)
            .await?;
        let adjustments = entities::cart_adjustment::Entity::find()
            .filter(entities::cart_adjustment::Column::CartId.eq(cart_id))
            .all(&self.db)
            .await?;
        let source_id = normalize_required_adjustment_source_id(source_id)?;
        let base_amount =
            resolve_promotion_base_amount(&line_items, &adjustments, line_item_id, &source_id)?;
        let adjusted_amount = (base_amount
            * ((Decimal::from(100) - discount_percent) / Decimal::from(100)))
        .round_dp(2);
        let adjustment_amount = (base_amount - adjusted_amount).round_dp(2);

        Ok(CartPromotionPreview {
            kind: CartPromotionKind::PercentageDiscount,
            line_item_id,
            currency_code: cart.currency_code,
            base_amount,
            adjustment_amount,
            adjusted_amount,
        })
    }

    pub async fn apply_percentage_promotion(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        line_item_id: Option<Uuid>,
        source_id: &str,
        discount_percent: Decimal,
        metadata: Value,
    ) -> CartResult<CartResponse> {
        let preview = self
            .preview_percentage_promotion(
                tenant_id,
                cart_id,
                line_item_id,
                source_id,
                discount_percent,
            )
            .await?;
        let metadata = promotion_metadata(
            metadata,
            CartPromotionKind::PercentageDiscount,
            if line_item_id.is_some() {
                LINE_ITEM_PROMOTION_SCOPE
            } else {
                CART_PROMOTION_SCOPE
            },
            Some(discount_percent),
            None,
        );

        self.apply_promotion_adjustment(
            tenant_id,
            cart_id,
            PromotionAdjustmentInput {
                line_item_id,
                source_id,
                amount: preview.adjustment_amount,
                scope: if line_item_id.is_some() {
                    LINE_ITEM_PROMOTION_SCOPE
                } else {
                    CART_PROMOTION_SCOPE
                },
                metadata,
            },
        )
        .await
    }

    pub async fn preview_fixed_promotion(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        line_item_id: Option<Uuid>,
        source_id: &str,
        amount: Decimal,
    ) -> CartResult<CartPromotionPreview> {
        super::helpers::validate_fixed_promotion_amount(amount)?;

        let cart = load_cart(&self.db, tenant_id, cart_id).await?;
        ensure_active(&cart.status, "preview_fixed_promotion")?;
        let line_items = entities::cart_line_item::Entity::find()
            .filter(entities::cart_line_item::Column::CartId.eq(cart_id))
            .all(&self.db)
            .await?;
        let adjustments = entities::cart_adjustment::Entity::find()
            .filter(entities::cart_adjustment::Column::CartId.eq(cart_id))
            .all(&self.db)
            .await?;
        let source_id = normalize_required_adjustment_source_id(source_id)?;
        let base_amount =
            resolve_promotion_base_amount(&line_items, &adjustments, line_item_id, &source_id)?;
        if amount > base_amount {
            return Err(CartError::Validation(
                "promotion amount cannot exceed the remaining base amount".to_string(),
            ));
        }

        Ok(CartPromotionPreview {
            kind: CartPromotionKind::FixedDiscount,
            line_item_id,
            currency_code: cart.currency_code,
            base_amount,
            adjustment_amount: amount.round_dp(2),
            adjusted_amount: (base_amount - amount).round_dp(2),
        })
    }

    pub async fn apply_fixed_promotion(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        line_item_id: Option<Uuid>,
        source_id: &str,
        amount: Decimal,
        metadata: Value,
    ) -> CartResult<CartResponse> {
        let preview = self
            .preview_fixed_promotion(tenant_id, cart_id, line_item_id, source_id, amount)
            .await?;
        let metadata = promotion_metadata(
            metadata,
            CartPromotionKind::FixedDiscount,
            if line_item_id.is_some() {
                LINE_ITEM_PROMOTION_SCOPE
            } else {
                CART_PROMOTION_SCOPE
            },
            None,
            Some(preview.adjustment_amount),
        );

        self.apply_promotion_adjustment(
            tenant_id,
            cart_id,
            PromotionAdjustmentInput {
                line_item_id,
                source_id,
                amount: preview.adjustment_amount,
                scope: if line_item_id.is_some() {
                    LINE_ITEM_PROMOTION_SCOPE
                } else {
                    CART_PROMOTION_SCOPE
                },
                metadata,
            },
        )
        .await
    }

    pub async fn preview_percentage_shipping_promotion(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        source_id: &str,
        discount_percent: Decimal,
    ) -> CartResult<CartPromotionPreview> {
        super::helpers::validate_promotion_percent(discount_percent)?;

        let cart = load_cart(&self.db, tenant_id, cart_id).await?;
        ensure_active(&cart.status, "preview_percentage_shipping_promotion")?;
        let adjustments = entities::cart_adjustment::Entity::find()
            .filter(entities::cart_adjustment::Column::CartId.eq(cart_id))
            .all(&self.db)
            .await?;
        let source_id = normalize_required_adjustment_source_id(source_id)?;
        let base_amount =
            resolve_shipping_promotion_base_amount(cart.shipping_total, &adjustments, &source_id);
        let adjusted_amount = (base_amount
            * ((Decimal::from(100) - discount_percent) / Decimal::from(100)))
        .round_dp(2);
        let adjustment_amount = (base_amount - adjusted_amount).round_dp(2);

        Ok(CartPromotionPreview {
            kind: CartPromotionKind::PercentageDiscount,
            line_item_id: None,
            currency_code: cart.currency_code,
            base_amount,
            adjustment_amount,
            adjusted_amount,
        })
    }

    pub async fn apply_percentage_shipping_promotion(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        source_id: &str,
        discount_percent: Decimal,
        metadata: Value,
    ) -> CartResult<CartResponse> {
        let preview = self
            .preview_percentage_shipping_promotion(tenant_id, cart_id, source_id, discount_percent)
            .await?;
        let metadata = promotion_metadata(
            metadata,
            CartPromotionKind::PercentageDiscount,
            SHIPPING_PROMOTION_SCOPE,
            Some(discount_percent),
            None,
        );

        self.apply_promotion_adjustment(
            tenant_id,
            cart_id,
            PromotionAdjustmentInput {
                line_item_id: None,
                source_id,
                amount: preview.adjustment_amount,
                scope: SHIPPING_PROMOTION_SCOPE,
                metadata,
            },
        )
        .await
    }

    pub async fn preview_fixed_shipping_promotion(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        source_id: &str,
        amount: Decimal,
    ) -> CartResult<CartPromotionPreview> {
        super::helpers::validate_fixed_promotion_amount(amount)?;

        let cart = load_cart(&self.db, tenant_id, cart_id).await?;
        ensure_active(&cart.status, "preview_fixed_shipping_promotion")?;
        let adjustments = entities::cart_adjustment::Entity::find()
            .filter(entities::cart_adjustment::Column::CartId.eq(cart_id))
            .all(&self.db)
            .await?;
        let source_id = normalize_required_adjustment_source_id(source_id)?;
        let base_amount =
            resolve_shipping_promotion_base_amount(cart.shipping_total, &adjustments, &source_id);
        if amount > base_amount {
            return Err(CartError::Validation(
                "shipping promotion amount cannot exceed the remaining shipping amount".to_string(),
            ));
        }

        Ok(CartPromotionPreview {
            kind: CartPromotionKind::FixedDiscount,
            line_item_id: None,
            currency_code: cart.currency_code,
            base_amount,
            adjustment_amount: amount.round_dp(2),
            adjusted_amount: (base_amount - amount).round_dp(2),
        })
    }

    pub async fn apply_fixed_shipping_promotion(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        source_id: &str,
        amount: Decimal,
        metadata: Value,
    ) -> CartResult<CartResponse> {
        let preview = self
            .preview_fixed_shipping_promotion(tenant_id, cart_id, source_id, amount)
            .await?;
        let metadata = promotion_metadata(
            metadata,
            CartPromotionKind::FixedDiscount,
            SHIPPING_PROMOTION_SCOPE,
            None,
            Some(preview.adjustment_amount),
        );

        self.apply_promotion_adjustment(
            tenant_id,
            cart_id,
            PromotionAdjustmentInput {
                line_item_id: None,
                source_id,
                amount: preview.adjustment_amount,
                scope: SHIPPING_PROMOTION_SCOPE,
                metadata,
            },
        )
        .await
    }

    async fn apply_promotion_adjustment(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        input: PromotionAdjustmentInput<'_>,
    ) -> CartResult<CartResponse> {
        let PromotionAdjustmentInput {
            line_item_id,
            source_id,
            amount,
            scope,
            metadata,
        } = input;
        let source_id = normalize_required_adjustment_source_id(source_id)?;
        let txn = self.db.begin().await?;
        let cart = load_cart_in_tx(&txn, tenant_id, cart_id).await?;
        ensure_active(&cart.status, "apply_promotion_adjustment")?;

        let line_items = entities::cart_line_item::Entity::find()
            .filter(entities::cart_line_item::Column::CartId.eq(cart_id))
            .all(&txn)
            .await?;
        let adjustments = entities::cart_adjustment::Entity::find()
            .filter(entities::cart_adjustment::Column::CartId.eq(cart_id))
            .all(&txn)
            .await?;

        if let Some(line_item_id) = line_item_id {
            if !line_items.iter().any(|item| item.id == line_item_id) {
                return Err(CartError::Validation(format!(
                    "cart line item {line_item_id} does not belong to cart {cart_id}"
                )));
            }
        }

        let base_amount = match scope {
            SHIPPING_PROMOTION_SCOPE => resolve_shipping_promotion_base_amount(
                cart.shipping_total,
                &adjustments,
                &source_id,
            ),
            _ => {
                resolve_promotion_base_amount(&line_items, &adjustments, line_item_id, &source_id)?
            }
        };
        if amount > base_amount {
            return Err(CartError::Validation(match scope {
                SHIPPING_PROMOTION_SCOPE => {
                    "shipping promotion amount cannot exceed the remaining shipping amount"
                        .to_string()
                }
                _ => "promotion amount cannot exceed the remaining base amount".to_string(),
            }));
        }

        let mut delete_query = entities::cart_adjustment::Entity::delete_many()
            .filter(entities::cart_adjustment::Column::CartId.eq(cart_id))
            .filter(
                entities::cart_adjustment::Column::SourceType.eq(PROMOTION_ADJUSTMENT_SOURCE_TYPE),
            )
            .filter(entities::cart_adjustment::Column::SourceId.eq(source_id.as_str()));
        delete_query = match line_item_id {
            Some(line_item_id) => delete_query
                .filter(entities::cart_adjustment::Column::CartLineItemId.eq(line_item_id)),
            None => {
                delete_query.filter(entities::cart_adjustment::Column::CartLineItemId.is_null())
            }
        };
        delete_query.exec(&txn).await?;

        entities::cart_adjustment::ActiveModel {
            id: Set(generate_id()),
            cart_id: Set(cart_id),
            cart_line_item_id: Set(line_item_id),
            source_type: Set(PROMOTION_ADJUSTMENT_SOURCE_TYPE.to_string()),
            source_id: Set(Some(source_id)),
            amount: Set(amount.round_dp(2)),
            currency_code: Set(cart.currency_code.to_ascii_uppercase()),
            metadata: Set(sanitize_adjustment_metadata(metadata)),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        }
        .insert(&txn)
        .await?;

        recalculate_totals(&txn, self.tax_calculation_port.as_ref(), cart).await?;
        reconcile_cart_shipping_state(&txn, cart_id).await?;
        txn.commit().await?;
        self.get_cart(tenant_id, cart_id).await
    }
}
