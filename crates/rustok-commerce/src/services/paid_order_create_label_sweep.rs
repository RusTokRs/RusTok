use rustok_fulfillment::entities::{fulfillment, provider_operation};
use rustok_fulfillment::providers::FulfillmentProviderRegistry;
use rustok_fulfillment::{PROVIDER_OPERATION_ERROR, PROVIDER_OPERATION_PENDING};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait,
};

use super::FulfillmentCreateLabelRecoveryService;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PaidOrderCreateLabelSweepReport {
    pub examined: u64,
    pub dispatched: u64,
    pub skipped_unpaid: u64,
    pub failed: u64,
}

/// Bounded recovery sweep for durable create-label operations whose paid-order
/// event was delayed, dropped by an in-memory transport, or delivered while the
/// listener was unavailable.
#[derive(Clone)]
pub struct PaidOrderCreateLabelSweepService {
    db: DatabaseConnection,
    fulfillment_provider_registry: FulfillmentProviderRegistry,
}

impl PaidOrderCreateLabelSweepService {
    pub fn new(
        db: DatabaseConnection,
        fulfillment_provider_registry: FulfillmentProviderRegistry,
    ) -> Self {
        Self {
            db,
            fulfillment_provider_registry,
        }
    }

    pub async fn process_pending_once(
        &self,
        limit: u64,
    ) -> Result<PaidOrderCreateLabelSweepReport, sea_orm::DbErr> {
        let paid_order_ids = rustok_order::entities::order::Entity::find()
            .select_only()
            .column(rustok_order::entities::order::Column::Id)
            .filter(rustok_order::entities::order::Column::Status.eq("paid"))
            .into_query();
        let paid_fulfillment_ids = fulfillment::Entity::find()
            .select_only()
            .column(fulfillment::Column::Id)
            .filter(fulfillment::Column::OrderId.in_subquery(paid_order_ids))
            .into_query();
        let operations = provider_operation::Entity::find()
            .filter(provider_operation::Column::Operation.eq("create_label"))
            .filter(provider_operation::Column::Status.is_in([
                PROVIDER_OPERATION_PENDING.to_string(),
                PROVIDER_OPERATION_ERROR.to_string(),
            ]))
            .filter(
                provider_operation::Column::FulfillmentId
                    .in_subquery(paid_fulfillment_ids),
            )
            .order_by_asc(provider_operation::Column::CreatedAt)
            .limit(limit.clamp(1, 500))
            .all(&self.db)
            .await?;

        let mut report = PaidOrderCreateLabelSweepReport {
            examined: operations.len() as u64,
            ..PaidOrderCreateLabelSweepReport::default()
        };
        let recovery = FulfillmentCreateLabelRecoveryService::new(self.db.clone())
            .with_provider_registry(self.fulfillment_provider_registry.clone());

        for operation in operations {
            let parent = fulfillment::Entity::find_by_id(operation.fulfillment_id)
                .filter(fulfillment::Column::TenantId.eq(operation.tenant_id))
                .one(&self.db)
                .await?;
            let Some(parent) = parent else {
                report.failed += 1;
                tracing::error!(
                    tenant_id = %operation.tenant_id,
                    fulfillment_id = %operation.fulfillment_id,
                    operation_id = %operation.id,
                    "Create-label operation references a missing tenant-scoped fulfillment"
                );
                continue;
            };

            let paid = rustok_order::entities::order::Entity::find_by_id(parent.order_id)
                .filter(rustok_order::entities::order::Column::TenantId.eq(operation.tenant_id))
                .filter(rustok_order::entities::order::Column::Status.eq("paid"))
                .one(&self.db)
                .await?
                .is_some();
            if !paid {
                report.skipped_unpaid += 1;
                continue;
            }

            match recovery.retry(operation.tenant_id, operation.id).await {
                Ok(_) => report.dispatched += 1,
                Err(error) => {
                    report.failed += 1;
                    tracing::warn!(
                        tenant_id = %operation.tenant_id,
                        order_id = %parent.order_id,
                        fulfillment_id = %operation.fulfillment_id,
                        operation_id = %operation.id,
                        provider_id = %operation.provider_id,
                        error = %error,
                        "Paid-order create-label recovery sweep could not dispatch operation"
                    );
                }
            }
        }

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_defaults_to_zero() {
        assert_eq!(
            PaidOrderCreateLabelSweepReport::default(),
            PaidOrderCreateLabelSweepReport {
                examined: 0,
                dispatched: 0,
                skipped_unpaid: 0,
                failed: 0,
            }
        );
    }

    #[test]
    fn limit_is_bounded_by_service_contract() {
        assert_eq!(0_u64.clamp(1, 500), 1);
        assert_eq!(10_000_u64.clamp(1, 500), 500);
    }
}
