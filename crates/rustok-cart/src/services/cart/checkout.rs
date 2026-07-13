use chrono::Utc;
use sea_orm::{
    sea_query::Expr, ColumnTrait, EntityTrait, QueryFilter, TransactionTrait,
};
use uuid::Uuid;

use crate::dto::{CartResponse, CartStatus};
use crate::entities;
use crate::error::{CartError, CartResult};

use super::CartService;
use super::helpers::load_cart_in_tx;

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
