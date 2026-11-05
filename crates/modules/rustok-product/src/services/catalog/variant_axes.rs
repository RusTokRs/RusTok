/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

//! The existing axes command's bounded planner, consistent preview and atomic application.
//! No alternative option/assignment model: all inputs resolve to canonical Variant EAV rows.

use super::*;
use sea_orm::TransactionTrait;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const PREVIEW_REQUIRED: &str = "product variant axes: preview required";
const STALE_PREVIEW: &str = "product variant axes: stale preview";
const INVALID_PLAN: &str = "product variant axes: unresolved combinations";
const LIMIT_EXCEEDED: &str = "product variant axes: synchronous limit exceeded";

/// Closed, literal-only refusal classes, like the existing document revision refusal. Never
/// forwards authored data or SQL through a generic CommerceError::Validation boundary.
pub(crate) fn reconfiguration_error_code(message: &str) -> Option<&'static str> {
    match message {
        PREVIEW_REQUIRED => Some("product.variant_axes_preview_required"),
        STALE_PREVIEW => Some("product.variant_axes_stale"),
        INVALID_PLAN => Some("product.variant_axes_conflict"),
        LIMIT_EXCEEDED => Some("product.variant_axes_limit_exceeded"),
        _ => None,
    }
}

fn refusal(message: &'static str) -> CommerceError {
    CommerceError::Validation(message.to_owned())
}

#[derive(Clone, Debug, Serialize, FromQueryResult)]
struct AssignmentRow {
    variant_id: Uuid,
    attribute_id: Uuid,
    value_id: Uuid,
    option_id: Option<Uuid>,
    detached: bool,
}

#[derive(Clone)]
struct AxisSnapshot {
    product: entities::product::Model,
    variants: Vec<entities::product_variant::Model>,
    axes: Vec<entities::product_variant_axis::Model>,
    allowed: Vec<entities::product_variant_axis_value::Model>,
    assignments: Vec<AssignmentRow>,
}

struct VariantPlan {
    variant_id: Uuid,
    identity: Option<String>,
    identity_changed: bool,
    /// Only changed target-axis values. Removed-axis EAV is deliberately not in this list.
    values_to_write: Vec<VariantAxisValueInput>,
}

struct AxisPlan {
    preview: VariantAxesPreviewResponse,
    configuration_changed: bool,
    variants: Vec<VariantPlan>,
}

#[derive(Default)]
struct Conflicts {
    entries: Vec<VariantAxesConflictResponse>,
    has_more: bool,
}

impl Conflicts {
    fn push(
        &mut self,
        kind: VariantAxesConflictKind,
        variant_ids: &[Uuid],
        attribute_id: Option<Uuid>,
        option_id: Option<Uuid>,
    ) {
        if self.entries.len() == MAX_VARIANT_AXIS_RECONFIGURATION_CONFLICTS {
            self.has_more = true;
            return;
        }
        self.entries.push(VariantAxesConflictResponse {
            kind,
            attribute_id,
            option_id,
            variant_ids: variant_ids
                .iter()
                .copied()
                .take(MAX_VARIANT_AXIS_CONFLICT_EXAMPLES)
                .collect(),
            has_more_variants: variant_ids.len() > MAX_VARIANT_AXIS_CONFLICT_EXAMPLES,
        });
    }
}

fn validate_input(input: &SetVariantAxesInput) -> CommerceResult<()> {
    if input.axes.len() > MAX_VARIANT_AXES
        || input.reassignments.len() > MAX_VARIANT_AXIS_RECONFIGURATION_VARIANTS
        || input
            .axes
            .iter()
            .any(|axis| axis.allowed_option_ids.len() > MAX_VARIANT_AXIS_VALUES)
        || input
            .reassignments
            .iter()
            .any(|row| row.axis_values.len() > MAX_VARIANT_AXES)
    {
        return Err(refusal(LIMIT_EXCEEDED));
    }
    if input.expected_state.as_ref().is_some_and(|token| {
        token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit())
    }) {
        return Err(CommerceError::Validation(
            "variant axes predecessor must be an owner preview token".to_owned(),
        ));
    }
    let mut variants = HashSet::new();
    if input
        .reassignments
        .iter()
        .any(|row| !variants.insert(row.variant_id))
    {
        return Err(CommerceError::Validation(
            "a variant can have only one explicit reconfiguration tuple".to_owned(),
        ));
    }
    input
        .validate()
        .map_err(|error| CommerceError::Validation(error.to_string()))
}

/// Caller already holds schema → Product locks. Limit+1 is a refusal, not a partial snapshot.
async fn load_snapshot(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product: entities::product::Model,
    input: &SetVariantAxesInput,
) -> CommerceResult<AxisSnapshot> {
    let variants = entities::product_variant::Entity::find()
        .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
        .filter(entities::product_variant::Column::ProductId.eq(product.id))
        .order_by_asc(entities::product_variant::Column::Id)
        .limit((MAX_VARIANT_AXIS_RECONFIGURATION_VARIANTS + 1) as u64)
        .all(txn)
        .await?;
    if variants.is_empty() {
        return Err(CommerceError::NoVariants);
    }
    if variants.len() > MAX_VARIANT_AXIS_RECONFIGURATION_VARIANTS {
        return Err(refusal(LIMIT_EXCEEDED));
    }
    let variant_ids = variants
        .iter()
        .map(|variant| variant.id)
        .collect::<HashSet<_>>();
    for row in &input.reassignments {
        if !variant_ids.contains(&row.variant_id) {
            return Err(CommerceError::VariantNotFound(row.variant_id));
        }
    }
    let axes = entities::product_variant_axis::Entity::find()
        .filter(entities::product_variant_axis::Column::TenantId.eq(tenant_id))
        .filter(entities::product_variant_axis::Column::ProductId.eq(product.id))
        .order_by_asc(entities::product_variant_axis::Column::Position)
        .order_by_asc(entities::product_variant_axis::Column::Id)
        .limit((MAX_VARIANT_AXES + 1) as u64)
        .all(txn)
        .await?;
    if axes.len() > MAX_VARIANT_AXES {
        return Err(refusal(LIMIT_EXCEEDED));
    }
    let allowed = if axes.is_empty() {
        Vec::new()
    } else {
        let values = entities::product_variant_axis_value::Entity::find()
            .filter(entities::product_variant_axis_value::Column::TenantId.eq(tenant_id))
            .filter(
                entities::product_variant_axis_value::Column::AxisId
                    .is_in(axes.iter().map(|axis| axis.id)),
            )
            .order_by_asc(entities::product_variant_axis_value::Column::AxisId)
            .order_by_asc(entities::product_variant_axis_value::Column::Position)
            .order_by_asc(entities::product_variant_axis_value::Column::OptionId)
            .limit((MAX_VARIANT_AXES * MAX_VARIANT_AXIS_VALUES + 1) as u64)
            .all(txn)
            .await?;
        if values.len() > MAX_VARIANT_AXES * MAX_VARIANT_AXIS_VALUES {
            return Err(refusal(LIMIT_EXCEEDED));
        }
        values
    };
    let attribute_ids = axes
        .iter()
        .map(|axis| axis.attribute_id)
        .chain(input.axes.iter().map(|axis| axis.attribute_id))
        .collect::<BTreeSet<_>>();
    let assignments = if attribute_ids.is_empty() {
        Vec::new()
    } else {
        let placeholders = (0..attribute_ids.len())
            .map(|index| format!("${}", index + 3))
            .collect::<Vec<_>>()
            .join(", ");
        let mut values = vec![tenant_id.into(), product.id.into()];
        values.extend(attribute_ids.into_iter().map(Into::into));
        // Normally one row per select attribute. A small bounded surplus lets preview report
        // malformed duplicate option links; a huge corrupt/multi-select source fails closed.
        let maximum = MAX_VARIANT_AXIS_RECONFIGURATION_VARIANTS * MAX_VARIANT_AXES * 4;
        let rows = AssignmentRow::find_by_statement(Statement::from_sql_and_values(
            txn.get_database_backend(),
            format!(
                "SELECT value.variant_id, value.attribute_id, value.id AS value_id, \
                 selected.option_id, (value.detached_at IS NOT NULL) AS detached \
                 FROM product_variant_attribute_values value \
                 JOIN product_variants variant ON variant.tenant_id = value.tenant_id \
                   AND variant.id = value.variant_id \
                 LEFT JOIN product_variant_attribute_value_options selected \
                   ON selected.tenant_id = value.tenant_id AND selected.value_id = value.id \
                 WHERE value.tenant_id = $1 AND variant.product_id = $2 \
                   AND value.attribute_id IN ({placeholders}) \
                 ORDER BY value.variant_id, value.attribute_id, value.id, selected.option_id \
                 LIMIT {}",
                maximum + 1,
            ),
            values,
        ))
        .all(txn)
        .await?;
        if rows.len() > maximum {
            return Err(refusal(LIMIT_EXCEEDED));
        }
        rows
    };
    Ok(AxisSnapshot {
        product,
        variants,
        axes,
        allowed,
        assignments,
    })
}

fn canonical_values(values: &[VariantAxisValueInput]) -> Vec<VariantAxisValueInput> {
    let mut values = values.to_vec();
    values.sort_by_key(|value| (value.attribute_id, value.option_id));
    values
}

fn configuration_changed(
    snapshot: &AxisSnapshot,
    input: &SetVariantAxesInput,
) -> CommerceResult<bool> {
    if snapshot.axes.len() != input.axes.len() {
        return Ok(true);
    }
    for (index, axis) in input.axes.iter().enumerate() {
        let position = resolved_axis_position(axis.position, index)?;
        let Some(current) = snapshot
            .axes
            .iter()
            .find(|current| current.attribute_id == axis.attribute_id)
        else {
            return Ok(true);
        };
        if current.position != position {
            return Ok(true);
        }
        let current_values = snapshot
            .allowed
            .iter()
            .filter(|value| value.axis_id == current.id)
            .map(|value| (value.option_id, value.position))
            .collect::<Vec<_>>();
        let proposed_values = axis
            .allowed_option_ids
            .iter()
            .enumerate()
            .map(|(index, option_id)| Ok((*option_id, resolved_axis_position(0, index)?)))
            .collect::<CommerceResult<Vec<_>>>()?;
        if current_values != proposed_values {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Framework/transport independent and bounded by validate_input/load_snapshot. The same plan
/// drives preview and application; no UI-local collision policy can authorize owner mutation.
fn plan(snapshot: &AxisSnapshot, input: &SetVariantAxesInput) -> CommerceResult<AxisPlan> {
    let target_axes = input
        .axes
        .iter()
        .map(|axis| (axis.attribute_id, axis))
        .collect::<BTreeMap<_, _>>();
    let current_axes = snapshot
        .axes
        .iter()
        .map(|axis| axis.attribute_id)
        .collect::<HashSet<_>>();
    let explicit = input
        .reassignments
        .iter()
        .map(|row| (row.variant_id, row))
        .collect::<BTreeMap<_, _>>();
    let mut source = BTreeMap::<Uuid, Vec<VariantAxisValueInput>>::new();
    for row in &snapshot.assignments {
        if !row.detached
            && let Some(option_id) = row.option_id
        {
            source
                .entry(row.variant_id)
                .or_default()
                .push(VariantAxisValueInput {
                    attribute_id: row.attribute_id,
                    option_id,
                });
        }
    }
    let membership = snapshot
        .variants
        .iter()
        .map(|variant| (variant.id, &variant.sku, &variant.combination_identity))
        .collect::<Vec<_>>();
    // Include the entire proposal, not only the old state. A token cannot approve later tuple
    // edits. Serialization of expected_state is intentionally excluded.
    let bytes = serde_json::to_vec(&(
        "product.variant-axes.preview.v1",
        snapshot.product.tenant_id,
        snapshot.product.id,
        snapshot.product.revision,
        &snapshot.axes,
        &snapshot.allowed,
        &membership,
        &snapshot.assignments,
        &input.axes,
        &input.reassignments,
    ))
    .map_err(|_| {
        CommerceError::Core(CoreError::Internal(
            "cannot fingerprint variant axes plan".into(),
        ))
    })?;
    let state_token = hex::encode(Sha256::digest(bytes));
    let mut conflicts = Conflicts::default();
    let mut identities = BTreeMap::<String, Vec<Uuid>>::new();
    let mut variants = Vec::with_capacity(snapshot.variants.len());
    let mut rows = Vec::with_capacity(snapshot.variants.len());
    let configuration_changed = configuration_changed(snapshot, input)?;
    let mut has_changes = configuration_changed;
    if input.axes.is_empty() && snapshot.variants.len() > 1 {
        conflicts.push(
            VariantAxesConflictKind::MultipleDefaultVariants,
            &snapshot
                .variants
                .iter()
                .map(|variant| variant.id)
                .collect::<Vec<_>>(),
            None,
            None,
        );
    }
    for variant in &snapshot.variants {
        let existing = source.get(&variant.id).cloned().unwrap_or_default();
        let current_values = canonical_values(
            &existing
                .iter()
                .filter(|value| current_axes.contains(&value.attribute_id))
                .cloned()
                .collect::<Vec<_>>(),
        );
        let proposed_values = canonical_values(&match explicit.get(&variant.id) {
            Some(row) => row.axis_values.clone(),
            None => existing
                .iter()
                .filter(|value| target_axes.contains_key(&value.attribute_id))
                .cloned()
                .collect(),
        });
        let mut seen = HashSet::new();
        let mut valid = true;
        for value in &proposed_values {
            let kind = if !seen.insert(value.attribute_id) {
                Some(VariantAxesConflictKind::DuplicateAssignment)
            } else {
                match target_axes.get(&value.attribute_id) {
                    None => Some(VariantAxesConflictKind::UnexpectedAssignment),
                    Some(axis) if !axis.allowed_option_ids.contains(&value.option_id) => {
                        Some(VariantAxesConflictKind::ValueNotAllowed)
                    }
                    Some(_) => None,
                }
            };
            if let Some(kind) = kind {
                valid = false;
                conflicts.push(
                    kind,
                    &[variant.id],
                    Some(value.attribute_id),
                    Some(value.option_id),
                );
            }
        }
        for attribute_id in target_axes.keys() {
            if !seen.contains(attribute_id) {
                valid = false;
                conflicts.push(
                    VariantAxesConflictKind::MissingAssignment,
                    &[variant.id],
                    Some(*attribute_id),
                    None,
                );
            }
        }
        let identity = variant_combination_identity(&proposed_values);
        if valid
            && !target_axes.is_empty()
            && let Some(identity) = &identity
        {
            identities
                .entry(identity.clone())
                .or_default()
                .push(variant.id);
        }
        let mut existing_by_attribute = BTreeMap::<Uuid, Vec<&VariantAxisValueInput>>::new();
        for value in &existing {
            existing_by_attribute
                .entry(value.attribute_id)
                .or_default()
                .push(value);
        }
        let values_to_write = proposed_values
            .iter()
            .filter(|value| {
                existing_by_attribute
                    .get(&value.attribute_id)
                    .is_none_or(|existing| existing.len() != 1 || existing[0] != *value)
            })
            .cloned()
            .collect::<Vec<_>>();
        let identity_changed = variant.combination_identity != identity;
        let changes =
            current_values != proposed_values || identity_changed || !values_to_write.is_empty();
        has_changes |= changes;
        rows.push(VariantAxesPreviewVariant {
            variant_id: variant.id,
            sku: variant.sku.clone(),
            current_values,
            proposed_values,
            changes,
        });
        variants.push(VariantPlan {
            variant_id: variant.id,
            identity,
            identity_changed,
            values_to_write,
        });
    }
    for duplicate in identities.values().filter(|ids| ids.len() > 1) {
        conflicts.push(
            VariantAxesConflictKind::DuplicateCombination,
            duplicate,
            None,
            None,
        );
    }
    Ok(AxisPlan {
        configuration_changed,
        preview: VariantAxesPreviewResponse {
            product_id: snapshot.product.id,
            product_revision: snapshot.product.revision,
            state_token,
            variant_count: snapshot.variants.len() as i32,
            can_apply: conflicts.entries.is_empty() && !conflicts.has_more,
            has_changes,
            conflicts: conflicts.entries,
            has_more_conflicts: conflicts.has_more,
            variants: rows,
        },
        variants,
    })
}

impl CatalogService {
    /// Consistent owner read with the same lock order/planner as the command. No receipt, outbox,
    /// index or catalog data is persisted; rollback releases the short-lived snapshot locks.
    pub async fn preview_variant_axes(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        input: SetVariantAxesInput,
    ) -> CommerceResult<VariantAxesPreviewResponse> {
        validate_input(&input)?;
        let txn = self.db.begin().await?;
        lock_schema_for_product_write(&txn, tenant_id).await?;
        let product = find_product_for_update_in_tx(&txn, tenant_id, product_id).await?;
        validate_variant_axis_configuration_in(
            &txn,
            tenant_id,
            product.primary_category_id,
            &input.axes,
        )
        .await?;
        let snapshot = load_snapshot(&txn, tenant_id, product, &input).await?;
        let result = plan(&snapshot, &input)?.preview;
        txn.rollback().await?;
        Ok(result)
    }

    /// Existing durable command/result shape, with explicit preview predecessor and optional
    /// full target tuples. The receipt wrapper replays completed historical requests first.
    pub async fn set_variant_axes(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        product_id: Uuid,
        input: SetVariantAxesInput,
    ) -> CommerceResult<Vec<VariantAxisConfigResponse>> {
        validate_input(&input)?;
        let expected = input
            .expected_state
            .as_deref()
            .ok_or_else(|| refusal(PREVIEW_REQUIRED))?;
        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
        lock_schema_for_product_write(&txn, tenant_id).await?;
        let product = find_product_for_update_in_tx(&txn, tenant_id, product_id).await?;
        validate_variant_axis_configuration_in(
            &txn,
            tenant_id,
            product.primary_category_id,
            &input.axes,
        )
        .await?;
        let snapshot = load_snapshot(&txn, tenant_id, product, &input).await?;
        let plan = plan(&snapshot, &input)?;
        if expected != plan.preview.state_token {
            return Err(refusal(STALE_PREVIEW));
        }
        if !plan.preview.can_apply {
            return Err(refusal(INVALID_PLAN));
        }
        if plan.preview.has_changes {
            // Exhaustion is checked before any catalog mutation.
            let revision = next_revision(snapshot.product.revision)?;
            if plan.configuration_changed {
                persist_configuration(&txn, tenant_id, product_id, &snapshot, &input).await?;
            }
            // The unique combination index is immediate. Stage changing identities before any
            // tuple update, including swaps. Unchanged identities cannot collide with the final
            // unique plan. Avoid configuration writes/deferred triggers for tuple-only edits.
            if plan.variants.len() > 1
                && plan.variants.iter().any(|variant| variant.identity_changed)
            {
                let invocation = generate_id();
                for variant in plan
                    .variants
                    .iter()
                    .filter(|variant| variant.identity_changed)
                {
                    set_identity(
                        &txn,
                        tenant_id,
                        product_id,
                        variant.variant_id,
                        Some(format!(
                            "__axis-reconfiguration:{invocation}:{}",
                            variant.variant_id
                        )),
                    )
                    .await?;
                }
            }
            for variant in &plan.variants {
                if !variant.values_to_write.is_empty() {
                    assign_variant_axis_value_rows_in_tx(
                        &txn,
                        tenant_id,
                        variant.variant_id,
                        &variant.values_to_write,
                    )
                    .await?;
                }
            }
            for variant in plan.variants.iter().filter(|variant| {
                plan.configuration_changed
                    || variant.identity_changed
                    || !variant.values_to_write.is_empty()
            }) {
                // Also touch unchanged tuples on configuration/order changes so the canonical
                // PostgreSQL index-revision triggers advance all required Variant sources.
                set_identity(
                    &txn,
                    tenant_id,
                    product_id,
                    variant.variant_id,
                    variant.identity.clone(),
                )
                .await?;
            }
            let mut product: entities::product::ActiveModel = snapshot.product.into();
            product.revision = Set(revision);
            product.updated_at = Set(Utc::now().into());
            product.update(&txn).await?;
            txn.publish(
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductUpdated { product_id },
            )
            .await?;
        }
        let result = super::super::projection::load_product_variant_axes(
            &txn,
            tenant_id,
            product_id,
            PLATFORM_FALLBACK_LOCALE,
            None,
            false,
        )
        .await?;
        record_product_operation_result(&result);
        txn.commit().await?;
        Ok(result)
    }
}

async fn set_identity(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
    variant_id: Uuid,
    identity: Option<String>,
) -> CommerceResult<()> {
    let result = txn.execute_raw(Statement::from_sql_and_values(
        txn.get_database_backend(),
        "UPDATE product_variants SET combination_identity = $1 WHERE tenant_id = $2 AND product_id = $3 AND id = $4",
        vec![identity.into(), tenant_id.into(), product_id.into(), variant_id.into()],
    )).await?;
    if result.rows_affected() != 1 {
        return Err(CommerceError::VariantNotFound(variant_id));
    }
    Ok(())
}

/// Only configuration membership is removed. Existing attribute values, stock, price, channels,
/// media references and retained axis/allowed-value row IDs are untouched.
async fn persist_configuration(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
    snapshot: &AxisSnapshot,
    input: &SetVariantAxesInput,
) -> CommerceResult<()> {
    let target = input
        .axes
        .iter()
        .map(|axis| axis.attribute_id)
        .collect::<HashSet<_>>();
    for axis in &snapshot.axes {
        if !target.contains(&axis.attribute_id) {
            entities::product_variant_axis_value::Entity::delete_many()
                .filter(entities::product_variant_axis_value::Column::TenantId.eq(tenant_id))
                .filter(entities::product_variant_axis_value::Column::AxisId.eq(axis.id))
                .exec(txn)
                .await?;
            entities::product_variant_axis::Entity::delete_many()
                .filter(entities::product_variant_axis::Column::TenantId.eq(tenant_id))
                .filter(entities::product_variant_axis::Column::ProductId.eq(product_id))
                .filter(entities::product_variant_axis::Column::Id.eq(axis.id))
                .exec(txn)
                .await?;
        }
    }
    // Configuration position indexes are immediate UNIQUE indexes as well. Stage retained
    // rows in bounded, non-negative holes disjoint from both predecessor and target positions;
    // never use negative positions or `max + gap` (CHECK constraints / i32 exhaustion).
    let positions = input
        .axes
        .iter()
        .enumerate()
        .map(|(index, axis)| resolved_axis_position(axis.position, index))
        .collect::<CommerceResult<Vec<_>>>()?;
    let target_positions = input
        .axes
        .iter()
        .zip(&positions)
        .map(|(axis, position)| (axis.attribute_id, *position))
        .collect::<HashMap<_, _>>();
    let retained = snapshot
        .axes
        .iter()
        .filter(|axis| {
            target_positions
                .get(&axis.attribute_id)
                .is_some_and(|position| *position != axis.position)
        })
        .collect::<Vec<_>>();
    let staged = free_configuration_positions(
        snapshot.axes.iter().map(|axis| axis.position),
        positions.iter().copied(),
        retained.len(),
    )?;
    for (axis, position) in retained.into_iter().zip(staged) {
        let mut active: entities::product_variant_axis::ActiveModel = axis.clone().into();
        active.position = Set(position);
        active.update(txn).await?;
    }
    let now = Utc::now();
    for (index, input_axis) in input.axes.iter().enumerate() {
        let position = resolved_axis_position(input_axis.position, index)?;
        let existing = snapshot
            .axes
            .iter()
            .find(|axis| axis.attribute_id == input_axis.attribute_id);
        let axis_id = match existing {
            Some(axis) => {
                if axis.position != position {
                    let mut active: entities::product_variant_axis::ActiveModel =
                        axis.clone().into();
                    active.position = Set(position);
                    active.update(txn).await?;
                }
                axis.id
            }
            None => {
                let axis_id = generate_id();
                entities::product_variant_axis::ActiveModel {
                    id: Set(axis_id),
                    tenant_id: Set(tenant_id),
                    product_id: Set(product_id),
                    attribute_id: Set(input_axis.attribute_id),
                    position: Set(position),
                    created_at: Set(now.into()),
                }
                .insert(txn)
                .await?;
                axis_id
            }
        };
        for value in snapshot
            .allowed
            .iter()
            .filter(|value| value.axis_id == axis_id)
        {
            if !input_axis.allowed_option_ids.contains(&value.option_id) {
                entities::product_variant_axis_value::Entity::delete_many()
                    .filter(entities::product_variant_axis_value::Column::TenantId.eq(tenant_id))
                    .filter(entities::product_variant_axis_value::Column::AxisId.eq(axis_id))
                    .filter(entities::product_variant_axis_value::Column::Id.eq(value.id))
                    .exec(txn)
                    .await?;
            }
        }
        let proposed_positions = (0..input_axis.allowed_option_ids.len())
            .map(|index| resolved_axis_position(0, index))
            .collect::<CommerceResult<Vec<_>>>()?;
        let target_positions = input_axis
            .allowed_option_ids
            .iter()
            .zip(&proposed_positions)
            .map(|(option_id, position)| (*option_id, *position))
            .collect::<HashMap<_, _>>();
        let retained_values = snapshot
            .allowed
            .iter()
            .filter(|value| {
                value.axis_id == axis_id
                    && target_positions
                        .get(&value.option_id)
                        .is_some_and(|position| *position != value.position)
            })
            .collect::<Vec<_>>();
        let staged = free_configuration_positions(
            snapshot
                .allowed
                .iter()
                .filter(|value| value.axis_id == axis_id)
                .map(|value| value.position),
            proposed_positions,
            retained_values.len(),
        )?;
        for (value, position) in retained_values.into_iter().zip(staged) {
            let mut active: entities::product_variant_axis_value::ActiveModel =
                value.clone().into();
            active.position = Set(position);
            active.update(txn).await?;
        }
        for (index, option_id) in input_axis.allowed_option_ids.iter().enumerate() {
            let position = resolved_axis_position(0, index)?;
            match snapshot
                .allowed
                .iter()
                .find(|value| value.axis_id == axis_id && value.option_id == *option_id)
            {
                Some(value) => {
                    if value.position != position {
                        let mut active: entities::product_variant_axis_value::ActiveModel =
                            value.clone().into();
                        active.position = Set(position);
                        active.update(txn).await?;
                    }
                }
                None => {
                    entities::product_variant_axis_value::ActiveModel {
                        id: Set(generate_id()),
                        tenant_id: Set(tenant_id),
                        axis_id: Set(axis_id),
                        option_id: Set(*option_id),
                        position: Set(position),
                        created_at: Set(now.into()),
                    }
                    .insert(txn)
                    .await?;
                }
            }
        }
    }
    Ok(())
}

/// Pigeonhole bound: at most N occupied positions plus K staging rows needs only N+K probes,
/// independent of authored positions (including i32::MAX). Caller snapshots are already bounded.
fn free_configuration_positions(
    current: impl IntoIterator<Item = i32>,
    proposed: impl IntoIterator<Item = i32>,
    count: usize,
) -> CommerceResult<Vec<i32>> {
    if count > MAX_VARIANT_AXIS_VALUES {
        return Err(refusal(LIMIT_EXCEEDED));
    }
    let occupied = current.into_iter().chain(proposed).collect::<BTreeSet<_>>();
    let upper = occupied
        .len()
        .checked_add(count)
        .and_then(|upper| i32::try_from(upper).ok())
        .ok_or_else(|| refusal(LIMIT_EXCEEDED))?;
    let result = (0..upper)
        .filter(|position| !occupied.contains(position))
        .take(count)
        .collect::<Vec<_>>();
    if result.len() != count {
        return Err(refusal(LIMIT_EXCEEDED));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u128) -> Uuid {
        Uuid::from_u128(value)
    }
    fn values(color: u128, size: u128) -> Vec<VariantAxisValueInput> {
        vec![
            VariantAxisValueInput {
                attribute_id: id(3),
                option_id: id(color),
            },
            VariantAxisValueInput {
                attribute_id: id(4),
                option_id: id(size),
            },
        ]
    }
    fn input() -> SetVariantAxesInput {
        SetVariantAxesInput {
            axes: vec![
                VariantAxisInput {
                    attribute_id: id(3),
                    position: 0,
                    allowed_option_ids: vec![id(5), id(6)],
                },
                VariantAxisInput {
                    attribute_id: id(4),
                    position: 100,
                    allowed_option_ids: vec![id(7), id(8)],
                },
            ],
            ..Default::default()
        }
    }
    fn snapshot() -> AxisSnapshot {
        let now = Utc::now().fixed_offset();
        let variants = [(10, 7), (11, 8)]
            .into_iter()
            .map(|(variant, size)| entities::product_variant::Model {
                id: id(variant),
                tenant_id: id(1),
                product_id: id(2),
                sku: Some(format!("sku-{variant}")),
                barcode: None,
                shipping_profile_slug: None,
                ean: None,
                upc: None,
                inventory_policy: "deny".into(),
                inventory_management: "shopify".into(),
                inventory_quantity: variant as i32,
                weight: None,
                weight_unit: None,
                combination_identity: variant_combination_identity(&values(5, size)),
                position: variant as i32,
                created_at: now,
                updated_at: now,
            })
            .collect::<Vec<_>>();
        let assignments = variants
            .iter()
            .enumerate()
            .flat_map(|(index, variant)| {
                values(5, if index == 0 { 7 } else { 8 })
                    .into_iter()
                    .enumerate()
                    .map(move |(axis, value)| AssignmentRow {
                        variant_id: variant.id,
                        attribute_id: value.attribute_id,
                        value_id: id(100 + (index * 2 + axis) as u128),
                        option_id: Some(value.option_id),
                        detached: false,
                    })
            })
            .collect();
        AxisSnapshot {
            product: entities::product::Model {
                id: id(2),
                tenant_id: id(1),
                status: entities::product::ProductStatus::Draft,
                seller_id: None,
                vendor: None,
                product_type: None,
                shipping_profile_slug: None,
                primary_category_id: Some(id(9)),
                metadata: serde_json::json!({}),
                created_at: now,
                updated_at: now,
                published_at: None,
                revision: 7,
            },
            axes: input()
                .axes
                .iter()
                .enumerate()
                .map(|(index, axis)| entities::product_variant_axis::Model {
                    id: id(20 + index as u128),
                    tenant_id: id(1),
                    product_id: id(2),
                    attribute_id: axis.attribute_id,
                    position: axis.position,
                    created_at: now,
                })
                .collect(),
            allowed: [(20, 5, 0), (20, 6, 100), (21, 7, 0), (21, 8, 100)]
                .into_iter()
                .enumerate()
                .map(|(index, (axis, option, position))| {
                    entities::product_variant_axis_value::Model {
                        id: id(30 + index as u128),
                        tenant_id: id(1),
                        axis_id: id(axis),
                        option_id: id(option),
                        position,
                        created_at: now,
                    }
                })
                .collect(),
            variants,
            assignments,
        }
    }

    #[test]
    fn unchanged_sparse_configuration_is_a_noop_and_keeps_variant_ids() {
        let snapshot = snapshot();
        let planned = plan(&snapshot, &input()).unwrap();
        assert!(planned.preview.can_apply);
        assert!(!planned.preview.has_changes);
        assert!(!planned.configuration_changed);
        assert!(
            planned
                .variants
                .iter()
                .all(|variant| !variant.identity_changed)
        );
        assert_eq!(
            planned
                .variants
                .iter()
                .map(|variant| variant.variant_id)
                .collect::<Vec<_>>(),
            vec![id(10), id(11)]
        );
        assert!(
            planned
                .variants
                .iter()
                .all(|variant| variant.values_to_write.is_empty())
        );
    }

    #[test]
    fn removing_axis_reports_collision_until_operator_reassigns_a_full_target_tuple() {
        let snapshot = snapshot();
        let mut proposal = input();
        proposal.axes.pop();
        let conflict = plan(&snapshot, &proposal).unwrap();
        assert!(!conflict.preview.can_apply);
        assert!(
            conflict
                .preview
                .conflicts
                .iter()
                .any(
                    |conflict| conflict.kind == VariantAxesConflictKind::DuplicateCombination
                        && conflict.variant_ids == vec![id(10), id(11)]
                )
        );
        proposal.reassignments.push(VariantAxisReassignmentInput {
            variant_id: id(11),
            axis_values: vec![VariantAxisValueInput {
                attribute_id: id(3),
                option_id: id(6),
            }],
        });
        let planned = plan(&snapshot, &proposal).unwrap();
        assert!(planned.preview.can_apply);
        assert!(
            planned
                .variants
                .iter()
                .flat_map(|row| &row.values_to_write)
                .all(|value| value.attribute_id == id(3)),
            "a removed axis's EAV is retained, never scheduled for deletion/overwrite"
        );
        assert_eq!(snapshot.assignments.len(), 4);
        assert_eq!(snapshot.variants[1].inventory_quantity, 11);
    }

    #[test]
    fn explicit_tuples_are_not_partial_patches_or_silently_pruned() {
        let snapshot = snapshot();
        let mut proposal = input();
        proposal.reassignments.push(VariantAxisReassignmentInput {
            variant_id: id(10),
            axis_values: vec![VariantAxisValueInput {
                attribute_id: id(3),
                option_id: id(5),
            }],
        });
        let planned = plan(&snapshot, &proposal).unwrap();
        assert!(
            planned
                .preview
                .conflicts
                .iter()
                .any(|conflict| conflict.kind == VariantAxesConflictKind::MissingAssignment)
        );
        proposal.axes.pop();
        proposal.reassignments[0].axis_values = values(6, 8);
        assert!(
            plan(&snapshot, &proposal)
                .unwrap()
                .preview
                .conflicts
                .iter()
                .any(|conflict| conflict.kind == VariantAxesConflictKind::UnexpectedAssignment)
        );
    }

    #[test]
    fn complete_tuple_swaps_are_valid_and_still_target_the_original_skus() {
        let snapshot = snapshot();
        let mut proposal = input();
        proposal.reassignments = vec![
            VariantAxisReassignmentInput {
                variant_id: id(10),
                axis_values: values(5, 8),
            },
            VariantAxisReassignmentInput {
                variant_id: id(11),
                axis_values: values(5, 7),
            },
        ];
        let planned = plan(&snapshot, &proposal).unwrap();
        assert!(planned.preview.can_apply);
        assert!(
            !planned.configuration_changed,
            "tuple swaps must not rewrite configuration rows or enqueue their deferred triggers"
        );
        assert!(
            planned
                .variants
                .iter()
                .all(|variant| variant.identity_changed)
        );
        assert_eq!(
            planned.variants[0].identity,
            snapshot.variants[1].combination_identity
        );
        assert_eq!(
            planned.variants[1].identity,
            snapshot.variants[0].combination_identity
        );
        assert_eq!(planned.variants[0].variant_id, id(10));
        assert!(
            planned
                .variants
                .iter()
                .all(|row| row.values_to_write.len() == 1)
        );
    }

    #[test]
    fn token_binds_exact_proposal_membership_and_relevant_eav_not_stock() {
        let mut snapshot = snapshot();
        let mut proposal = input();
        let token = plan(&snapshot, &proposal).unwrap().preview.state_token;
        proposal.expected_state = Some(token.clone());
        assert_eq!(
            plan(&snapshot, &proposal).unwrap().preview.state_token,
            token
        );
        snapshot.variants[0].inventory_quantity += 1;
        assert_eq!(
            plan(&snapshot, &proposal).unwrap().preview.state_token,
            token
        );
        let mut changed = snapshot.clone();
        changed.variants[0].sku = Some("renamed".into());
        assert_ne!(
            plan(&changed, &proposal).unwrap().preview.state_token,
            token
        );
        changed = snapshot.clone();
        changed.assignments[0].value_id = id(999);
        assert_ne!(
            plan(&changed, &proposal).unwrap().preview.state_token,
            token
        );
        changed = snapshot.clone();
        changed.assignments[0].detached = true;
        assert_ne!(
            plan(&changed, &proposal).unwrap().preview.state_token,
            token
        );
        changed = snapshot.clone();
        changed.product.revision += 1;
        assert_ne!(
            plan(&changed, &proposal).unwrap().preview.state_token,
            token
        );
        changed = snapshot.clone();
        changed.variants.pop();
        assert_ne!(
            plan(&changed, &proposal).unwrap().preview.state_token,
            token
        );
        proposal.reassignments = vec![
            VariantAxisReassignmentInput {
                variant_id: id(10),
                axis_values: values(5, 7),
            },
            VariantAxisReassignmentInput {
                variant_id: id(11),
                axis_values: values(5, 8),
            },
        ];
        let ordered_token = plan(&snapshot, &proposal).unwrap().preview.state_token;
        proposal.reassignments.reverse();
        assert_ne!(
            plan(&snapshot, &proposal).unwrap().preview.state_token,
            ordered_token
        );
    }

    #[test]
    fn no_axes_cannot_coalesce_two_skus_into_one_default() {
        let planned = plan(&snapshot(), &SetVariantAxesInput::default()).unwrap();
        assert!(!planned.preview.can_apply);
        assert!(
            planned
                .preview
                .conflicts
                .iter()
                .any(|conflict| conflict.kind == VariantAxesConflictKind::MultipleDefaultVariants)
        );
    }

    #[test]
    fn diagnostics_and_sync_requests_are_bounded_and_historical_input_shape_survives() {
        let mut conflicts = Conflicts::default();
        let examples = (1..=10).map(id).collect::<Vec<_>>();
        for _ in 0..20 {
            conflicts.push(
                VariantAxesConflictKind::DuplicateCombination,
                &examples,
                None,
                None,
            );
        }
        assert_eq!(conflicts.entries.len(), 16);
        assert!(conflicts.has_more);
        assert!(
            conflicts
                .entries
                .iter()
                .all(|conflict| conflict.variant_ids.len() == 8 && conflict.has_more_variants)
        );
        let mut too_many = input();
        too_many.axes = vec![too_many.axes[0].clone(); MAX_VARIANT_AXES + 1];
        assert!(
            matches!(validate_input(&too_many), Err(CommerceError::Validation(message)) if message == LIMIT_EXCEEDED)
        );
        let old: SetVariantAxesInput =
            serde_json::from_value(serde_json::json!({ "axes": [] })).unwrap();
        assert_eq!(
            serde_json::to_value(old).unwrap(),
            serde_json::json!({ "axes": [] })
        );
    }

    #[test]
    fn staging_positions_handle_swaps_and_i32_max_without_negative_values_or_overflow() {
        assert_eq!(
            free_configuration_positions([0, 100], [100, 0], 2).unwrap(),
            vec![1, 2]
        );
        assert_eq!(
            free_configuration_positions([i32::MAX, 0], [i32::MAX - 1, 1], 2).unwrap(),
            vec![2, 3]
        );
        assert!(free_configuration_positions([], [], 257).is_err());
    }
}
