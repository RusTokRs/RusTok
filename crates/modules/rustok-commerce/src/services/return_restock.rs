//! Bind the operator's return-inspection decision to saved owner identities before effects.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use rustok_api::{PortCallPolicy, PortContext, PortError};
use rustok_inventory::return_restock::{
    MAX_RETURN_RESTOCK_ITEMS, ReturnRestockLine, ReturnRestockOutcome, restock_returned_items,
};
use rustok_order::{
    OrderLineInventoryBindingsSnapshot, OrderReadPort, OrderReturnItemResponse,
    OrderReturnResponse, ReadOrderLineInventoryBindingsRequest,
};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use uuid::Uuid;

use super::post_order::{PostOrderOrchestrationError, PostOrderOrchestrationResult};

const MAX_RESTOCK_BUDGET: Duration = Duration::from_secs(30);

type ReturnItemFacts = BTreeMap<Uuid, (Uuid, i32)>;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) enum RestockDecision {
    #[default]
    None,
    All,
    Items(Vec<(Uuid, i32)>),
}

/// An immutable, fully correlated projection of owner facts and the admitted operator intent.
/// It is not a stock receipt, a new identity, or authority to compensate another owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedReturnRestock {
    tenant_id: Uuid,
    order_id: Uuid,
    return_id: Uuid,
    // None is the explicit no-goods decision, not missing owner data.
    return_items: Option<ReturnItemFacts>,
    lines: Vec<ReturnRestockLine>,
}

fn invalid(message: impl Into<String>) -> PostOrderOrchestrationError {
    PostOrderOrchestrationError::Validation(message.into())
}

fn owner_error(owner: &'static str, error: PortError) -> PostOrderOrchestrationError {
    PostOrderOrchestrationError::OwnerPort { owner, error }
}

fn binding_conflict() -> PostOrderOrchestrationError {
    owner_error(
        "rustok_order",
        PortError::invariant_violation(
            "order.return_restock_binding_conflict",
            "saved return and order inventory bindings disagree; operator reconciliation is required",
        ),
    )
}

/// Absent/null or an explicit empty selection means no goods were selected for restock.
/// Nothing converts an unavailable owner/stock lookup into this explicit no-movement decision.
pub(crate) fn parse_restock_decision(
    metadata: &Value,
) -> PostOrderOrchestrationResult<RestockDecision> {
    if !metadata.is_null() && !metadata.is_object() {
        return Err(invalid("metadata must be an object"));
    }
    let Some(value) = metadata.get("restock").filter(|value| !value.is_null()) else {
        return Ok(RestockDecision::None);
    };
    if value
        .as_str()
        .is_some_and(|value| value.eq_ignore_ascii_case("all"))
    {
        return Ok(RestockDecision::All);
    }
    let entries = value.as_array().ok_or_else(|| {
        invalid("metadata.restock must be \"all\" or an array of {return_item_id, quantity}")
    })?;
    if entries.len() > MAX_RETURN_RESTOCK_ITEMS {
        return Err(invalid(
            "metadata.restock supports at most 1024 return items",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut selected = Vec::with_capacity(entries.len());
    for entry in entries {
        let fields = entry
            .as_object()
            .ok_or_else(|| invalid("each restock item must be an object"))?;
        if fields.len() != 2
            || !fields.contains_key("return_item_id")
            || !fields.contains_key("quantity")
        {
            return Err(invalid(
                "each restock item requires only return_item_id and quantity",
            ));
        }
        let id = fields["return_item_id"]
            .as_str()
            .and_then(|value| Uuid::parse_str(value).ok())
            .filter(|id| !id.is_nil())
            .ok_or_else(|| invalid("restock return_item_id must be a non-nil UUID"))?;
        let quantity = fields["quantity"]
            .as_i64()
            .and_then(|value| i32::try_from(value).ok())
            .filter(|quantity| *quantity > 0)
            .ok_or_else(|| invalid("restock quantity must be a positive 32-bit integer"))?;
        if !seen.insert(id) {
            return Err(invalid("metadata.restock must not repeat return_item_id"));
        }
        selected.push((id, quantity));
    }
    Ok(RestockDecision::Items(selected))
}

fn validate_return_parent(
    tenant_id: Uuid,
    order_return: &OrderReturnResponse,
) -> PostOrderOrchestrationResult<()> {
    if tenant_id.is_nil()
        || order_return.tenant_id != tenant_id
        || order_return.id.is_nil()
        || order_return.order_id.is_nil()
        || !matches!(order_return.status.as_str(), "pending" | "completed")
    {
        return Err(binding_conflict());
    }
    Ok(())
}

fn explicit_no_goods(decision: &RestockDecision) -> bool {
    match decision {
        RestockDecision::None => true,
        RestockDecision::Items(items) => items.is_empty(),
        RestockDecision::All => false,
    }
}

fn return_item_facts(
    tenant_id: Uuid,
    order_return: &OrderReturnResponse,
) -> PostOrderOrchestrationResult<ReturnItemFacts> {
    validate_return_parent(tenant_id, order_return)?;
    if order_return.items.len() > MAX_RETURN_RESTOCK_ITEMS {
        return Err(owner_error(
            "rustok_order",
            PortError::invariant_violation(
                "order.return_restock_item_limit",
                "return stock plans support at most 1024 return items; reconcile the admitted decision without changing mutation identities",
            ),
        ));
    }
    let mut facts = BTreeMap::new();
    let mut line_ids = BTreeSet::new();
    for item in &order_return.items {
        if item.id.is_nil()
            || item.line_item_id.is_nil()
            || item.tenant_id != tenant_id
            || item.return_id != order_return.id
            || item.order_id != order_return.order_id
            || item.quantity <= 0
            || !line_ids.insert(item.line_item_id)
            || facts
                .insert(item.id, (item.line_item_id, item.quantity))
                .is_some()
        {
            return Err(binding_conflict());
        }
    }
    Ok(facts)
}

fn selected_items<'a>(
    tenant_id: Uuid,
    order_return: &'a OrderReturnResponse,
    decision: &RestockDecision,
) -> PostOrderOrchestrationResult<Vec<(&'a OrderReturnItemResponse, i32)>> {
    validate_return_parent(tenant_id, order_return)?;
    if explicit_no_goods(decision) {
        return Ok(Vec::new());
    }
    return_item_facts(tenant_id, order_return)?;
    let by_id = order_return
        .items
        .iter()
        .map(|item| (item.id, item))
        .collect::<BTreeMap<_, _>>();
    let selected = match decision {
        RestockDecision::None => Vec::new(),
        RestockDecision::All => order_return
            .items
            .iter()
            .map(|item| (item, item.quantity))
            .collect(),
        RestockDecision::Items(entries) => {
            if entries.len() > MAX_RETURN_RESTOCK_ITEMS {
                return Err(invalid("restock supports at most 1024 return items"));
            }
            let mut seen = BTreeSet::new();
            let mut selected = Vec::with_capacity(entries.len());
            for (id, quantity) in entries {
                let item = by_id.get(id).copied().ok_or_else(|| {
                    invalid("metadata.restock references an item outside this return")
                })?;
                if *quantity <= 0 || *quantity > item.quantity || !seen.insert(*id) {
                    return Err(invalid(
                        "restock requires distinct return items and quantities within the returned amount",
                    ));
                }
                selected.push((item, *quantity));
            }
            selected
        }
    };
    Ok(selected)
}

fn plan_restock_lines(
    tenant_id: Uuid,
    order_return: &OrderReturnResponse,
    decision: &RestockDecision,
    snapshot: &OrderLineInventoryBindingsSnapshot,
) -> PostOrderOrchestrationResult<PreparedReturnRestock> {
    let selected = selected_items(tenant_id, order_return, decision)?;
    if snapshot.tenant_id != tenant_id || snapshot.order_id != order_return.order_id {
        return Err(binding_conflict());
    }
    let expected = selected
        .iter()
        .map(|(item, _)| item.line_item_id)
        .collect::<BTreeSet<_>>();
    if snapshot.items.len() != expected.len() {
        return Err(binding_conflict());
    }
    let mut bindings = BTreeMap::new();
    for binding in &snapshot.items {
        if !expected.contains(&binding.order_line_item_id)
            || binding.quantity <= 0
            || binding.variant_id.is_some_and(|id| id.is_nil())
            || bindings
                .insert(binding.order_line_item_id, binding)
                .is_some()
        {
            return Err(binding_conflict());
        }
    }
    let mut lines = Vec::with_capacity(selected.len());
    for (item, quantity) in selected {
        let binding = bindings
            .get(&item.line_item_id)
            .ok_or_else(binding_conflict)?;
        if item.quantity > binding.quantity {
            return Err(binding_conflict());
        }
        lines.push(ReturnRestockLine {
            return_item_id: item.id,
            return_id: order_return.id,
            order_line_item_id: item.line_item_id,
            variant_id: binding.variant_id,
            quantity,
        });
    }
    Ok(PreparedReturnRestock {
        tenant_id,
        order_id: order_return.order_id,
        return_id: order_return.id,
        return_items: if explicit_no_goods(decision) {
            None
        } else {
            Some(return_item_facts(tenant_id, order_return)?)
        },
        lines,
    })
}

/// Uses the caller's selected capability exactly once for a nonempty selection. The batch
/// contains only selected saved lines; it does not fetch a whole priced Order or fresh Catalog.
pub(crate) async fn prepare_restock_plan(
    order_reads: &dyn OrderReadPort,
    mut context: PortContext,
    tenant_id: Uuid,
    order_return: &OrderReturnResponse,
    decision: &RestockDecision,
) -> PostOrderOrchestrationResult<PreparedReturnRestock> {
    let selected = selected_items(tenant_id, order_return, decision)?;
    let line_ids = selected
        .iter()
        .map(|(item, _)| item.line_item_id)
        .collect::<Vec<_>>();
    if line_ids.is_empty() {
        return plan_restock_lines(
            tenant_id,
            order_return,
            decision,
            &OrderLineInventoryBindingsSnapshot {
                tenant_id,
                order_id: order_return.order_id,
                items: Vec::new(),
            },
        );
    }
    let budget = call_budget(&context, PortCallPolicy::read())
        .map_err(|error| owner_error("rustok_order", error))?;
    if Uuid::parse_str(context.tenant_id.trim()).ok() != Some(tenant_id) {
        return Err(binding_conflict());
    }
    context.deadline_ms = Some(budget.as_millis() as u64);
    let snapshot = tokio::time::timeout(
        budget,
        order_reads.read_order_line_inventory_bindings(
            context,
            ReadOrderLineInventoryBindingsRequest {
                order_id: order_return.order_id,
                order_line_item_ids: line_ids,
            },
        ),
    )
    .await
    .map_err(|_| {
        owner_error(
            "rustok_order",
            PortError::timeout(
                "order.return_restock_binding_timeout",
                "saved return inventory bindings are unavailable within the call budget",
            ),
        )
    })?
    .map_err(|error| owner_error("rustok_order", error))?;
    plan_restock_lines(tenant_id, order_return, decision, &snapshot)
}

impl PreparedReturnRestock {
    fn validate_completed(
        &self,
        completed: &OrderReturnResponse,
    ) -> PostOrderOrchestrationResult<()> {
        if completed.status != "completed"
            || completed.id != self.return_id
            || completed.order_id != self.order_id
        {
            return Err(binding_conflict());
        }
        validate_return_parent(self.tenant_id, completed)?;
        if let Some(expected) = &self.return_items {
            if return_item_facts(self.tenant_id, completed)? != *expected {
                return Err(binding_conflict());
            }
        }
        Ok(())
    }
}

pub(crate) async fn apply_restock_plan(
    db: &DatabaseConnection,
    mut context: PortContext,
    completed: &OrderReturnResponse,
    plan: &PreparedReturnRestock,
) -> PostOrderOrchestrationResult<ReturnRestockOutcome> {
    plan.validate_completed(completed)?;
    if Uuid::parse_str(context.tenant_id.trim()).ok() != Some(plan.tenant_id) {
        return Err(binding_conflict());
    }
    if plan.lines.is_empty() {
        return Ok(ReturnRestockOutcome::default());
    }
    let budget = call_budget(&context, PortCallPolicy::write())
        .map_err(|error| owner_error("rustok_inventory", error))?;
    context.deadline_ms = Some(budget.as_millis() as u64);
    let outcome = tokio::time::timeout(
        budget,
        restock_returned_items(db, context, &plan.lines),
    )
    .await
    .map_err(|_| owner_error("rustok_inventory", PortError::timeout(
        "inventory.return_restock_timeout",
        "return stock outcome is unknown; recover using the same saved mutation identities",
    )))?
    .map_err(|error| owner_error("rustok_inventory", error))?;
    validate_restock_outcome(&plan.lines, &outcome)?;
    tracing::info!(
        boundary = "commerce_return_restock",
        restocked_count = outcome.restocked.len(),
        replay_count = outcome.already_restocked.len(),
        sku_less_count = outcome.untracked.len(),
        "return stock outcome correlated"
    );
    Ok(outcome)
}

fn call_budget(context: &PortContext, policy: PortCallPolicy) -> Result<Duration, PortError> {
    context.require_policy(policy)?;
    // require_policy rejects absent/zero deadlines before any future is created/polled.
    Ok(Duration::from_millis(context.deadline_ms.unwrap_or_default()).min(MAX_RESTOCK_BUDGET))
}

fn validate_restock_outcome(
    lines: &[ReturnRestockLine],
    outcome: &ReturnRestockOutcome,
) -> PostOrderOrchestrationResult<()> {
    let conflict = || {
        owner_error(
            "rustok_inventory",
            PortError::invariant_violation(
                "inventory.return_restock_outcome_conflict",
                "return stock outcome does not fully match the saved request; reconciliation is required",
            ),
        )
    };
    let total = outcome
        .restocked
        .len()
        .checked_add(outcome.already_restocked.len())
        .and_then(|count| count.checked_add(outcome.untracked.len()))
        .ok_or_else(conflict)?;
    if total != lines.len() || total > MAX_RETURN_RESTOCK_ITEMS {
        return Err(conflict());
    }
    let by_id = lines
        .iter()
        .map(|line| (line.return_item_id, line))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    for (id, quantity) in &outcome.restocked {
        let line = by_id.get(id).copied().ok_or_else(conflict)?;
        if line.variant_id.is_none() || *quantity != line.quantity || !seen.insert(*id) {
            return Err(conflict());
        }
    }
    for id in &outcome.already_restocked {
        let line = by_id.get(id).copied().ok_or_else(conflict)?;
        if line.variant_id.is_none() || !seen.insert(*id) {
            return Err(conflict());
        }
    }
    for id in &outcome.untracked {
        let line = by_id.get(id).copied().ok_or_else(conflict)?;
        if line.variant_id.is_some() || !seen.insert(*id) {
            return Err(conflict());
        }
    }
    if seen.len() != lines.len() {
        return Err(conflict());
    }
    Ok(())
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
        let tenant_id = Uuid::new_v4();
        let return_id = Uuid::new_v4();
        let order_id = Uuid::new_v4();
        let items = items
            .into_iter()
            .map(|mut item| {
                item.tenant_id = tenant_id;
                item.return_id = return_id;
                item.order_id = order_id;
                item
            })
            .collect();
        OrderReturnResponse {
            id: return_id,
            tenant_id,
            order_id,
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

    fn bindings(ret: &OrderReturnResponse) -> OrderLineInventoryBindingsSnapshot {
        OrderLineInventoryBindingsSnapshot {
            tenant_id: ret.tenant_id,
            order_id: ret.order_id,
            items: ret
                .items
                .iter()
                .map(|item| rustok_order::OrderLineInventoryBinding {
                    order_line_item_id: item.line_item_id,
                    variant_id: Some(Uuid::new_v4()),
                    quantity: item.quantity,
                })
                .collect(),
        }
    }

    fn plan(
        ret: &OrderReturnResponse,
        decision: &RestockDecision,
    ) -> PostOrderOrchestrationResult<PreparedReturnRestock> {
        let expected = selected_items(ret.tenant_id, ret, decision)?
            .iter()
            .map(|(item, _)| item.line_item_id)
            .collect::<BTreeSet<_>>();
        let mut snapshot = bindings(ret);
        snapshot
            .items
            .retain(|binding| expected.contains(&binding.order_line_item_id));
        plan_restock_lines(ret.tenant_id, ret, decision, &snapshot)
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
            assert!(
                parse_restock_decision(&bad).is_err(),
                "{bad} must be rejected"
            );
        }
    }

    #[test]
    fn plan_restocks_every_item_for_all() {
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        let ret = order_return(vec![item(Uuid::new_v4(), a, 2), item(Uuid::new_v4(), b, 1)]);
        let lines = plan(&ret, &RestockDecision::All).expect("plan").lines;
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
        assert!(plan(&ret, &foreign).is_err());
        let too_many = RestockDecision::Items(vec![(item_id, 3)]);
        assert!(plan(&ret, &too_many).is_err());
        let partial = RestockDecision::Items(vec![(item_id, 1)]);
        let lines = plan(&ret, &partial).expect("plan").lines;
        assert_eq!(lines[0].quantity, 1);
    }

    #[test]
    fn default_decision_plans_nothing() {
        let ret = order_return(vec![item(Uuid::new_v4(), Uuid::new_v4(), 1)]);
        assert!(
            plan(&ret, &RestockDecision::None)
                .expect("plan")
                .lines
                .is_empty()
        );
    }
    #[test]
    fn restock_admission_rejects_nil_unknown_fields_and_oversized_selections() {
        for value in [
            json!({"restock": [{"return_item_id": Uuid::nil(), "quantity": 1}]}),
            json!({"restock": [{"return_item_id": Uuid::new_v4(), "quantity": i64::from(i32::MAX) + 1}]}),
            json!({"restock": [{"return_item_id": Uuid::new_v4(), "quantity": 1, "location_id": Uuid::new_v4()}]}),
            json!(["restock", "all"]),
            json!({"restock": (0..=MAX_RETURN_RESTOCK_ITEMS).map(|_| json!({
                "return_item_id": Uuid::new_v4(), "quantity": 1,
            })).collect::<Vec<_>>()}),
        ] {
            assert!(parse_restock_decision(&value).is_err());
        }
    }

    #[test]
    fn return_parent_and_item_scope_are_required_before_binding_reads() {
        let ret = order_return(vec![item(Uuid::new_v4(), Uuid::new_v4(), 2)]);
        let mut bad = Vec::new();
        let mut value = ret.clone();
        value.tenant_id = Uuid::new_v4();
        bad.push(value);
        let mut value = ret.clone();
        value.order_id = Uuid::nil();
        bad.push(value);
        let mut value = ret.clone();
        value.items[0].tenant_id = Uuid::new_v4();
        bad.push(value);
        let mut value = ret.clone();
        value.items[0].return_id = Uuid::new_v4();
        bad.push(value);
        let mut value = ret.clone();
        value.items[0].order_id = Uuid::new_v4();
        bad.push(value);
        let mut value = ret.clone();
        value.items[0].quantity = 0;
        bad.push(value);
        let mut value = ret.clone();
        value.items.push(value.items[0].clone());
        bad.push(value);
        let mut value = ret.clone();
        value.status = "cancelled".into();
        bad.push(value);
        for changed in bad {
            assert!(selected_items(ret.tenant_id, &changed, &RestockDecision::All).is_err());
        }
    }

    #[test]
    fn whole_order_binding_response_must_match_without_missing_extra_or_duplicate_lines() {
        let ret = order_return(vec![item(Uuid::new_v4(), Uuid::new_v4(), 2)]);
        let saved = bindings(&ret);
        let mut bad = Vec::new();
        let mut value = saved.clone();
        value.tenant_id = Uuid::new_v4();
        bad.push(value);
        let mut value = saved.clone();
        value.order_id = Uuid::new_v4();
        bad.push(value);
        let mut value = saved.clone();
        value.items.clear();
        bad.push(value);
        let mut value = saved.clone();
        value.items.push(value.items[0].clone());
        bad.push(value);
        let mut value = saved.clone();
        value.items[0].order_line_item_id = Uuid::new_v4();
        bad.push(value);
        let mut value = saved.clone();
        value.items[0].variant_id = Some(Uuid::nil());
        bad.push(value);
        let mut value = saved.clone();
        value.items[0].quantity = 0;
        bad.push(value);
        let mut value = saved.clone();
        value.items[0].quantity = 1;
        bad.push(value);
        for changed in bad {
            assert!(
                plan_restock_lines(ret.tenant_id, &ret, &RestockDecision::All, &changed).is_err()
            );
        }
        let prepared = plan_restock_lines(ret.tenant_id, &ret, &RestockDecision::All, &saved)
            .expect("exact binding");
        assert_eq!(prepared.lines[0].variant_id, saved.items[0].variant_id);
        assert_eq!(prepared.lines[0].quantity, 2);
    }

    #[test]
    fn only_an_explicit_sku_less_binding_plans_untracked_stock() {
        let ret = order_return(vec![item(Uuid::new_v4(), Uuid::new_v4(), 1)]);
        let mut saved = bindings(&ret);
        saved.items[0].variant_id = None;
        let prepared = plan_restock_lines(ret.tenant_id, &ret, &RestockDecision::All, &saved)
            .expect("explicit SKU-less");
        assert_eq!(prepared.lines[0].variant_id, None);
        saved.items.clear();
        assert!(plan_restock_lines(ret.tenant_id, &ret, &RestockDecision::All, &saved).is_err());
    }

    #[test]
    fn completed_return_must_preserve_the_frozen_parent_and_item_facts() {
        let ret = order_return(vec![item(Uuid::new_v4(), Uuid::new_v4(), 2)]);
        let prepared = plan(&ret, &RestockDecision::All).expect("prepare");
        prepared.validate_completed(&ret).expect("exact completion");
        let mut bad = Vec::new();
        let mut value = ret.clone();
        value.id = Uuid::new_v4();
        bad.push(value);
        let mut value = ret.clone();
        value.tenant_id = Uuid::new_v4();
        bad.push(value);
        let mut value = ret.clone();
        value.order_id = Uuid::new_v4();
        bad.push(value);
        let mut value = ret.clone();
        value.items[0].quantity = 1;
        bad.push(value);
        let mut value = ret.clone();
        value.items[0].line_item_id = Uuid::new_v4();
        bad.push(value);
        let mut value = ret.clone();
        value.items.clear();
        bad.push(value);
        let mut value = ret.clone();
        value.status = "pending".into();
        bad.push(value);
        for changed in bad {
            assert!(prepared.validate_completed(&changed).is_err());
        }
    }

    #[test]
    fn inventory_outcome_requires_complete_exactly_once_quantity_and_tracking_correlation() {
        let ret = order_return(vec![item(Uuid::new_v4(), Uuid::new_v4(), 2)]);
        let prepared = plan(&ret, &RestockDecision::All).expect("prepare");
        let line = prepared.lines[0];
        for outcome in [
            ReturnRestockOutcome::default(),
            ReturnRestockOutcome {
                restocked: vec![(line.return_item_id, 1)],
                ..Default::default()
            },
            ReturnRestockOutcome {
                restocked: vec![(Uuid::new_v4(), 2)],
                ..Default::default()
            },
            ReturnRestockOutcome {
                untracked: vec![line.return_item_id],
                ..Default::default()
            },
            ReturnRestockOutcome {
                already_restocked: vec![line.return_item_id, line.return_item_id],
                ..Default::default()
            },
        ] {
            assert!(validate_restock_outcome(&prepared.lines, &outcome).is_err());
        }
        validate_restock_outcome(
            &prepared.lines,
            &ReturnRestockOutcome {
                restocked: vec![(line.return_item_id, 2)],
                ..Default::default()
            },
        )
        .expect("new movement");
        validate_restock_outcome(
            &prepared.lines,
            &ReturnRestockOutcome {
                already_restocked: vec![line.return_item_id],
                ..Default::default()
            },
        )
        .expect("exact replay");
    }

    #[test]
    fn sku_less_outcomes_cannot_claim_new_or_previously_restocked_units() {
        let ret = order_return(vec![item(Uuid::new_v4(), Uuid::new_v4(), 1)]);
        let mut saved = bindings(&ret);
        saved.items[0].variant_id = None;
        let prepared = plan_restock_lines(ret.tenant_id, &ret, &RestockDecision::All, &saved)
            .expect("prepare");
        let id = prepared.lines[0].return_item_id;
        validate_restock_outcome(
            &prepared.lines,
            &ReturnRestockOutcome {
                untracked: vec![id],
                ..Default::default()
            },
        )
        .expect("explicit no-stock binding");
        for outcome in [
            ReturnRestockOutcome {
                restocked: vec![(id, 1)],
                ..Default::default()
            },
            ReturnRestockOutcome {
                already_restocked: vec![id],
                ..Default::default()
            },
        ] {
            assert!(validate_restock_outcome(&prepared.lines, &outcome).is_err());
        }
    }

    struct SelectedReads {
        calls: std::sync::atomic::AtomicUsize,
        contexts: std::sync::Mutex<Vec<PortContext>>,
        response: Result<OrderLineInventoryBindingsSnapshot, PortError>,
    }

    #[async_trait::async_trait]
    impl OrderReadPort for SelectedReads {
        async fn read_order_line_inventory_bindings(
            &self,
            context: PortContext,
            request: ReadOrderLineInventoryBindingsRequest,
        ) -> Result<OrderLineInventoryBindingsSnapshot, PortError> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if let Ok(snapshot) = &self.response {
                assert_eq!(request.order_id, snapshot.order_id);
                assert_eq!(request.order_line_item_ids.len(), snapshot.items.len());
            }
            self.contexts.lock().expect("contexts").push(context);
            self.response.clone()
        }
        async fn read_order_projection(
            &self,
            _: PortContext,
            _: rustok_order::ReadOrderProjectionRequest,
        ) -> Result<rustok_order::OrderResponse, PortError> {
            panic!("full/priced Order read is forbidden")
        }
        async fn list_order_projections(
            &self,
            _: PortContext,
            _: rustok_order::ListOrderProjectionsRequest,
        ) -> Result<rustok_order::OrderProjectionPage, PortError> {
            panic!("unrelated read")
        }
        async fn read_order_return_projection(
            &self,
            _: PortContext,
            _: rustok_order::ReadOrderReturnProjectionRequest,
        ) -> Result<OrderReturnResponse, PortError> {
            panic!("unrelated read")
        }
        async fn list_order_return_projections(
            &self,
            _: PortContext,
            _: rustok_order::ListOrderReturnProjectionsRequest,
        ) -> Result<rustok_order::OrderReturnProjectionPage, PortError> {
            panic!("unrelated read")
        }
        async fn read_order_change_projection(
            &self,
            _: PortContext,
            _: rustok_order::ReadOrderChangeProjectionRequest,
        ) -> Result<rustok_order::OrderChangeResponse, PortError> {
            panic!("unrelated read")
        }
        async fn list_order_change_projections(
            &self,
            _: PortContext,
            _: rustok_order::ListOrderChangeProjectionsRequest,
        ) -> Result<rustok_order::OrderChangeProjectionPage, PortError> {
            panic!("unrelated read")
        }
    }

    fn selected_reads(
        response: Result<OrderLineInventoryBindingsSnapshot, PortError>,
    ) -> SelectedReads {
        SelectedReads {
            calls: Default::default(),
            contexts: Default::default(),
            response,
        }
    }

    fn read_context(tenant_id: Uuid) -> PortContext {
        PortContext::new(
            tenant_id.to_string(),
            rustok_api::PortActor::user(Uuid::new_v4().to_string()),
            "fr",
            "return-stock-binding",
        )
        .with_channel("store-fr")
        .with_deadline(Duration::from_secs(3))
    }

    #[tokio::test]
    async fn preparation_uses_the_selected_batch_capability_once_with_trusted_context() {
        let ret = order_return(vec![item(Uuid::new_v4(), Uuid::new_v4(), 2)]);
        let saved = bindings(&ret);
        let selected = selected_reads(Ok(saved.clone()));
        let context = read_context(ret.tenant_id);
        let prepared = prepare_restock_plan(
            &selected,
            context.clone(),
            ret.tenant_id,
            &ret,
            &RestockDecision::All,
        )
        .await
        .expect("selected read");
        assert_eq!(prepared.lines[0].variant_id, saved.items[0].variant_id);
        assert_eq!(selected.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        let contexts = selected.contexts.lock().expect("contexts");
        assert_eq!(contexts[0].tenant_id, context.tenant_id);
        assert_eq!(contexts[0].actor.id, context.actor.id);
        assert_eq!(contexts[0].locale, "fr");
        assert_eq!(contexts[0].channel, context.channel);
        assert_eq!(contexts[0].correlation_id, context.correlation_id);
        assert_eq!(contexts[0].deadline_ms, Some(3000));
    }

    #[tokio::test]
    async fn selected_binding_failure_preserves_typed_cause_and_does_not_fall_back() {
        let ret = order_return(vec![item(Uuid::new_v4(), Uuid::new_v4(), 1)]);
        let cause = PortError::new(
            rustok_api::PortErrorKind::Unavailable,
            "order.binding_parked",
            "operator review",
            false,
        );
        let selected = selected_reads(Err(cause.clone()));
        let error = prepare_restock_plan(
            &selected,
            read_context(ret.tenant_id),
            ret.tenant_id,
            &ret,
            &RestockDecision::All,
        )
        .await
        .expect_err("owner refusal");
        match error {
            PostOrderOrchestrationError::OwnerPort { owner, error } => {
                assert_eq!(owner, "rustok_order");
                assert_eq!(error, cause);
            }
            other => panic!("unexpected error: {other:?}"),
        }
        assert_eq!(selected.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn absent_or_zero_budget_refuses_before_even_a_ready_selected_provider_is_polled() {
        let ret = order_return(vec![item(Uuid::new_v4(), Uuid::new_v4(), 1)]);
        let selected = selected_reads(Ok(bindings(&ret)));
        for deadline in [None, Some(0)] {
            let mut context = read_context(ret.tenant_id);
            context.deadline_ms = deadline;
            assert!(
                prepare_restock_plan(
                    &selected,
                    context,
                    ret.tenant_id,
                    &ret,
                    &RestockDecision::All
                )
                .await
                .is_err()
            );
        }
        assert_eq!(selected.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn no_restock_decision_makes_no_owner_binding_call() {
        let ret = order_return(vec![item(Uuid::new_v4(), Uuid::new_v4(), 1)]);
        let selected = selected_reads(Err(PortError::unavailable(
            "order.test_unavailable",
            "unavailable",
        )));
        let prepared = prepare_restock_plan(
            &selected,
            read_context(ret.tenant_id),
            ret.tenant_id,
            &ret,
            &RestockDecision::None,
        )
        .await
        .expect("explicit no movement");
        assert!(prepared.lines.is_empty());
        assert_eq!(selected.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }
    #[test]
    fn an_explicit_no_goods_decision_does_not_impose_stock_batch_bounds_on_financial_only_returns()
    {
        let ret = order_return(
            (0..=MAX_RETURN_RESTOCK_ITEMS)
                .map(|_| item(Uuid::new_v4(), Uuid::new_v4(), 1))
                .collect(),
        );
        for decision in [RestockDecision::None, RestockDecision::Items(Vec::new())] {
            let prepared =
                plan(&ret, &decision).expect("financial-only return does not invoke stock");
            assert!(prepared.lines.is_empty());
            assert!(prepared.return_items.is_none());
            prepared
                .validate_completed(&ret)
                .expect("completed parent scope remains mandatory");
        }
        assert!(plan(&ret, &RestockDecision::All).is_err());
    }
}
