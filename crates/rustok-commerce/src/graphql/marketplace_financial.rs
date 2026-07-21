use async_graphql::{Context, ErrorExtensions, Object, Result, SimpleObject};
use chrono::{DateTime, FixedOffset};
use rust_decimal::Decimal;
use rustok_api::{Permission, TenantContext, graphql::require_module_enabled};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::graphql_runtime::CommerceGraphqlRuntimeData;

use super::{MODULE_SLUG, require_commerce_permission};

#[derive(Default)]
pub struct MarketplaceFinancialQuery;

#[Object]
impl MarketplaceFinancialQuery {
    async fn admin_marketplace_financial_operation(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> Result<MarketplaceFinancialOperationGql> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PAYMENTS_READ, Permission::ORDERS_READ],
            "Permission denied: payments:read or orders:read required",
        )?;
        let (tenant_id, service) = operator_service(ctx)?;
        service
            .get_financial_operation(tenant_id, id)
            .await
            .map(Into::into)
            .map_err(map_operator_error)
    }

    async fn admin_marketplace_financial_operations_operator_review(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
    ) -> Result<Vec<MarketplaceFinancialOperationGql>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PAYMENTS_READ, Permission::ORDERS_READ],
            "Permission denied: payments:read or orders:read required",
        )?;
        let (tenant_id, service) = operator_service(ctx)?;
        service
            .list_financial_operator_review(tenant_id, bounded_limit(limit))
            .await
            .map(|items| items.into_iter().map(Into::into).collect())
            .map_err(map_operator_error)
    }

    async fn admin_marketplace_paid_event(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> Result<MarketplacePaidEventGql> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PAYMENTS_READ, Permission::ORDERS_READ],
            "Permission denied: payments:read or orders:read required",
        )?;
        let (tenant_id, service) = operator_service(ctx)?;
        service
            .get_paid_event(tenant_id, id)
            .await
            .map(Into::into)
            .map_err(map_operator_error)
    }

    async fn admin_marketplace_paid_events_operator_review(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
    ) -> Result<Vec<MarketplacePaidEventGql>> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PAYMENTS_READ, Permission::ORDERS_READ],
            "Permission denied: payments:read or orders:read required",
        )?;
        let (tenant_id, service) = operator_service(ctx)?;
        service
            .list_paid_event_operator_review(tenant_id, bounded_limit(limit))
            .await
            .map(|items| items.into_iter().map(Into::into).collect())
            .map_err(map_operator_error)
    }
}

#[derive(Default)]
pub struct MarketplaceFinancialMutation;

#[Object]
impl MarketplaceFinancialMutation {
    async fn retry_marketplace_financial_operation(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> Result<MarketplaceFinancialOperationGql> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PAYMENTS_MANAGE, Permission::ORDERS_MANAGE],
            "Permission denied: payments:manage or orders:manage required",
        )?;
        let (tenant_id, service) = operator_service(ctx)?;
        service
            .retry_financial_operation(tenant_id, id)
            .await
            .map(Into::into)
            .map_err(map_operator_error)
    }

    async fn retry_marketplace_paid_event(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> Result<MarketplacePaidEventGql> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PAYMENTS_MANAGE, Permission::ORDERS_MANAGE],
            "Permission denied: payments:manage or orders:manage required",
        )?;
        let (tenant_id, service) = operator_service(ctx)?;
        service
            .retry_paid_event(tenant_id, id)
            .await
            .map(Into::into)
            .map_err(map_operator_error)
    }

    async fn run_marketplace_financial_recovery_sweep(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
    ) -> Result<MarketplaceFinancialSweepGql> {
        require_module_enabled(ctx, MODULE_SLUG).await?;
        require_commerce_permission(
            ctx,
            &[Permission::PAYMENTS_MANAGE, Permission::ORDERS_MANAGE],
            "Permission denied: payments:manage or orders:manage required",
        )?;
        let (tenant_id, service) = operator_service(ctx)?;
        service
            .sweep_tenant(tenant_id, bounded_limit(limit))
            .await
            .map(Into::into)
            .map_err(map_operator_error)
    }
}

#[derive(Clone, Debug, SimpleObject)]
pub struct MarketplaceFinancialOperationGql {
    pub checkout_operation_id: Uuid,
    pub order_id: Uuid,
    pub payment_collection_id: Uuid,
    pub currency_code: String,
    pub status: String,
    pub stage: String,
    pub attempt_count: i32,
    pub ledger_transaction_id: Option<Uuid>,
    pub ledger_debit_total_amount: Option<i64>,
    pub ledger_credit_total_amount: Option<i64>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub completed_at: Option<DateTime<FixedOffset>>,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct MarketplacePaidEventGql {
    pub id: Uuid,
    pub event_source: String,
    pub event_id: String,
    pub checkout_operation_id: Uuid,
    pub order_id: Uuid,
    pub payment_collection_id: Uuid,
    pub captured_at: DateTime<FixedOffset>,
    pub currency_code: String,
    pub captured_amount: Decimal,
    pub status: String,
    pub attempt_count: i32,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub processed_at: Option<DateTime<FixedOffset>>,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct MarketplaceFinancialSweepFailureGql {
    pub inbox_id: Uuid,
    pub retryable: bool,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct MarketplaceFinancialSweepGql {
    pub selected: i32,
    pub processed: i32,
    pub retryable_failures: i32,
    pub operator_review_failures: i32,
    pub failures: Vec<MarketplaceFinancialSweepFailureGql>,
}

fn operator_service(
    ctx: &Context<'_>,
) -> Result<(Uuid, crate::MarketplaceFinancialOperatorService)> {
    let tenant_id = ctx.data::<TenantContext>()?.id;
    let db = ctx.data::<DatabaseConnection>()?.clone();
    let event_bus = ctx.data::<TransactionalEventBus>()?.clone();
    let runtime = ctx.data::<CommerceGraphqlRuntimeData>()?;
    Ok((
        tenant_id,
        runtime
            .marketplace_financial_runtime()
            .operator_service(db, event_bus),
    ))
}

fn bounded_limit(limit: Option<i32>) -> u64 {
    limit.unwrap_or(50).clamp(1, 100) as u64
}

fn map_operator_error(error: crate::MarketplaceFinancialOperatorError) -> async_graphql::Error {
    match error {
        crate::MarketplaceFinancialOperatorError::Validation(_) => async_graphql::Error::new(
            "Marketplace financial operator request is invalid",
        )
        .extend_with(|_, extensions| {
            extensions.set("code", "MARKETPLACE_FINANCIAL_OPERATOR_INVALID")
        }),
        crate::MarketplaceFinancialOperatorError::Conflict(message)
            if message.contains("was not found") =>
        {
            async_graphql::Error::new("Marketplace financial operation or paid event was not found")
                .extend_with(|_, extensions| {
                    extensions.set("code", "MARKETPLACE_FINANCIAL_NOT_FOUND")
                })
        }
        crate::MarketplaceFinancialOperatorError::Conflict(_) => async_graphql::Error::new(
            "Marketplace financial operation requires reconciliation or is not safely retryable",
        )
        .extend_with(|_, extensions| {
            extensions.set("code", "MARKETPLACE_FINANCIAL_OPERATOR_CONFLICT")
        }),
        crate::MarketplaceFinancialOperatorError::Database(_) => async_graphql::Error::new(
            "Marketplace financial operator storage is unavailable",
        )
        .extend_with(|_, extensions| {
            extensions.set("code", "MARKETPLACE_FINANCIAL_STORAGE_UNAVAILABLE")
        }),
        crate::MarketplaceFinancialOperatorError::Inbox(error) => {
            let (message, code) = if error.retryable() {
                (
                    "Marketplace financial recovery is temporarily unavailable",
                    "MARKETPLACE_FINANCIAL_RECOVERY_UNAVAILABLE",
                )
            } else {
                (
                    "Marketplace financial recovery requires operator review",
                    "MARKETPLACE_FINANCIAL_RECONCILIATION_REQUIRED",
                )
            };
            async_graphql::Error::new(message)
                .extend_with(|_, extensions| extensions.set("code", code))
        }
    }
}

impl From<crate::MarketplaceFinancialOperationOperatorView>
    for MarketplaceFinancialOperationGql
{
    fn from(value: crate::MarketplaceFinancialOperationOperatorView) -> Self {
        Self {
            checkout_operation_id: value.checkout_operation_id,
            order_id: value.order_id,
            payment_collection_id: value.payment_collection_id,
            currency_code: value.currency_code,
            status: value.status,
            stage: value.stage,
            attempt_count: value.attempt_count,
            ledger_transaction_id: value.ledger_transaction_id,
            ledger_debit_total_amount: value.ledger_debit_total_amount,
            ledger_credit_total_amount: value.ledger_credit_total_amount,
            last_error_code: value.last_error_code,
            last_error_message: value.last_error_message,
            created_at: value.created_at,
            updated_at: value.updated_at,
            completed_at: value.completed_at,
        }
    }
}

impl From<crate::MarketplacePaidEventOperatorView> for MarketplacePaidEventGql {
    fn from(value: crate::MarketplacePaidEventOperatorView) -> Self {
        Self {
            id: value.id,
            event_source: value.event_source,
            event_id: value.event_id,
            checkout_operation_id: value.checkout_operation_id,
            order_id: value.order_id,
            payment_collection_id: value.payment_collection_id,
            captured_at: value.captured_at,
            currency_code: value.currency_code,
            captured_amount: value.captured_amount,
            status: value.status,
            attempt_count: value.attempt_count,
            last_error_code: value.last_error_code,
            last_error_message: value.last_error_message,
            created_at: value.created_at,
            updated_at: value.updated_at,
            processed_at: value.processed_at,
        }
    }
}

impl From<crate::MarketplacePaidEventSweepReport> for MarketplaceFinancialSweepGql {
    fn from(value: crate::MarketplacePaidEventSweepReport) -> Self {
        Self {
            selected: value.selected as i32,
            processed: value.processed as i32,
            retryable_failures: value.retryable_failures as i32,
            operator_review_failures: value.operator_review_failures as i32,
            failures: value
                .failures
                .into_iter()
                .map(|failure| MarketplaceFinancialSweepFailureGql {
                    inbox_id: failure.inbox_id,
                    retryable: failure.retryable,
                })
                .collect(),
        }
    }
}
