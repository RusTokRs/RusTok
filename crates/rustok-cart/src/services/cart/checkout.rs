use chrono::Utc;
use sea_orm::{ActiveModelTrait, Set, TransactionTrait};
use uuid::Uuid;

use crate::dto::CartResponse;
use crate::entities;
use crate::error::{CartError, CartResult};

use super::CartService;
use super::helpers::{
    load_cart_in_tx, STATUS_ABANDONED, STATUS_ACTIVE, STATUS_CHECKING_OUT, STATUS_COMPLETED,
};

impl CartService {
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
        let cart = load_cart_in_tx(&txn, tenant_id, cart_id).await?;
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
        let cart = load_cart_in_tx(&txn, tenant_id, cart_id).await?;
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
}
