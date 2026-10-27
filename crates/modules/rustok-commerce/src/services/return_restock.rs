//! Operator-decided restock of returned goods at return completion.
//!
//! Policy: nothing returns to stock by default — whether a returned unit is sellable again is a
//! physical-inspection decision. The operator opts in through `metadata.restock` of the
//! complete-return request:
//!
//! * `"restock": "all"` — every item of the return, in its returned quantity;
//! * `"restock": [{"return_item_id": "<uuid>", "quantity": <n>}, ...]` — selected items, each at
//!   most its returned quantity.
//!
//! Inventory owns the stock movement and its idempotency ledger
//! (`rustok_inventory::return_restock`); this module only parses the decision and maps it onto
//! the completed return's items.

use std::collections::BTreeMap;

use rustok_api::PortError;
use rustok_inventory::return_restock::{
    ReturnRestockError, ReturnRestockLine, ReturnRestockOutcome, restock_returned_items,
};
use rustok_order::dto::OrderReturnResponse;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use uuid::Uuid;

use super::post_order::{PostOrderOrchestrationError, PostOrderOrchestrationResult};

/// Key of the operator decision inside the complete-return `metadata`.
pub(crate) const RESTOCK_METADATA_KEY: &str = "restock";

/// What the operator decided for the returned goods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RestockDecision {
    /// Default: no stock movement.
    None,
    /// Every return item, in its returned quantity.
    All,
    /// Selected return items: `(return_item_id, quantity)`.
    Items(Vec<(Uuid, i32)>),
}

fn invalid(message: impl Into<String>) -> PostOrderOrchestrationError {
    PostOrderOrchestrationError::Validation(message.into())
}

/// Reads `metadata.restock`. A missing key or `null` means "do not restock"; any other shape
/// that is not `"all"` or a well-formed item list is rejected before any side effect.
pub(crate) fn parse_restock_decision(
    metadata: &Value,
) -> PostOrderOrchestrationResult<RestockDecision> {
    let Some(raw) = metadata.get(RESTOCK_METADATA_KEY) else {
        return Ok(RestockDecision::None);
    };
    match raw {
        Value::Null => Ok(RestockDecision::None),
        Value::String(value) if value.trim().eq_ignore_ascii_case("all") => {
            Ok(RestockDecision::All)
        }
        Value::Array(entries) => {
            let mut seen = BTreeMap::<Uuid, i32>::new();
            for entry in entries {
                let return_item_id = entry
                    .get("return_item_id")
                    .and_then(Value::as_str)
                    .and_then(|value| Uuid::parse_str(value).ok())
                    .filter(|value| !value.is_nil())
                    .ok_or_else(|| {
                        invalid("metadata.restock[].return_item_id must be a non-nil UUID")
                    })?;
                let quantity = entry
                    .get("quantity")
                    .and_then(Value::as_i64)
                    .and_then(|value| i32::try_from(value).ok())
                    .filter(|value| *value > 0)
                    .ok_or_else(|| {
                        invalid("metadata.restock[].quantity must be a positive integer")
                    })?;
                if seen.insert(return_item_id, quantity).is_some() {
                    return Err(invalid(format!(
                        "metadata.restock lists return item {return_item_id} more than once"
                    )));
                }
            }
            Ok(RestockDecision::Items(seen.into_iter().collect()))
        }
        _ => Err(invalid(
            "metadata.restock must be \"all\" or a list of {return_item_id, quantity}",
        )),
    }
}

/// Maps the decision onto the completed return's items.
pub(crate) fn plan_restock_lines(
    order_return: &OrderReturnResponse,
    decision: &RestockDecision,
) -> PostOrderOrchestrationResult<Vec<ReturnRestockLine>> {
    let line_for = |item: &rustok_order::dto::OrderReturnItemResponse, quantity: i32| {
        ReturnRestockLine {
            return_item_id: item.id,
            return_id: order_return.id,
            order_line_item_id: item.line_item_id,
            quantity,
        }
    };
    match decision {
        RestockDecision::None => Ok(Vec::new()),
        RestockDecision::All => Ok(order_return
            .items
            .iter()
            .filter(|item| item.quantity > 0)
            .map(|item| line_for(item, item.quantity))
            .collect()),
        RestockDecision::Items(selected) => selected
            .iter()
            .map(|(return_item_id, quantity)| {
                let item = order_return
                    .items
                    .iter()
                    .find(|item| item.id == *return_item_id)
                    .ok_or_else(|| {
                        invalid(format!(
                            "metadata.restock references return item {return_item_id}, \
                             which does not belong to return {}",
                            order_return.id
                        ))
                    })?;
                if *quantity > item.quantity {
                    return Err(invalid(format!(
                        "metadata.restock quantity {quantity} exceeds the returned quantity {} \
                         of return item {return_item_id}",
                        item.quantity
                    )));
                }
                Ok(line_for(item, *quantity))
            })
            .collect(),
    }
}

/// Restocks the operator-selected items of a completed return. Idempotent: inventory's ledger
/// skips return items that were already restocked, so retries and replays are safe.
pub(crate) async fn apply_restock_decision(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    order_return: &OrderReturnResponse,
    decision: &RestockDecision,
) -> PostOrderOrchestrationResult<ReturnRestockOutcome> {
    let lines = plan_restock_lines(order_return, decision)?;
    if lines.is_empty() {
        return Ok(ReturnRestockOutcome::default());
    }
    let outcome = restock_returned_items(db, tenant_id, &lines)
        .await
        .map_err(map_restock_error)?;
    tracing::info!(
        boundary = "commerce_return_restock",
        %tenant_id,
        return_id = %order_return.id,
        restocked = outcome.restocked.len(),
        already_restocked = outcome.already_restocked.len(),
        untracked = outcome.untracked.len(),
        "return restock applied"
    );
    Ok(outcome)
}

fn map_restock_error(error: ReturnRestockError) -> PostOrderOrchestrationError {
    let error = match error {
        // The ledger or level is inconsistent: operator action, not a blind retry.
        ReturnRestockError::Rejected(message) => {
            tracing::error!(%message, "return restock rejected by inventory");
            PortError::invariant_violation(
                "inventory.return_restock_rejected",
                "inventory refused the return restock",
            )
        }
        ReturnRestockError::Database(error) => {
            tracing::warn!(%error, "return restock storage failure");
            PortError::unavailable(
                "inventory.return_restock_unavailable",
                "inventory storage is temporarily unavailable",
            )
        }
    };
    PostOrderOrchestrationError::OwnerPort {
        owner: "inventory",
        error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rustok_order::dto::OrderReturnItemResponse;
    use serde_json::json;

    fn item(id: Uuid, line: Uuid, quantity: i32) -> OrderReturnItemResponse {
        OrderReturnItemResponse {
            id,
            tenant_id: Uuid::new_v4(),
            return_id: Uuid::new_v4(),
            order_id: Uuid::new_v4(),
            line_item_id: line,
            quantity,
            reason: None,
            note: None,
            metadata: json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn order_return(items: Vec<OrderReturnItemResponse>) -> OrderReturnResponse {
        OrderReturnResponse {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            order_id: Uuid::new_v4(),
            reason: None,
            note: None,
            status: "completed".to_string(),
            resolution_type: None,
            refund_id: None,
            order_change_id: None,
            metadata: json!({}),
            items,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
            cancelled_at: None,
        }
    }

    #[test]
    fn absent_or_null_restock_means_no_movement() {
        assert_eq!(
            parse_restock_decision(&json!({})).expect("parse"),
            RestockDecision::None
        );
        assert_eq!(
            parse_restock_decision(&json!({"restock": null})).expect("parse"),
            RestockDecision::None
        );
    }

    #[test]
    fn restock_all_and_item_lists_parse() {
        assert_eq!(
            parse_restock_decision(&json!({"restock": "ALL"})).expect("parse"),
            RestockDecision::All
        );
        let id = Uuid::new_v4();
        assert_eq!(
            parse_restock_decision(
                &json!({"restock": [{"return_item_id": id.to_string(), "quantity": 2}]})
            )
            .expect("parse"),
            RestockDecision::Items(vec![(id, 2)])
        );
    }

    #[test]
    fn malformed_restock_is_rejected() {
        let id = Uuid::new_v4().to_string();
        for bad in [
            json!({"restock": "some"}),
            json!({"restock": true}),
            json!({"restock": [{"return_item_id": "nope", "quantity": 1}]}),
            json!({"restock": [{"return_item_id": id, "quantity": 0}]}),
            json!({"restock": [{"return_item_id": id}]}),
            json!({"restock": [
                {"return_item_id": id, "quantity": 1},
                {"return_item_id": id, "quantity": 1}
            ]}),
        ] {
            assert!(parse_restock_decision(&bad).is_err(), "{bad} must be rejected");
        }
    }

    #[test]
    fn plan_restocks_every_item_for_all() {
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        let ret = order_return(vec![
            item(Uuid::new_v4(), a, 2),
            item(Uuid::new_v4(), b, 1),
        ]);
        let lines = plan_restock_lines(&ret, &RestockDecision::All).expect("plan");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].order_line_item_id, a);
        assert_eq!(lines[0].quantity, 2);
        assert!(lines.iter().all(|line| line.return_id == ret.id));
    }

    #[test]
    fn plan_rejects_foreign_items_and_quantities_above_the_return() {
        let item_id = Uuid::new_v4();
        let ret = order_return(vec![item(item_id, Uuid::new_v4(), 2)]);
        let foreign = RestockDecision::Items(vec![(Uuid::new_v4(), 1)]);
        assert!(plan_restock_lines(&ret, &foreign).is_err());
        let too_many = RestockDecision::Items(vec![(item_id, 3)]);
        assert!(plan_restock_lines(&ret, &too_many).is_err());
        let partial = RestockDecision::Items(vec![(item_id, 1)]);
        let lines = plan_restock_lines(&ret, &partial).expect("plan");
        assert_eq!(lines[0].quantity, 1);
    }

    #[test]
    fn default_decision_plans_nothing() {
        let ret = order_return(vec![item(Uuid::new_v4(), Uuid::new_v4(), 1)]);
        assert!(
            plan_restock_lines(&ret, &RestockDecision::None)
                .expect("plan")
                .is_empty()
        );
    }
}
