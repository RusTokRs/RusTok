use chrono::Utc;
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use uuid::Uuid;
use validator::Validate;

use crate::checkout_snapshot::PrepareCartCheckoutSnapshotRequest;
use crate::dto::{CartResponse, CartStatus, UpdateCartContextInput};
use crate::entities;
use crate::error::{CartError, CartResult};

use super::CartService;
use super::helpers::{
    apply_shipping_selection_patch, load_cart_in_tx, normalize_country_code, normalize_locale_code,
    recalculate_totals,
};

impl CartService {
    pub async fn complete_cart(&self, tenant_id: Uuid, cart_id: Uuid) -> CartResult<CartResponse> {
        self.transition_status(
            tenant_id,
            cart_id,
            &[CartStatus::Active, CartStatus::CheckingOut],
            CartStatus::Completed,
            true,
        )
        .await
    }

    pub async fn abandon_cart(&self, tenant_id: Uuid, cart_id: Uuid) -> CartResult<CartResponse> {
        self.transition_status(
            tenant_id,
            cart_id,
            &[CartStatus::Active],
            CartStatus::Abandoned,
            false,
        )
        .await
    }

    pub async fn begin_checkout(&self, tenant_id: Uuid, cart_id: Uuid) -> CartResult<CartResponse> {
        self.transition_status(
            tenant_id,
            cart_id,
            &[CartStatus::Active],
            CartStatus::CheckingOut,
            false,
        )
        .await
    }

    /// Atomically applies the effective checkout context, recalculates all
    /// cart-owned totals and transitions the cart into `checking_out`.
    ///
    /// The status compare-and-set is executed first inside the transaction. It
    /// acquires the cart row for the remainder of the transaction, so normal
    /// active-cart mutations cannot race the prepared commercial snapshot.
    pub async fn prepare_checkout(
        &self,
        tenant_id: Uuid,
        request: PrepareCartCheckoutSnapshotRequest,
    ) -> CartResult<CartResponse> {
        for selection in request.shipping_selections.iter().flatten() {
            selection
                .validate()
                .map_err(|error| CartError::Validation(error.to_string()))?;
        }

        let txn = self.db.begin().await?;
        let now = Utc::now().fixed_offset();
        let claimed = entities::cart::Entity::update_many()
            .col_expr(
                entities::cart::Column::Status,
                Expr::value(CartStatus::CheckingOut.as_str()),
            )
            .col_expr(entities::cart::Column::UpdatedAt, Expr::value(now))
            .filter(entities::cart::Column::TenantId.eq(tenant_id))
            .filter(entities::cart::Column::Id.eq(request.cart_id))
            .filter(entities::cart::Column::Status.eq(CartStatus::Active.as_str()))
            .exec(&txn)
            .await?;

        if claimed.rows_affected != 1 {
            let current = load_cart_in_tx(&txn, tenant_id, request.cart_id).await?;
            let from = current.status;
            txn.rollback().await?;
            return Err(CartError::InvalidTransition {
                from,
                to: CartStatus::CheckingOut.to_string(),
            });
        }

        let cart = load_cart_in_tx(&txn, tenant_id, request.cart_id).await?;
        let has_line_items = entities::cart_line_item::Entity::find()
            .filter(entities::cart_line_item::Column::CartId.eq(cart.id))
            .one(&txn)
            .await?
            .is_some();
        if !has_line_items {
            txn.rollback().await?;
            return Err(CartError::Validation(format!(
                "cart {} has no line items",
                cart.id
            )));
        }

        let shipping_patch_requested =
            request.shipping_selections.is_some() || request.selected_shipping_option_id.is_some();
        let country_code = match cart.country_code.clone() {
            Some(country_code) => Some(country_code),
            None => request
                .country_code
                .as_deref()
                .map(normalize_country_code)
                .transpose()?,
        };
        let locale_code = match cart.locale_code.clone() {
            Some(locale_code) => Some(locale_code),
            None => request
                .locale_code
                .as_deref()
                .map(normalize_locale_code)
                .transpose()?,
        };
        let selected_shipping_option_id = if shipping_patch_requested {
            request.selected_shipping_option_id
        } else {
            cart.selected_shipping_option_id
        };
        let context_input = UpdateCartContextInput {
            email: cart.email.clone(),
            region_id: cart.region_id.or(request.region_id),
            country_code,
            locale_code,
            selected_shipping_option_id,
            shipping_selections: request.shipping_selections,
        };

        let mut active: entities::cart::ActiveModel = cart.clone().into();
        active.email = Set(context_input.email.clone());
        active.region_id = Set(context_input.region_id);
        active.country_code = Set(context_input.country_code.clone());
        active.locale_code = Set(context_input.locale_code.clone());
        active.selected_shipping_option_id = Set(context_input.selected_shipping_option_id);
        active.updated_at = Set(Utc::now().into());
        active.update(&txn).await?;

        if shipping_patch_requested {
            apply_shipping_selection_patch(&txn, &cart, &context_input).await?;
        }

        let prepared_cart = load_cart_in_tx(&txn, tenant_id, cart.id).await?;
        recalculate_totals(
            &txn,
            self.tax_calculation_port.as_ref(),
            prepared_cart,
        )
        .await?;
        txn.commit().await?;
        self.get_cart(tenant_id, cart.id).await
    }

    pub async fn release_checkout(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
    ) -> CartResult<CartResponse> {
        self.transition_status(
            tenant_id,
            cart_id,
            &[CartStatus::CheckingOut],
            CartStatus::Active,
            false,
        )
        .await
    }

    async fn transition_status(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        allowed_from: &[CartStatus],
        target: CartStatus,
        mark_completed: bool,
    ) -> CartResult<CartResponse> {
        let txn = self.db.begin().await?;
        let now = Utc::now().fixed_offset();
        let completed_at = mark_completed.then(|| now.clone());
        let allowed_from = allowed_from
            .iter()
            .map(|status| status.as_str())
            .collect::<Vec<_>>();

        let result = entities::cart::Entity::update_many()
            .col_expr(
                entities::cart::Column::Status,
                Expr::value(target.as_str()),
            )
            .col_expr(entities::cart::Column::UpdatedAt, Expr::value(now))
            .col_expr(
                entities::cart::Column::CompletedAt,
                Expr::value(completed_at),
            )
            .filter(entities::cart::Column::TenantId.eq(tenant_id))
            .filter(entities::cart::Column::Id.eq(cart_id))
            .filter(entities::cart::Column::Status.is_in(allowed_from))
            .exec(&txn)
            .await?;

        if result.rows_affected != 1 {
            let current = load_cart_in_tx(&txn, tenant_id, cart_id).await?;
            let from = current.status;
            txn.rollback().await?;
            return Err(CartError::InvalidTransition {
                from,
                to: target.to_string(),
            });
        }

        txn.commit().await?;
        self.get_cart(tenant_id, cart_id).await
    }
}
