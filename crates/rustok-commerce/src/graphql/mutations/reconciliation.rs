use async_graphql::{Context, Object, Result};
use rustok_api::{Permission, graphql::require_module_enabled};
use uuid::Uuid;

use crate::graphql_runtime::refund_reconciliation_from_context;

use super::super::{MODULE_SLUG, require_commerce_permission, types::GqlRefund};

#[derive(Default)]
pub struct CommerceReconciliationMutation;

#[Object]
impl CommerceReconciliationMutation {
    /// Resume a previously journaled refund provider operation using its original
    /// persisted request and idempotency key.
    async fn retry_refund_provider(
        &self,
        ctx: &Context<'_>,
        tenant_id: Uuid,
        refund_id: Uuid,
    ) -> Result<GqlRefund> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PAYMENTS_UPDATE],
            "Permission denied: payments:update required",
        )?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let refund = refund_reconciliation_from_context(ctx, db.clone())
            .retry_refund_provider(tenant_id, refund_id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(refund.into())
    }
}
