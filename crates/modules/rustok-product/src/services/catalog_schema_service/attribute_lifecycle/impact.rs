/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

//! Bounded evidence, never a destructive cleanup plan. Detached/historical rows also retain IDs.

use super::*;
use crate::services::catalog_schema::VirtualCategoryAttributeCondition;

#[derive(FromQueryResult)]
struct ReferenceRow {
    resource_id: Uuid,
}
#[derive(FromQueryResult)]
struct JsonReferenceRow {
    resource_id: Uuid,
    payload: Value,
}
#[derive(FromQueryResult)]
struct BoundedJsonReferenceRow {
    resource_id: Uuid,
    payload: Option<String>,
    payload_bytes: i64,
    within_limit: bool,
}

const MAX_REFERENCE_JSON_ROW_BYTES: usize = 1024 * 1024;
const MAX_REFERENCE_JSON_TOTAL_BYTES: usize = 4 * 1024 * 1024;
struct InspectionBudget {
    rows: usize,
    bytes: usize,
    bindings: usize,
}
impl Default for InspectionBudget {
    fn default() -> Self {
        Self {
            rows: MAX_ATTRIBUTE_LIFECYCLE_ROWS,
            bytes: MAX_REFERENCE_JSON_TOTAL_BYTES,
            bindings: MAX_ATTRIBUTE_LIFECYCLE_ROWS,
        }
    }
}

impl AttributeReferenceImpact {
    fn observe(&mut self, kind: AttributeReferenceKind, resource_id: Uuid) {
        if resource_id.is_nil() {
            self.inspection_complete = false;
            return;
        }
        let reference = AttributeReference { kind, resource_id };
        if self.references.contains(&reference) {
            return;
        }
        if self.references.len() < MAX_ATTRIBUTE_REFERENCE_EXAMPLES {
            self.references.push(reference);
        } else {
            self.has_more = true;
        }
    }
}

async fn references(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    target: Uuid,
    sql: &str,
    kind: AttributeReferenceKind,
    impact: &mut AttributeReferenceImpact,
) -> CommerceResult<()> {
    let rows = ReferenceRow::find_by_statement(Statement::from_sql_and_values(
        txn.get_database_backend(),
        format!(
            "{sql} ORDER BY resource_id LIMIT {}",
            MAX_ATTRIBUTE_REFERENCE_EXAMPLES + 1
        ),
        vec![tenant_id.into(), target.into()],
    ))
    .all(txn)
    .await?;
    for row in rows {
        impact.observe(kind, row.resource_id);
    }
    Ok(())
}

/// JSON is compared structurally (typed UUID strings), never with substring matching. These are
/// owner-defined defaults/rules/snapshots, not arbitrary metadata. Unknown stored payload shapes
/// cannot be used to prove absence of a reference.
fn contains_uuid(value: &Value, target: Uuid) -> bool {
    contains_any_uuid(value, &std::collections::HashSet::from([target]))
}
fn contains_any_uuid(value: &Value, targets: &std::collections::HashSet<Uuid>) -> bool {
    if targets.is_empty() {
        return false;
    }
    match value {
        Value::String(value) => Uuid::parse_str(value.trim()).is_ok_and(|id| targets.contains(&id)),
        Value::Array(values) => values.iter().any(|value| contains_any_uuid(value, targets)),
        Value::Object(values) => values.iter().any(|(key, value)| {
            Uuid::parse_str(key).is_ok_and(|id| targets.contains(&id))
                || contains_any_uuid(value, targets)
        }),
        _ => false,
    }
}

/// Bound both decoding and the whole inspection, not just the number of database rows.
/// Oversized JSON stays SQL NULL on the wire with an explicit failed budget flag. Window
/// accounting prevents 1000 individually-valid rows from allocating a gigabyte in Rust.
async fn json_rows(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    attribute_id: Option<Uuid>,
    sql: &str,
    impact: &mut AttributeReferenceImpact,
    budget: &mut InspectionBudget,
) -> CommerceResult<Vec<JsonReferenceRow>> {
    let bytes = match txn.get_database_backend() {
        DatabaseBackend::Postgres => "COALESCE(octet_length(CAST(payload AS TEXT)), 4)",
        DatabaseBackend::Sqlite => "length(CAST(COALESCE(payload, 'null') AS BLOB))",
        _ => {
            return Err(CommerceError::Database(sea_orm::DbErr::Custom(
                "Attribute reference inspection capability is unavailable".into(),
            )));
        }
    };
    let mut values = vec![tenant_id.into()];
    if let Some(id) = attribute_id {
        values.push(id.into());
    }
    let statement = format!(
        "WITH page AS (SELECT resource_id, payload, CAST({bytes} AS BIGINT) AS payload_bytes FROM ({sql}) AS source ORDER BY resource_id LIMIT {}), measured AS (SELECT *, SUM(payload_bytes) OVER (ORDER BY resource_id ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS running_bytes, ROW_NUMBER() OVER (ORDER BY resource_id) AS row_number FROM page) SELECT resource_id, CASE WHEN row_number <= {} AND payload_bytes <= {MAX_REFERENCE_JSON_ROW_BYTES} AND running_bytes <= {} THEN CAST(payload AS TEXT) ELSE NULL END AS payload, payload_bytes, (row_number <= {} AND payload_bytes <= {MAX_REFERENCE_JSON_ROW_BYTES} AND running_bytes <= {}) AS within_limit FROM measured ORDER BY resource_id",
        budget.rows + 1,
        budget.rows,
        budget.bytes,
        budget.rows,
        budget.bytes,
    );
    let rows = BoundedJsonReferenceRow::find_by_statement(Statement::from_sql_and_values(
        txn.get_database_backend(),
        statement,
        values,
    ))
    .all(txn)
    .await?;
    let mut decoded = Vec::new();
    for row in rows {
        if budget.rows == 0 {
            impact.inspection_complete = false;
            impact.has_more = true;
            break;
        }
        budget.rows -= 1;
        let within_limit = row.within_limit
            && row.payload_bytes >= 0
            && row.payload_bytes as u64 <= budget.bytes as u64;
        if !within_limit {
            impact.inspection_complete = false;
            impact.has_more = true;
            continue;
        }
        budget.bytes -= row.payload_bytes as usize;
        let payload = match row.payload {
            None => Value::Null,
            Some(raw) => match serde_json::from_str(&raw) {
                Ok(payload) => payload,
                Err(_) => {
                    impact.inspection_complete = false;
                    continue;
                }
            },
        };
        if json_depth(&payload) > MAX_PRODUCT_JSON_DEPTH + 2 || row.resource_id.is_nil() {
            impact.inspection_complete = false;
            continue;
        }
        decoded.push(JsonReferenceRow {
            resource_id: row.resource_id,
            payload,
        });
    }
    Ok(decoded)
}

#[cfg(test)]
fn inspect_rules(
    payload: &Value,
    target: Uuid,
    resource: Uuid,
    kind: AttributeReferenceKind,
    impact: &mut AttributeReferenceImpact,
) {
    inspect_rule_targets(
        payload,
        &std::collections::HashSet::from([target]),
        resource,
        kind,
        impact,
    );
}
fn inspect_rule_targets(
    payload: &Value,
    targets: &std::collections::HashSet<Uuid>,
    resource: Uuid,
    kind: AttributeReferenceKind,
    impact: &mut AttributeReferenceImpact,
) {
    let rules = validate_bounded_json_object("reference rules", payload)
        .and_then(|_| attribute_validation::validate_declared_rule_keys(payload))
        .and_then(|_| attribute_validation::parse_product_attribute_validation(payload));
    match rules {
        Ok(rules) => {
            if rules
                .allowed_option_ids
                .as_ref()
                .is_some_and(|ids| ids.iter().any(|id| targets.contains(id)))
            {
                impact.observe(kind, resource);
            }
        }
        Err(_) => {
            impact.inspection_complete = false;
            if contains_any_uuid(payload, targets) {
                impact.observe(kind, resource);
            }
        }
    }
}

/// There is no owner-defined object-shaped select default. Preserve unknown stored payloads
/// by refusing the absence proof. UUIDs and stable option codes are inspected structurally.
fn inspect_default(
    payload: &Value,
    option: &AttributeOptionDefinition,
    resource: Uuid,
    impact: &mut AttributeReferenceImpact,
) {
    let refers = |value: &str| {
        value.trim() == option.code || Uuid::parse_str(value.trim()).is_ok_and(|id| id == option.id)
    };
    let known = match payload {
        Value::Null => true,
        Value::String(value) => {
            if refers(value) {
                impact.observe(AttributeReferenceKind::OptionRule, resource);
            }
            true
        }
        Value::Array(values) if values.iter().all(Value::is_string) => {
            if values.iter().filter_map(Value::as_str).any(refers) {
                impact.observe(AttributeReferenceKind::OptionRule, resource);
            }
            true
        }
        _ => false,
    };
    if !known {
        impact.inspection_complete = false;
    }
    if contains_uuid(payload, option.id) {
        impact.observe(AttributeReferenceKind::OptionRule, resource);
    }
}

fn known_clone_shape(payload: &Value) -> bool {
    let Some(bindings) = payload.as_array() else {
        return false;
    };
    bindings.len() <= MAX_ATTRIBUTE_LIFECYCLE_ROWS
        && bindings.iter().all(|binding| {
            binding.as_object().is_some_and(|binding| {
                binding.keys().all(|key| {
                    [
                        "attribute_id",
                        "group_code",
                        "is_required",
                        "is_disabled",
                        "position",
                        "visibility_overrides",
                        "validation_overrides",
                        "validation",
                        "source",
                        "variant_axis_policy",
                        "default_variant_axis",
                    ]
                    .contains(&key.as_str())
                })
            })
        })
}

pub(super) async fn inspect(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    attribute: &AttributeDefinition,
    option: Option<&AttributeOptionDefinition>,
) -> CommerceResult<AttributeReferenceImpact> {
    let mut impact = AttributeReferenceImpact {
        inspection_complete: true,
        ..Default::default()
    };
    let mut budget = InspectionBudget::default();
    match option {
        None => {
            for (sql, kind) in [
                (
                    "SELECT DISTINCT product_id AS resource_id FROM product_attribute_values WHERE tenant_id = $1 AND attribute_id = $2",
                    AttributeReferenceKind::ProductValue,
                ),
                (
                    "SELECT DISTINCT variant_id AS resource_id FROM product_variant_attribute_values WHERE tenant_id = $1 AND attribute_id = $2",
                    AttributeReferenceKind::VariantValue,
                ),
                (
                    "SELECT DISTINCT schema_id AS resource_id FROM product_attribute_schema_attributes WHERE tenant_id = $1 AND attribute_id = $2",
                    AttributeReferenceKind::SchemaBinding,
                ),
                (
                    "SELECT DISTINCT category_id AS resource_id FROM category_attributes WHERE tenant_id = $1 AND attribute_id = $2",
                    AttributeReferenceKind::CategoryBinding,
                ),
                (
                    "SELECT DISTINCT product_id AS resource_id FROM product_variant_axes WHERE tenant_id = $1 AND attribute_id = $2",
                    AttributeReferenceKind::VariantAxis,
                ),
                (
                    "SELECT DISTINCT group_id AS resource_id FROM product_attribute_group_attributes WHERE tenant_id = $1 AND attribute_id = $2",
                    AttributeReferenceKind::AttributeGroup,
                ),
                (
                    "SELECT DISTINCT channel_id AS resource_id FROM product_attribute_channel_settings WHERE tenant_id = $1 AND attribute_id = $2",
                    AttributeReferenceKind::ChannelSetting,
                ),
                // Also retain historical foreign-parent EAV/allowed-option links to children.
                (
                    "SELECT DISTINCT v.product_id AS resource_id FROM product_attribute_value_options o INNER JOIN product_attribute_values v ON v.id = o.value_id AND v.tenant_id = o.tenant_id INNER JOIN product_attribute_options p ON p.id = o.option_id AND p.tenant_id = o.tenant_id WHERE p.tenant_id = $1 AND p.attribute_id = $2",
                    AttributeReferenceKind::ProductValue,
                ),
                (
                    "SELECT DISTINCT v.variant_id AS resource_id FROM product_variant_attribute_value_options o INNER JOIN product_variant_attribute_values v ON v.id = o.value_id AND v.tenant_id = o.tenant_id INNER JOIN product_attribute_options p ON p.id = o.option_id AND p.tenant_id = o.tenant_id WHERE p.tenant_id = $1 AND p.attribute_id = $2",
                    AttributeReferenceKind::VariantValue,
                ),
                (
                    "SELECT DISTINCT a.product_id AS resource_id FROM product_variant_axis_values v INNER JOIN product_variant_axes a ON a.id = v.axis_id AND a.tenant_id = v.tenant_id INNER JOIN product_attribute_options p ON p.id = v.option_id AND p.tenant_id = v.tenant_id WHERE p.tenant_id = $1 AND p.attribute_id = $2",
                    AttributeReferenceKind::AllowedOption,
                ),
            ] {
                references(txn, tenant_id, attribute.id, sql, kind, &mut impact).await?;
            }
        }
        Some(option) => {
            for (sql, kind) in [
                (
                    "SELECT DISTINCT v.product_id AS resource_id FROM product_attribute_value_options o INNER JOIN product_attribute_values v ON v.id = o.value_id AND v.tenant_id = o.tenant_id WHERE o.tenant_id = $1 AND o.option_id = $2",
                    AttributeReferenceKind::ProductValue,
                ),
                (
                    "SELECT DISTINCT v.variant_id AS resource_id FROM product_variant_attribute_value_options o INNER JOIN product_variant_attribute_values v ON v.id = o.value_id AND v.tenant_id = o.tenant_id WHERE o.tenant_id = $1 AND o.option_id = $2",
                    AttributeReferenceKind::VariantValue,
                ),
                (
                    "SELECT DISTINCT a.product_id AS resource_id FROM product_variant_axis_values v INNER JOIN product_variant_axes a ON a.id = v.axis_id AND a.tenant_id = v.tenant_id WHERE v.tenant_id = $1 AND v.option_id = $2",
                    AttributeReferenceKind::AllowedOption,
                ),
            ] {
                references(txn, tenant_id, option.id, sql, kind, &mut impact).await?;
            }
        }
    }
    // Parent archival also deactivates its retained children. Inspect their explicit UUIDs in
    // foreign/historical rules/defaults/clones, rather than assuming installed data is valid.
    let targets = match option {
        Some(option) => std::collections::HashSet::from([option.id]),
        None => {
            let rows = ReferenceRow::find_by_statement(Statement::from_sql_and_values(txn.get_database_backend(),
                format!("SELECT id AS resource_id FROM product_attribute_options WHERE tenant_id = $1 AND attribute_id = $2 ORDER BY id LIMIT {}", MAX_ATTRIBUTE_LIFECYCLE_ROWS + 1),
                vec![tenant_id.into(), attribute.id.into()],
            )).all(txn).await?;
            if rows.len() > MAX_ATTRIBUTE_LIFECYCLE_ROWS {
                impact.inspection_complete = false;
                impact.has_more = true;
            }
            let mut targets = std::collections::HashSet::new();
            for row in rows.into_iter().take(MAX_ATTRIBUTE_LIFECYCLE_ROWS) {
                if row.resource_id.is_nil() {
                    impact.inspection_complete = false;
                } else {
                    targets.insert(row.resource_id);
                }
            }
            targets
        }
    };
    let no_targets = std::collections::HashSet::new();
    for (root_rules, sql) in [
        (
            true,
            "SELECT id AS resource_id, validation AS payload FROM product_attributes WHERE tenant_id = $1",
        ),
        (
            false,
            "SELECT schema_id AS resource_id, validation_overrides AS payload FROM product_attribute_schema_attributes WHERE tenant_id = $1",
        ),
        (
            false,
            "SELECT category_id AS resource_id, validation_overrides AS payload FROM category_attributes WHERE tenant_id = $1",
        ),
    ] {
        for row in json_rows(txn, tenant_id, None, sql, &mut impact, &mut budget).await? {
            // Internal rules of the parent being archived are not an external consumer.
            let row_targets = if option.is_none() && root_rules && row.resource_id == attribute.id {
                &no_targets
            } else {
                &targets
            };
            inspect_rule_targets(
                &row.payload,
                row_targets,
                row.resource_id,
                AttributeReferenceKind::OptionRule,
                &mut impact,
            );
        }
    }
    if let Some(option) = option {
        for row in json_rows(txn, tenant_id, Some(attribute.id),
            "SELECT id AS resource_id, default_value AS payload FROM product_attributes WHERE tenant_id = $1 AND id = $2 AND default_value IS NOT NULL",
            &mut impact, &mut budget,
        ).await? { inspect_default(&row.payload, option, row.resource_id, &mut impact); }
    }
    for row in json_rows(txn, tenant_id, Some(attribute.id),
        "SELECT id AS resource_id, default_value AS payload FROM product_attributes WHERE tenant_id = $1 AND id <> $2 AND value_type IN ('select', 'multiselect') AND default_value IS NOT NULL",
        &mut impact, &mut budget,
    ).await? {
        if contains_any_uuid(&row.payload, &targets) { impact.observe(AttributeReferenceKind::OptionRule, row.resource_id); }
        if !matches!(&row.payload, Value::Null | Value::String(_)) && !row.payload.as_array().is_some_and(|values| values.iter().all(Value::is_string)) { impact.inspection_complete = false; }
    }
    // A clone has no FK back to its frozen bindings. Inspect every retained clone, including
    // a deleted category's assignment, rather than only the current live inheritance map.
    for row in json_rows(txn, tenant_id, None,
        "SELECT category_id AS resource_id, snapshot AS payload FROM category_attribute_schema_assignments WHERE tenant_id = $1 AND mode = 'clone_from_category'",
        &mut impact, &mut budget,
    ).await? {
        if !known_clone_shape(&row.payload) { impact.inspection_complete = false; continue; }
        let bindings: Vec<AttributeBinding> = match serde_json::from_value(row.payload) {
            Ok(bindings) => bindings,
            Err(_) => { impact.inspection_complete = false; continue; }
        };
        if bindings.len() > budget.bindings { impact.inspection_complete = false; impact.has_more = true; }
        let take = bindings.len().min(budget.bindings); budget.bindings -= take;
        for binding in bindings.into_iter().take(take) {
            if option.is_none() && binding.attribute_id == attribute.id {
                impact.observe(AttributeReferenceKind::CloneSnapshot, row.resource_id);
            }
            inspect_rule_targets(&binding.validation_overrides, &targets, row.resource_id, AttributeReferenceKind::CloneSnapshot, &mut impact);
            inspect_rule_targets(&binding.validation, &targets, row.resource_id, AttributeReferenceKind::CloneSnapshot, &mut impact);
        }
    }
    for row in json_rows(txn, tenant_id, None,
        "SELECT id AS resource_id, rule_config AS payload FROM catalog_categories WHERE tenant_id = $1 AND kind = 'virtual'",
        &mut impact, &mut budget,
    ).await? {
        let rule = match parse_virtual_category_rule_v1(&row.payload) {
            Ok(rule) => rule,
            Err(_) => { impact.inspection_complete = false; continue; }
        };
        for predicate in rule.attributes.iter().filter(|predicate| predicate.code.trim() == attribute.code) {
            let referenced = match (option, &predicate.condition) {
                (None, _) => true,
                (Some(option), VirtualCategoryAttributeCondition::Eq { value }) => {
                    value.trim() == option.code
                }
                _ => false,
            };
            if referenced { impact.observe(AttributeReferenceKind::VirtualCategory, row.resource_id); }
        }
    }
    Ok(impact)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uuid_references_are_structural_case_independent_and_never_substrings() {
        let id = Uuid::new_v4();
        assert!(contains_uuid(
            &serde_json::json!({"options": [id.to_string().to_uppercase()]}),
            id
        ));
        assert!(!contains_uuid(
            &serde_json::json!({"label": format!("prefix{id}suffix")}),
            id
        ));
        assert!(contains_uuid(&serde_json::json!(format!("  {id}  ")), id));
        let children = std::collections::HashSet::from([id, Uuid::new_v4()]);
        assert!(contains_any_uuid(
            &serde_json::json!([id.to_string()]),
            &children
        ));
        assert!(!contains_any_uuid(
            &serde_json::json!([Uuid::new_v4().to_string()]),
            &children
        ));
        let mut impact = AttributeReferenceImpact {
            inspection_complete: true,
            ..Default::default()
        };
        let resource = Uuid::new_v4();
        inspect_rule_targets(
            &serde_json::json!({"options": [id]}),
            &children,
            resource,
            AttributeReferenceKind::CloneSnapshot,
            &mut impact,
        );
        assert!(!impact.can_archive());
        assert_eq!(impact.references[0].resource_id, resource);
        assert!(!contains_uuid(&Value::Null, id));
    }
    #[test]
    fn incomplete_and_bounded_evidence_never_claims_unused() {
        let mut impact = AttributeReferenceImpact {
            inspection_complete: true,
            ..Default::default()
        };
        assert!(impact.can_archive());
        for _ in 0..10 {
            impact.observe(AttributeReferenceKind::VariantValue, Uuid::new_v4());
        }
        assert_eq!(impact.references.len(), 8);
        assert!(impact.has_more);
        assert!(!impact.can_archive());
        impact.references.clear();
        impact.has_more = false;
        impact.inspection_complete = false;
        assert!(!impact.can_archive());
    }
    #[test]
    fn malformed_rules_unknown_clones_and_defaults_never_prove_unused() {
        let target = Uuid::new_v4();
        let resource = Uuid::new_v4();
        let mut impact = AttributeReferenceImpact {
            inspection_complete: true,
            ..Default::default()
        };
        inspect_rules(
            &serde_json::json!({"options": "broken"}),
            target,
            resource,
            AttributeReferenceKind::OptionRule,
            &mut impact,
        );
        assert!(!impact.can_archive());
        assert!(!impact.inspection_complete);
        assert!(!known_clone_shape(
            &serde_json::json!([{"attribute_id": target, "future_reference": target}])
        ));
        let option = AttributeOptionDefinition {
            id: target,
            attribute_id: Uuid::new_v4(),
            code: "red".into(),
            position: 0,
            is_archived: false,
            attribute_archived: false,
            label: None,
            state_token: String::new(),
        };
        let mut impact = AttributeReferenceImpact {
            inspection_complete: true,
            ..Default::default()
        };
        inspect_default(&serde_json::json!("red"), &option, resource, &mut impact);
        assert!(!impact.can_archive());
        assert_eq!(impact.references[0].resource_id, resource);
        let mut impact = AttributeReferenceImpact {
            inspection_complete: true,
            ..Default::default()
        };
        inspect_default(
            &serde_json::json!({"undocumented": "other"}),
            &option,
            resource,
            &mut impact,
        );
        assert!(!impact.inspection_complete);
        assert!(!impact.can_archive());
        // A valid literal regex is not an option constraint just because its text is a UUID.
        let mut impact = AttributeReferenceImpact {
            inspection_complete: true,
            ..Default::default()
        };
        inspect_rules(
            &serde_json::json!({"pattern": target.to_string()}),
            target,
            resource,
            AttributeReferenceKind::OptionRule,
            &mut impact,
        );
        assert!(impact.can_archive());
    }

    #[tokio::test]
    async fn json_payload_limits_are_applied_before_decoding_with_explicit_incompleteness() {
        use sea_orm::TransactionTrait;
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(
            "CREATE TABLE lifecycle_payloads (resource_id TEXT, tenant_id TEXT, payload TEXT)",
        )
        .await
        .unwrap();
        let tenant = Uuid::new_v4();
        for payload in [
            serde_json::json!({}),
            serde_json::json!("x".repeat(MAX_REFERENCE_JSON_ROW_BYTES + 1)),
        ] {
            db.execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "INSERT INTO lifecycle_payloads VALUES ($1, $2, $3)",
                vec![Uuid::new_v4().into(), tenant.into(), payload.into()],
            ))
            .await
            .unwrap();
        }
        let txn = db.begin().await.unwrap();
        let mut impact = AttributeReferenceImpact {
            inspection_complete: true,
            ..Default::default()
        };
        let rows = json_rows(
            &txn,
            tenant,
            None,
            "SELECT resource_id, payload FROM lifecycle_payloads WHERE tenant_id = $1",
            &mut impact,
            &mut InspectionBudget::default(),
        )
        .await
        .unwrap();
        assert!(rows.len() <= 1);
        assert!(!impact.inspection_complete);
        assert!(!impact.can_archive());
        let mut impact = AttributeReferenceImpact {
            inspection_complete: true,
            ..Default::default()
        };
        let rows = json_rows(
            &txn,
            tenant,
            None,
            "SELECT resource_id, payload FROM lifecycle_payloads WHERE tenant_id = $1",
            &mut impact,
            &mut InspectionBudget {
                rows: 0,
                bytes: MAX_REFERENCE_JSON_TOTAL_BYTES,
                bindings: 0,
            },
        )
        .await
        .unwrap();
        assert!(rows.is_empty());
        assert!(!impact.inspection_complete);
        assert!(impact.has_more);
        txn.rollback().await.unwrap();
    }
}
