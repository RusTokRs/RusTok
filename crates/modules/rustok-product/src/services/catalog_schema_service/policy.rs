/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

//! Schema policy changes and Product writes share a tenant-scoped transaction fence.
//! Policy is checked against the effective inherited form, never a second option model.

use super::*;
use crate::dto::{
    VariantAxisPolicy, VariantAxisPolicyConflict, VariantAxisPolicyViolation,
    VariantAxisPolicyViolationKind,
};

const IMPACT_PAGE_SIZE: usize = 128;
const MAX_POLICY_VIOLATIONS: usize = 8;
const MAX_PUBLISH_REQUIREMENT_VIOLATIONS: usize = 8;

type CategoryAxisPolicies = HashMap<Uuid, VariantAxisPolicy>;
pub(super) type AxisPolicySnapshot = HashMap<Uuid, CategoryAxisPolicies>;

/// Lock order: schema fence -> Product -> Variant/value. Readers may write different Products
/// concurrently; schema writers exclude them until validation, outbox and receipts commit.
pub(crate) async fn lock_schema_for_product_write(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
) -> CommerceResult<()> {
    lock_schema(txn, tenant_id, false).await
}

pub(super) async fn lock_schema_for_policy_write(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
) -> CommerceResult<()> {
    lock_schema(txn, tenant_id, true).await
}

async fn lock_schema(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    exclusive: bool,
) -> CommerceResult<()> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            if exclusive {
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))"
            } else {
                "SELECT pg_advisory_xact_lock_shared(hashtextextended($1, 0))"
            },
            vec![format!("product:catalog-schema:{tenant_id}").into()],
        ),
        // Even an empty UPDATE acquires SQLite's database writer reservation. Do this before
        // schema reads, rather than trying to upgrade a stale read snapshot later.
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "UPDATE catalog_categories SET updated_at = updated_at WHERE tenant_id = $1 AND 1 = 0",
            vec![tenant_id.into()],
        ),
        _ => {
            return Err(CommerceError::Validation(
                "Product schema policy requires PostgreSQL or SQLite".into(),
            ));
        }
    };
    txn.execute_raw(statement).await?;
    Ok(())
}

#[derive(Clone, Copy)]
pub(super) enum BindingOwner {
    Schema(Uuid),
    Category(Uuid),
}

#[derive(FromQueryResult)]
struct StoredAxisPolicy {
    variant_axis_policy: String,
    default_variant_axis: bool,
}

/// Omitted fields retain a previous bind's rules. Only a new binding defaults to forbidden.
/// Validate the merged pair, so `forbidden` cannot silently retain a true default hint.
pub(super) async fn resolve_binding_axis_policy_in(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    owner: BindingOwner,
    attribute_id: Uuid,
    policy: Option<VariantAxisPolicy>,
    default_axis: Option<bool>,
) -> CommerceResult<(VariantAxisPolicy, bool)> {
    let (table, owner_column, owner_id) = match owner {
        BindingOwner::Schema(id) => ("product_attribute_schema_attributes", "schema_id", id),
        BindingOwner::Category(id) => ("category_attributes", "category_id", id),
    };
    let stored = StoredAxisPolicy::find_by_statement(Statement::from_sql_and_values(
        txn.get_database_backend(),
        format!("SELECT variant_axis_policy, default_variant_axis FROM {table} WHERE tenant_id = $1 AND {owner_column} = $2 AND attribute_id = $3"),
        vec![tenant_id.into(), owner_id.into(), attribute_id.into()],
    ))
    .one(txn)
    .await?;
    let stored_policy = stored
        .as_ref()
        .map(|row| parse_stored_policy(&row.variant_axis_policy))
        .transpose()?
        .unwrap_or_default();
    let policy = policy.unwrap_or(stored_policy);
    let default_axis =
        default_axis.unwrap_or_else(|| stored.as_ref().is_some_and(|row| row.default_variant_axis));
    validate_policy_hint(policy, default_axis)?;

    if policy != VariantAxisPolicy::Forbidden {
        let definition = load_attribute_write_definition(txn, tenant_id, attribute_id).await?;
        validate_axis_attribute(&definition.value_type, &definition.scope)?;
    }
    Ok((policy, default_axis))
}

pub(super) fn parse_stored_policy(value: &str) -> CommerceResult<VariantAxisPolicy> {
    value.parse().map_err(|_| {
        CommerceError::Core(rustok_core::error::Error::Internal(
            "Stored variant axis policy is invalid".into(),
        ))
    })
}

pub(super) fn validate_policy_hint(
    policy: VariantAxisPolicy,
    default_axis: bool,
) -> CommerceResult<()> {
    if policy == VariantAxisPolicy::Forbidden && default_axis {
        return Err(CommerceError::Validation(
            "default_variant_axis cannot be true when variant_axis_policy is forbidden".into(),
        ));
    }
    Ok(())
}

fn validate_axis_attribute(value_type: &str, scope: &str) -> CommerceResult<()> {
    if value_type != "select" || !matches!(scope, "variant" | "both") {
        return Err(CommerceError::Validation(
            "Allowed and required variant axes must use an active select attribute with variant or both scope".into(),
        ));
    }
    Ok(())
}

impl ProductCatalogSchemaService {
    pub(super) async fn schema_policy_snapshot_in(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
    ) -> CommerceResult<SchemaPolicySnapshot> {
        Ok(SchemaPolicySnapshot {
            axis_policies: Self::axis_policy_snapshot_in(txn, tenant_id).await?,
            publish_requirements: Self::publish_requirement_snapshot_in(txn, tenant_id).await?,
        })
    }

    /// Revalidates active Products after effective schema policy changes. The caller holds the
    /// exclusive schema fence and the same transaction that wrote the new binding.
    pub(super) async fn validate_schema_policy_change_in(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        previous: SchemaPolicySnapshot,
    ) -> CommerceResult<()> {
        let next = Self::schema_policy_snapshot_in(txn, tenant_id).await?;
        Self::validate_axis_policy_impacts_in(
            txn,
            tenant_id,
            &previous.axis_policies,
            &next.axis_policies,
        )
        .await?;
        Self::validate_publish_requirement_impacts_in(
            txn,
            tenant_id,
            &previous.publish_requirements,
            &next.publish_requirements,
        )
        .await
    }

    async fn publish_requirement_snapshot_in(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
    ) -> CommerceResult<PublishRequirementSnapshot> {
        let categories = Self::load_category_schema_map(txn, tenant_id).await?;
        let schemas = Self::load_attribute_schema_map(txn, tenant_id).await?;
        let mut category_ids = Vec::new();
        let mut forms = Vec::new();
        for category in categories
            .values()
            .filter(|category| category.kind == CatalogCategoryKind::Structural)
        {
            let form = resolve_effective_product_form(
                category.category_id,
                &categories,
                &schemas,
                &[],
            )
            .map_err(map_schema_resolution_error)?;
            category_ids.push(category.category_id);
            forms.push(form);
        }
        super::effective_forms::load_effective_attribute_validation_for_forms(
            txn,
            tenant_id,
            &mut forms,
        )
        .await?;

        let mut signatures = HashMap::new();
        let mut effective_forms = HashMap::new();
        for (category_id, form) in category_ids.into_iter().zip(forms) {
            let requirements = publish_requirements_for_form(&form)?;
            if !requirements.is_empty() {
                signatures.insert(category_id, requirements);
                effective_forms.insert(category_id, form);
            }
        }
        Ok(PublishRequirementSnapshot {
            signatures,
            forms: effective_forms,
        })
    }

    async fn validate_publish_requirement_impacts_in(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        previous: &PublishRequirementSnapshot,
        next: &PublishRequirementSnapshot,
    ) -> CommerceResult<()> {
        let mut changed = previous
            .signatures
            .keys()
            .chain(next.signatures.keys())
            .copied()
            .collect::<HashSet<_>>();
        changed.retain(|category_id| {
            publish_requirements_tightened(
                previous.signatures.get(category_id),
                next.signatures.get(category_id),
            )
        });
        let mut changed = changed.into_iter().collect::<Vec<_>>();
        changed.sort_unstable();
        if changed.is_empty() {
            return Ok(());
        }

        let mut violations = Vec::new();
        let mut has_more = false;
        'category_chunks: for category_ids in changed.chunks(IMPACT_PAGE_SIZE) {
            let mut cursor: Option<Uuid> = None;
            loop {
                let (placeholders, mut values) = uuid_filter_values(tenant_id, category_ids);
                let cursor_clause = match cursor {
                    Some(id) => {
                        values.push(id.into());
                        format!(" AND id > ${}", values.len())
                    }
                    None => String::new(),
                };
                let products = PolicyProductRow::find_by_statement(Statement::from_sql_and_values(
                    txn.get_database_backend(),
                    format!("SELECT id, primary_category_id FROM products WHERE tenant_id = $1 AND status = 'active' AND primary_category_id IN ({placeholders}){cursor_clause} ORDER BY id LIMIT {IMPACT_PAGE_SIZE}"),
                    values,
                ))
                .all(txn)
                .await?;
                if products.is_empty() {
                    break;
                }

                for product in &products {
                    let form = next.forms.get(&product.primary_category_id).ok_or_else(|| {
                        CommerceError::Core(rustok_core::error::Error::Internal(format!(
                            "Effective Product schema snapshot is missing category {}",
                            product.primary_category_id
                        )))
                    })?;
                    if form.category_id != product.primary_category_id {
                        return Err(CommerceError::Core(rustok_core::error::Error::Internal(
                            "Effective Product schema snapshot category mismatch".to_owned(),
                        )));
                    }
                    match ProductCatalogSchemaService::validate_product_publish_requirements_for_form_in(
                        txn,
                        tenant_id,
                        product.id,
                        form,
                    )
                    .await
                    {
                        Ok(()) => {}
                        Err(CommerceError::Validation(message)) => {
                            if violations.len() < MAX_PUBLISH_REQUIREMENT_VIOLATIONS {
                                violations.push(format!("Product {}: {}", product.id, message));
                            } else {
                                has_more = true;
                                break 'category_chunks;
                            }
                        }
                        Err(error) => return Err(error),
                    }
                }

                cursor = products.last().map(|product| product.id);
                if products.len() < IMPACT_PAGE_SIZE {
                    break;
                }
            }
        }

        if violations.is_empty() {
            return Ok(());
        }
        let mut message = format!(
            "Schema change would invalidate active Products: {}",
            violations.join("; ")
        );
        if has_more {
            message.push_str("; additional affected Products omitted");
        }
        Err(CommerceError::Validation(message))
    }

    pub(super) async fn axis_policy_snapshot_in(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
    ) -> CommerceResult<AxisPolicySnapshot> {
        let categories = Self::load_category_schema_map(txn, tenant_id).await?;
        let schemas = Self::load_attribute_schema_map(txn, tenant_id).await?;
        let mut policies = HashMap::new();
        for category in categories
            .values()
            .filter(|category| category.kind == CatalogCategoryKind::Structural)
        {
            let form =
                resolve_effective_product_form(category.category_id, &categories, &schemas, &[])
                    .map_err(map_schema_resolution_error)?;
            let mut attributes = HashMap::new();
            for binding in form
                .attributes
                .into_iter()
                .filter(|binding| !binding.is_disabled)
            {
                let policy = parse_stored_policy(&binding.variant_axis_policy)?;
                validate_policy_hint(policy, binding.default_variant_axis)?;
                if policy != VariantAxisPolicy::Forbidden {
                    attributes.insert(binding.attribute_id, policy);
                }
            }
            policies.insert(category.category_id, attributes);
        }
        Ok(policies)
    }


    async fn validate_axis_policy_impacts_in(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        previous: &AxisPolicySnapshot,
        next: &AxisPolicySnapshot,
    ) -> CommerceResult<()> {
        let mut changed = next
            .iter()
            .filter(|(category_id, policies)| previous.get(*category_id) != Some(*policies))
            .map(|(category_id, _)| *category_id)
            .collect::<Vec<_>>();
        changed.sort();
        let mut violations = Vec::new();
        for category_ids in changed.chunks(IMPACT_PAGE_SIZE) {
            Self::validate_category_chunk_impacts(
                txn,
                tenant_id,
                category_ids,
                next,
                &mut violations,
            )
            .await?;
        }
        if !violations.is_empty() {
            return Err(CommerceError::VariantAxisPolicyConflict(
                VariantAxisPolicyConflict {
                    violations,
                    has_more: false,
                },
            ));
        }
        Ok(())
    }

    async fn validate_category_chunk_impacts(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_ids: &[Uuid],
        next: &AxisPolicySnapshot,
        violations: &mut Vec<VariantAxisPolicyViolation>,
    ) -> CommerceResult<()> {
        let mut cursor: Option<Uuid> = None;
        loop {
            let (placeholders, mut values) = uuid_filter_values(tenant_id, category_ids);
            let cursor_clause = match cursor {
                Some(id) => {
                    values.push(id.into());
                    format!(" AND id > ${}", values.len())
                }
                None => String::new(),
            };
            let products = PolicyProductRow::find_by_statement(Statement::from_sql_and_values(
                txn.get_database_backend(),
                format!("SELECT id, primary_category_id FROM products WHERE tenant_id = $1 AND primary_category_id IN ({placeholders}){cursor_clause} ORDER BY id LIMIT {IMPACT_PAGE_SIZE}"),
                values,
            )).all(txn).await?;
            if products.is_empty() {
                break;
            }
            let product_ids = products
                .iter()
                .map(|product| product.id)
                .collect::<Vec<_>>();
            let (placeholders, values) = uuid_filter_values(tenant_id, &product_ids);
            let rows = PolicyAxisRow::find_by_statement(Statement::from_sql_and_values(
                txn.get_database_backend(),
                format!("SELECT product_id, attribute_id FROM product_variant_axes WHERE tenant_id = $1 AND product_id IN ({placeholders}) ORDER BY product_id, attribute_id"),
                values,
            )).all(txn).await?;
            let mut axes = HashMap::<Uuid, HashSet<Uuid>>::new();
            for row in rows {
                axes.entry(row.product_id)
                    .or_default()
                    .insert(row.attribute_id);
            }
            for product in &products {
                let configured = axes.remove(&product.id).unwrap_or_default();
                let policies = next.get(&product.primary_category_id).ok_or_else(|| {
                    CommerceError::Core(rustok_core::error::Error::Internal(
                        "Effective category policy disappeared".into(),
                    ))
                })?;
                violations.extend(policy_violations(
                    product.id,
                    product.primary_category_id,
                    policies,
                    &configured,
                ));
                if violations.len() > MAX_POLICY_VIOLATIONS {
                    violations.truncate(MAX_POLICY_VIOLATIONS);
                    return Err(CommerceError::VariantAxisPolicyConflict(
                        VariantAxisPolicyConflict {
                            violations: violations.clone(),
                            has_more: true,
                        },
                    ));
                }
            }
            cursor = products.last().map(|product| product.id);
            if products.len() < IMPACT_PAGE_SIZE {
                break;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PublishRequirementAttribute {
    scope: String,
    required_locales: Vec<String>,
}

type PublishRequirementAttributes = HashMap<Uuid, PublishRequirementAttribute>;

#[derive(Clone, Debug)]
struct PublishRequirementSnapshot {
    signatures: HashMap<Uuid, PublishRequirementAttributes>,
    forms: HashMap<Uuid, EffectiveProductForm>,
}

#[derive(Clone, Debug)]
pub(super) struct SchemaPolicySnapshot {
    axis_policies: AxisPolicySnapshot,
    publish_requirements: PublishRequirementSnapshot,
}

fn scope_includes_publish_requirement(scope: &str, target: &str) -> bool {
    match (scope, target) {
        ("product" | "both", "product") | ("variant" | "both", "variant") => true,
        _ => false,
    }
}

fn publish_requirements_tightened(
    previous: Option<&PublishRequirementAttributes>,
    next: Option<&PublishRequirementAttributes>,
) -> bool {
    let Some(next) = next else {
        return false;
    };
    for (attribute_id, next_requirement) in next {
        let Some(previous_requirement) = previous.and_then(|values| values.get(attribute_id)) else {
            return true;
        };
        for target in ["product", "variant"] {
            if scope_includes_publish_requirement(next_requirement.scope.as_str(), target)
                && !scope_includes_publish_requirement(previous_requirement.scope.as_str(), target)
            {
                return true;
            }
        }
        if next_requirement
            .required_locales
            .iter()
            .any(|locale| !previous_requirement.required_locales.contains(locale))
        {
            return true;
        }
    }
    false
}

fn publish_requirements_for_form(
    form: &EffectiveProductForm,
) -> CommerceResult<PublishRequirementAttributes> {
    let mut requirements = HashMap::new();
    for binding in form.attributes.iter().filter(|binding| !binding.is_disabled) {
        let rules =
            super::attribute_validation::parse_product_attribute_validation(&binding.validation)?;
        let is_required =
            binding.is_required || rules.required || !rules.required_locales.is_empty();
        if !is_required {
            continue;
        }
        let mut required_locales = rules.required_locales;
        required_locales.sort_unstable();
        required_locales.dedup();
        requirements.insert(
            binding.attribute_id,
            PublishRequirementAttribute {
                scope: binding.scope.clone(),
                required_locales,
            },
        );
    }
    Ok(requirements)
}

#[derive(FromQueryResult)]
struct PolicyProductRow {
    id: Uuid,
    primary_category_id: Uuid,
}

#[derive(FromQueryResult)]
struct PolicyAxisRow {
    product_id: Uuid,
    attribute_id: Uuid,
}

fn policy_violations(
    product_id: Uuid,
    category_id: Uuid,
    policies: &CategoryAxisPolicies,
    configured: &HashSet<Uuid>,
) -> Vec<VariantAxisPolicyViolation> {
    let mut violations = configured
        .iter()
        .filter(|attribute| !policies.contains_key(*attribute))
        .map(|attribute_id| VariantAxisPolicyViolation {
            product_id,
            category_id,
            attribute_id: *attribute_id,
            kind: VariantAxisPolicyViolationKind::AxisNotAllowed,
        })
        .chain(
            policies
                .iter()
                .filter(|(attribute, policy)| {
                    **policy == VariantAxisPolicy::Required && !configured.contains(*attribute)
                })
                .map(|(attribute_id, _)| VariantAxisPolicyViolation {
                    product_id,
                    category_id,
                    attribute_id: *attribute_id,
                    kind: VariantAxisPolicyViolationKind::RequiredAxisMissing,
                }),
        )
        .collect::<Vec<_>>();
    violations.sort_by_key(|violation| violation.attribute_id);
    violations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_variant_select_attributes_can_be_axes() {
        for scope in ["variant", "both"] {
            assert!(validate_axis_attribute("select", scope).is_ok());
        }
        for (kind, scope) in [
            ("select", "product"),
            ("multiselect", "variant"),
            ("text", "both"),
        ] {
            assert!(validate_axis_attribute(kind, scope).is_err());
        }
        assert!(validate_policy_hint(VariantAxisPolicy::Forbidden, true).is_err());
        assert!(validate_policy_hint(VariantAxisPolicy::Allowed, true).is_ok());
        assert!("allwed".parse::<VariantAxisPolicy>().is_err());
    }

    #[test]
    fn publish_requirement_signature_tracks_requiredness_scope_and_locales() {
        let attribute_id = Uuid::from_u128(77);
        let binding = |scope: &str, is_required: bool, is_disabled: bool, validation: serde_json::Value| {
            AttributeBinding {
                attribute_id,
                group_code: None,
                scope: scope.to_owned(),
                is_required,
                is_disabled,
                position: 0,
                visibility_overrides: Default::default(),
                validation_overrides: serde_json::json!({}),
                validation,
                source: EffectiveAttributeSource::Schema,
                variant_axis_policy: "forbidden".to_owned(),
                default_variant_axis: false,
            }
        };
        let form = |binding| EffectiveProductForm {
            category_id: Uuid::nil(),
            attributes: vec![binding],
            detached_attribute_ids: Vec::new(),
        };

        assert!(publish_requirements_for_form(&form(binding(
            "product", false, false, serde_json::json!({})
        )))
        .unwrap()
        .is_empty());
        assert!(publish_requirements_for_form(&form(binding(
            "product", true, true, serde_json::json!({})
        )))
        .unwrap()
        .is_empty());

        let product = publish_requirements_for_form(&form(binding(
            "product", true, false, serde_json::json!({})
        )))
        .unwrap();
        let variant = publish_requirements_for_form(&form(binding(
            "variant", true, false, serde_json::json!({})
        )))
        .unwrap();
        assert_ne!(product, variant);

        let localized = publish_requirements_for_form(&form(binding(
            "product",
            false,
            false,
            serde_json::json!({"requiredLocales": ["fr", "en"]}),
        )))
        .unwrap();
        assert_eq!(
            localized[&attribute_id].required_locales,
            vec!["en".to_owned(), "fr".to_owned()]
        );
    }

    #[test]
    fn publish_requirement_impacts_only_scan_tightening_changes() {
        let attribute_id = Uuid::from_u128(88);
        let requirement = |scope: &str, required_locales: &[&str]| {
            HashMap::from([(
                attribute_id,
                PublishRequirementAttribute {
                    scope: scope.to_owned(),
                    required_locales: required_locales.iter().map(|locale| (*locale).to_owned()).collect(),
                },
            )])
        };
        let product = requirement("product", &["en"]);
        let both = requirement("both", &["en"]);
        let product_and_more_locales = requirement("product", &["en", "fr"]);
        assert!(publish_requirements_tightened(None, Some(&product)));
        assert!(publish_requirements_tightened(Some(&product), Some(&both)));
        assert!(publish_requirements_tightened(Some(&product), Some(&product_and_more_locales)));
        assert!(!publish_requirements_tightened(Some(&both), Some(&product)));
        assert!(!publish_requirements_tightened(Some(&product_and_more_locales), Some(&product)));
        assert!(!publish_requirements_tightened(Some(&product), None));
    }

    #[test]
    fn policy_change_reports_forbidden_and_missing_required_axes_without_touching_skus() {
        let color = Uuid::from_u128(1);
        let size = Uuid::from_u128(2);
        let configured = HashSet::from([color]);
        let policies = HashMap::from([(size, VariantAxisPolicy::Required)]);
        let result = policy_violations(
            Uuid::from_u128(3),
            Uuid::from_u128(4),
            &policies,
            &configured,
        );
        assert_eq!(result.len(), 2);
        assert_eq!(
            result[0].kind,
            VariantAxisPolicyViolationKind::AxisNotAllowed
        );
        assert_eq!(
            result[1].kind,
            VariantAxisPolicyViolationKind::RequiredAxisMissing
        );
        assert!(
            policy_violations(
                Uuid::nil(),
                Uuid::nil(),
                &HashMap::from([(color, VariantAxisPolicy::Allowed)]),
                &configured
            )
            .is_empty()
        );
        assert!(
            policy_violations(Uuid::nil(), Uuid::nil(), &HashMap::new(), &HashSet::new())
                .is_empty()
        );
    }
    async fn policy_test_database() -> DatabaseConnection {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(r#"
            CREATE TABLE catalog_categories (tenant_id TEXT, updated_at TEXT);
            CREATE TABLE product_attributes (id TEXT, tenant_id TEXT, value_type TEXT, scope TEXT, is_localized INTEGER, validation TEXT DEFAULT '{}', archived_at TEXT);
            CREATE TABLE product_attribute_schema_attributes (tenant_id TEXT, schema_id TEXT, attribute_id TEXT, variant_axis_policy TEXT, default_variant_axis INTEGER);
            CREATE TABLE category_attributes (tenant_id TEXT, category_id TEXT, attribute_id TEXT, variant_axis_policy TEXT, default_variant_axis INTEGER);
            CREATE TABLE products (id TEXT, tenant_id TEXT, primary_category_id TEXT);
            CREATE TABLE product_variant_axes (tenant_id TEXT, product_id TEXT, attribute_id TEXT);
        "#).await.unwrap();
        db
    }

    #[tokio::test]
    async fn rebind_merges_omitted_rules_and_checks_the_merged_default_without_cross_tenant_reads()
    {
        use sea_orm::TransactionTrait;
        let db = policy_test_database().await;
        let tenant = Uuid::new_v4();
        let schema = Uuid::new_v4();
        let attribute = Uuid::new_v4();
        db.execute_raw(Statement::from_sql_and_values(DatabaseBackend::Sqlite,
            "INSERT INTO product_attributes (id, tenant_id, value_type, scope, is_localized) VALUES ($1, $2, 'select', 'variant', 0)", vec![attribute.into(), tenant.into()])).await.unwrap();
        db.execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO product_attribute_schema_attributes VALUES ($1, $2, $3, 'allowed', 1)",
            vec![tenant.into(), schema.into(), attribute.into()],
        ))
        .await
        .unwrap();
        let txn = db.begin().await.unwrap();
        lock_schema_for_policy_write(&txn, tenant).await.unwrap();
        let owner = BindingOwner::Schema(schema);
        assert_eq!(
            resolve_binding_axis_policy_in(&txn, tenant, owner, attribute, None, None)
                .await
                .unwrap(),
            (VariantAxisPolicy::Allowed, true)
        );
        assert!(
            resolve_binding_axis_policy_in(
                &txn,
                tenant,
                owner,
                attribute,
                Some(VariantAxisPolicy::Forbidden),
                None
            )
            .await
            .is_err()
        );
        assert_eq!(
            resolve_binding_axis_policy_in(
                &txn,
                tenant,
                owner,
                attribute,
                Some(VariantAxisPolicy::Forbidden),
                Some(false)
            )
            .await
            .unwrap(),
            (VariantAxisPolicy::Forbidden, false)
        );
        assert_eq!(
            resolve_binding_axis_policy_in(&txn, Uuid::new_v4(), owner, attribute, None, None)
                .await
                .unwrap(),
            (VariantAxisPolicy::Forbidden, false)
        );
        assert!(
            resolve_binding_axis_policy_in(
                &txn,
                Uuid::new_v4(),
                owner,
                attribute,
                Some(VariantAxisPolicy::Allowed),
                None
            )
            .await
            .is_err()
        );
        txn.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn impact_refusal_is_bounded_tenant_scoped_and_the_proposed_binding_rolls_back() {
        use sea_orm::TransactionTrait;
        let db = policy_test_database().await;
        let tenant = Uuid::new_v4();
        let category = Uuid::new_v4();
        let attribute = Uuid::new_v4();
        db.execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO category_attributes VALUES ($1, $2, $3, 'allowed', 0)",
            vec![tenant.into(), category.into(), attribute.into()],
        ))
        .await
        .unwrap();
        let mut product_ids = HashSet::new();
        for _ in 0..130 {
            let product = Uuid::new_v4();
            product_ids.insert(product);
            db.execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "INSERT INTO products VALUES ($1, $2, $3)",
                vec![product.into(), tenant.into(), category.into()],
            ))
            .await
            .unwrap();
            db.execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "INSERT INTO product_variant_axes VALUES ($1, $2, $3)",
                vec![tenant.into(), product.into(), attribute.into()],
            ))
            .await
            .unwrap();
        }
        let previous = HashMap::from([(
            category,
            HashMap::from([(attribute, VariantAxisPolicy::Allowed)]),
        )]);
        let next = HashMap::from([(category, HashMap::new())]);
        let txn = db.begin().await.unwrap();
        lock_schema_for_policy_write(&txn, tenant).await.unwrap();
        ProductCatalogSchemaService::validate_axis_policy_impacts_in(
            &txn,
            Uuid::new_v4(),
            &previous,
            &next,
        )
        .await
        .unwrap();
        txn.execute_unprepared("UPDATE category_attributes SET variant_axis_policy = 'forbidden'")
            .await
            .unwrap();
        let error = ProductCatalogSchemaService::validate_axis_policy_impacts_in(
            &txn, tenant, &previous, &next,
        )
        .await
        .unwrap_err();
        let CommerceError::VariantAxisPolicyConflict(conflict) = error else {
            panic!("expected a typed conflict");
        };
        assert_eq!(conflict.violations.len(), MAX_POLICY_VIOLATIONS);
        assert!(conflict.has_more);
        assert!(conflict.violations.iter().all(|violation| {
            product_ids.contains(&violation.product_id)
                && violation.category_id == category
                && violation.attribute_id == attribute
                && violation.kind == VariantAxisPolicyViolationKind::AxisNotAllowed
        }));
        txn.rollback().await.unwrap();
        let stored = StoredAxisPolicy::find_by_statement(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT variant_axis_policy, default_variant_axis FROM category_attributes".to_string(),
        ))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
        assert_eq!(stored.variant_axis_policy, "allowed");
    }

    #[tokio::test]
    async fn required_axis_checks_scan_past_a_full_nonconflicting_product_page() {
        use sea_orm::TransactionTrait;
        let db = policy_test_database().await;
        let tenant = Uuid::new_v4();
        let category = Uuid::new_v4();
        let attribute = Uuid::new_v4();
        for index in 1..=129_u128 {
            let product = Uuid::from_u128(index);
            db.execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "INSERT INTO products VALUES ($1, $2, $3)",
                vec![product.into(), tenant.into(), category.into()],
            ))
            .await
            .unwrap();
            if index <= 128 {
                db.execute_raw(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    "INSERT INTO product_variant_axes VALUES ($1, $2, $3)",
                    vec![tenant.into(), product.into(), attribute.into()],
                ))
                .await
                .unwrap();
            }
        }
        let previous = HashMap::from([(
            category,
            HashMap::from([(attribute, VariantAxisPolicy::Allowed)]),
        )]);
        let next = HashMap::from([(
            category,
            HashMap::from([(attribute, VariantAxisPolicy::Required)]),
        )]);
        let txn = db.begin().await.unwrap();
        let error = ProductCatalogSchemaService::validate_axis_policy_impacts_in(
            &txn, tenant, &previous, &next,
        )
        .await
        .unwrap_err();
        let CommerceError::VariantAxisPolicyConflict(conflict) = error else {
            panic!("expected required-axis conflict");
        };
        assert_eq!(conflict.violations.len(), 1);
        assert!(!conflict.has_more);
        assert_eq!(conflict.violations[0].product_id, Uuid::from_u128(129));
        assert_eq!(
            conflict.violations[0].kind,
            VariantAxisPolicyViolationKind::RequiredAxisMissing
        );
        txn.rollback().await.unwrap();
    }

    #[test]
    fn unknown_stored_policies_fail_closed() {
        for invalid in ["", "ALLOW", "Allowed", "future_policy"] {
            assert!(parse_stored_policy(invalid).is_err());
        }
        for policy in ["forbidden", "allowed", "required"] {
            assert_eq!(parse_stored_policy(policy).unwrap().as_str(), policy);
        }
    }
}
