use async_graphql::{Context, Result};
use rustok_order::OrderService;
use rustok_payment::PaymentService;
use uuid::Uuid;

use crate::graphql_runtime::payment_orchestration_from_context;

use super::super::types::CompleteOrderReturnRefundInputObject;
use super::helpers::{parse_decimal, parse_optional_metadata};

/// Build a refund-backed return completion without bypassing the configured
/// payment provider registry.
pub(crate) async fn build_provider_refund_resolution_return_completion(
    ctx: &Context<'_>,
    db: &sea_orm::DatabaseConnection,
    order_service: &OrderService,
    tenant_id: Uuid,
    return_id: Uuid,
    mut complete_input: crate::dto::CompleteOrderReturnInput,
    refund_input: CompleteOrderReturnRefundInputObject,
) -> Result<crate::dto::CompleteOrderReturnInput> {
    if complete_input.refund_id.is_some() || complete_input.order_change_id.is_some() {
        return Err(async_graphql::Error::new(
            "refund helper cannot be combined with explicit refund_id or order_change_id",
        ));
    }
    if complete_input
        .resolution_type
        .as_deref()
        .map(|value| value.trim().eq_ignore_ascii_case("refund"))
        == Some(false)
    {
        return Err(async_graphql::Error::new(
            "refund helper requires resolution_type to be omitted or `refund`",
        ));
    }

    let existing_return = order_service.get_return(tenant_id, return_id).await?;
    let payment_service = PaymentService::new(db.clone());
    let collection_id = match refund_input.payment_collection_id {
        Some(collection_id) => {
            let collection = payment_service
                .get_collection(tenant_id, collection_id)
                .await?;
            if collection.order_id != Some(existing_return.order_id) {
                return Err(async_graphql::Error::new(format!(
                    "payment collection {collection_id} is not attached to order {}",
                    existing_return.order_id
                )));
            }
            collection_id
        }
        None => payment_service
            .find_latest_collection_by_order(tenant_id, existing_return.order_id)
            .await?
            .map(|collection| collection.id)
            .ok_or_else(|| {
                async_graphql::Error::new(format!(
                    "order {} has no payment collection for return refund",
                    existing_return.order_id
                ))
            })?,
    };

    let should_complete = refund_input.complete.unwrap_or(false);
    let payment_orchestration = payment_orchestration_from_context(ctx, db.clone());
    let refund = payment_orchestration
        .create_refund_idempotent(
            tenant_id,
            collection_id,
            format!("order_return:{return_id}:refund"),
            crate::dto::CreateRefundInput {
                amount: parse_decimal(&refund_input.amount)?,
                reason: refund_input.reason,
                metadata: parse_optional_metadata(refund_input.metadata.as_deref())?,
            },
        )
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    let refund = if should_complete {
        payment_orchestration
            .complete_refund(
                tenant_id,
                refund.id,
                crate::dto::CompleteRefundInput {
                    metadata: serde_json::json!({
                        "source": "order_return_completion",
                        "return_id": return_id,
                    }),
                },
            )
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?
    } else {
        refund
    };

    complete_input.resolution_type = Some("refund".to_string());
    complete_input.refund_id = Some(refund.id);
    Ok(complete_input)
}
