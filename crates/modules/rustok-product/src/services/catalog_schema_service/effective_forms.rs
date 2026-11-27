use super::*;
use crate::ports::{AsTaxonomyConnection, ProductTaxonomyCategorySnapshot, ProductTaxonomyPort};

const EFFECTIVE_ATTRIBUTE_DEFINITION_BATCH_SIZE: usize = 500;

#[derive(FromQueryResult)]
struct ProductCategoryIdRow {
    id: Uuid,
}

#[derive(FromQueryResult)]
struct AttributeValidationRow {
    id: Uuid,
    scope: String,
    value_type: String,
    is_localized: bool,
    validation: Value,
}

/// Merges every binding's declared `validation` object under its schema/category
/// `validation_overrides`, so enforcement and the effective-form projection share one
/// effective rule object per attribute.
async fn load_effective_attribute_validation<C>(
    db: &C,
    tenant_id: Uuid,
    form: &mut EffectiveProductForm,
) -> CommerceResult<()>
where
    C: ConnectionTrait,
{
    load_effective_attribute_validation_for_forms(db, tenant_id, std::slice::from_mut(form)).await
}

/// Hydrates multiple effective category forms from one tenant-scoped definition query.
/// Schema-impact scans use this to avoid an attribute query for every category.
pub(super) async fn load_effective_attribute_validation_for_forms<C>(
    db: &C,
    tenant_id: Uuid,
    forms: &mut [EffectiveProductForm],
) -> CommerceResult<()>
where
    C: ConnectionTrait,
{
    let mut attribute_ids = forms
        .iter()
        .flat_map(|form| form.attributes.iter().map(|binding| binding.attribute_id))
        .collect::<Vec<_>>();
    attribute_ids.sort_unstable();
    attribute_ids.dedup();
    if attribute_ids.is_empty() {
        return Ok(());
    }

    let mut declared = HashMap::new();
    for attribute_id_chunk in attribute_ids.chunks(EFFECTIVE_ATTRIBUTE_DEFINITION_BATCH_SIZE) {
        let (placeholders, values) = uuid_filter_values(tenant_id, attribute_id_chunk);
        let rows = AttributeValidationRow::find_by_statement(Statement::from_sql_and_values(
            db.get_database_backend(),
            format!(
                "SELECT id, scope, value_type, is_localized, validation FROM product_attributes WHERE tenant_id = $1 AND archived_at IS NULL AND id IN ({placeholders})"
            ),
            values,
        ))
        .all(db)
        .await?;
        for row in rows {
            declared.insert(
                row.id,
                (row.scope, row.value_type, row.is_localized, row.validation),
            );
        }
    }

    for form in forms {
        for binding in &mut form.attributes {
            let (scope, value_type, is_localized, base) =
                declared.get(&binding.attribute_id).ok_or_else(|| {
                    CommerceError::Core(rustok_core::error::Error::Internal(format!(
                        "Effective Product schema references missing or archived attribute {}",
                        binding.attribute_id
                    )))
                })?;
            if !matches!(scope.as_str(), "product" | "variant" | "both") {
                return Err(CommerceError::Validation(format!(
                    "Product attribute {} has unsupported scope",
                    binding.attribute_id
                )));
            }
            binding.scope = scope.clone();
            binding.validation = attribute_validation::merge_product_attribute_validation(
                base,
                &binding.validation_overrides,
            );
            attribute_validation::validate_declared_rule_keys(&binding.validation)?;
            let rules =
                attribute_validation::parse_product_attribute_validation(&binding.validation)?;
            attribute_validation::validate_required_locales_compatibility(
                value_type,
                *is_localized,
                &rules,
            )?;
        }
    }
    Ok(())
}

impl ProductCatalogSchemaService {
    pub async fn load_effective_form_for_product(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
    ) -> CommerceResult<Option<EffectiveProductForm>> {
        let taxonomy_port = self.taxonomy();
        Self::load_effective_form_for_product_in(
            &self.db,
            taxonomy_port.as_ref(),
            tenant_id,
            product_id,
        )
        .await
    }

    /// Effective form of a product's primary category.
    ///
    /// `pub(crate)` because the storefront attribute projection needs the same binding-level
    /// visibility overrides (and the same display order) the admin form uses.
    pub(crate) async fn load_effective_form_for_product_in<C>(
        db: &C,
        taxonomy_port: &dyn ProductTaxonomyPort,
        tenant_id: Uuid,
        product_id: Uuid,
    ) -> CommerceResult<Option<EffectiveProductForm>>
    where
        C: ConnectionTrait + AsTaxonomyConnection,
    {
        let product = ProductPrimaryCategoryRow::find_by_statement(Statement::from_sql_and_values(
            db.get_database_backend(),
            "SELECT primary_category_id FROM products WHERE tenant_id = $1 AND id = $2",
            vec![tenant_id.into(), product_id.into()],
        ))
        .one(db)
        .await?;
        let Some(primary_category_id) = product.and_then(|row| row.primary_category_id) else {
            return Ok(None);
        };

        let value_rows = AttributeIdRow::find_by_statement(Statement::from_sql_and_values(
            db.get_database_backend(),
            "SELECT attribute_id FROM product_attribute_values WHERE tenant_id = $1 AND product_id = $2",
            vec![tenant_id.into(), product_id.into()],
        ))
        .all(db)
        .await?;
        let existing_value_attribute_ids = value_rows
            .into_iter()
            .map(|row| row.attribute_id)
            .collect::<Vec<_>>();

        Self::load_effective_form_for_category_in(
            db,
            taxonomy_port,
            tenant_id,
            primary_category_id,
            &existing_value_attribute_ids,
        )
        .await
        .map(Some)
    }

    pub async fn load_effective_form_for_category(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        existing_value_attribute_ids: &[Uuid],
    ) -> CommerceResult<EffectiveProductForm> {
        let taxonomy_port = self.taxonomy();
        Self::load_effective_form_for_category_in(
            &self.db,
            taxonomy_port.as_ref(),
            tenant_id,
            category_id,
            existing_value_attribute_ids,
        )
        .await
    }

    pub(crate) async fn load_effective_form_for_category_in<C>(
        db: &C,
        taxonomy_port: &dyn ProductTaxonomyPort,
        tenant_id: Uuid,
        category_id: Uuid,
        existing_value_attribute_ids: &[Uuid],
    ) -> CommerceResult<EffectiveProductForm>
    where
        C: ConnectionTrait + AsTaxonomyConnection,
    {
        let categories = Self::load_category_schema_map(db, taxonomy_port, tenant_id).await?;
        let schemas = Self::load_attribute_schema_map(db, tenant_id).await?;
        let mut form = resolve_effective_product_form(
            category_id,
            &categories,
            &schemas,
            existing_value_attribute_ids,
        )
        .map_err(map_schema_resolution_error)?;
        for binding in &form.attributes {
            let policy = super::policy::parse_stored_policy(&binding.variant_axis_policy)?;
            super::policy::validate_policy_hint(policy, binding.default_variant_axis)?;
        }
        load_effective_attribute_validation(db, tenant_id, &mut form).await?;
        Ok(form)
    }

    pub async fn load_effective_form_group_labels(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        locale: &str,
    ) -> CommerceResult<HashMap<String, String>> {
        validate_locale(locale)?;
        let mut labels = HashMap::new();
        let taxonomy_port = self.taxonomy();
        let product_category_ids = load_product_category_ids(&self.db, tenant_id).await?;
        let parent_map = load_taxonomy_category_parent_map(
            &self.db,
            taxonomy_port.as_ref(),
            tenant_id,
            &product_category_ids,
            locale,
            Some(rustok_api::PLATFORM_FALLBACK_LOCALE),
        )
        .await?;
        let category_ids = taxonomy_ancestor_chain(category_id, &parent_map)?;

        if category_ids.is_empty() {
            return Ok(labels);
        }

        let (category_placeholders, mut category_values) =
            uuid_filter_values(tenant_id, &category_ids);
        let locale_placeholder = format!("${}", category_values.len() + 1);
        category_values.push(locale.trim().to_string().into());
        for row in EffectiveGroupLabelRow::find_by_statement(Statement::from_sql_and_values(
            self.db.get_database_backend(),
            format!(
                r#"
                SELECT g.category_id AS owner_id,
                       g.code,
                       COALESCE(t.label, g.code) AS label
                FROM category_attribute_groups g
                LEFT JOIN category_attribute_group_translations t
                  ON t.group_id = g.id AND t.locale = {locale_placeholder}
                WHERE g.tenant_id = $1
                  AND g.category_id IN ({category_placeholders})
                ORDER BY g.position ASC, g.code ASC
                "#
            ),
            category_values,
        ))
        .all(&self.db)
        .await?
        {
            let _ = row.owner_id;
            labels.insert(row.code, row.label);
        }

        let (schema_placeholders, mut schema_values) = uuid_filter_values(tenant_id, &category_ids);
        let schema_locale_placeholder = format!("${}", schema_values.len() + 1);
        schema_values.push(locale.trim().to_string().into());
        for row in EffectiveGroupLabelRow::find_by_statement(Statement::from_sql_and_values(
            self.db.get_database_backend(),
            format!(
                r#"
                SELECT a.category_id AS owner_id,
                       g.code,
                       COALESCE(t.label, g.code) AS label
                FROM category_attribute_schema_assignments a
                JOIN product_attribute_schema_groups g
                  ON g.schema_id = a.schema_id AND g.tenant_id = a.tenant_id
                LEFT JOIN product_attribute_schema_group_translations t
                  ON t.group_id = g.id AND t.locale = {schema_locale_placeholder}
                WHERE a.tenant_id = $1
                  AND a.category_id IN ({schema_placeholders})
                  AND a.mode = 'use_schema'
                ORDER BY g.position ASC, g.code ASC
                "#
            ),
            schema_values,
        ))
        .all(&self.db)
        .await?
        {
            let _ = row.owner_id;
            labels.entry(row.code).or_insert(row.label);
        }

        Ok(labels)
    }

    pub(super) async fn load_category_schema_map<C>(
        db: &C,
        taxonomy_port: &dyn ProductTaxonomyPort,
        tenant_id: Uuid,
    ) -> CommerceResult<HashMap<Uuid, CatalogCategorySchema>>
    where
        C: ConnectionTrait + AsTaxonomyConnection,
    {
        let category_rows = CategorySchemaRow::find_by_statement(Statement::from_sql_and_values(
            db.get_database_backend(),
            r#"
            SELECT
                c.id AS category_id,
                c.kind,
                COALESCE(a.mode, 'inherit') AS mode,
                a.schema_id,
                a.snapshot
            FROM catalog_categories c
            LEFT JOIN category_attribute_schema_assignments a
                ON a.category_id = c.id AND a.tenant_id = c.tenant_id
            WHERE c.tenant_id = $1 AND c.deleted_at IS NULL
            "#,
            vec![tenant_id.into()],
        ))
        .all(db)
        .await?;
        let category_ids = category_rows
            .iter()
            .map(|row| row.category_id)
            .collect::<Vec<_>>();
        let taxonomy_parent_map = load_taxonomy_category_parent_map(
            db,
            taxonomy_port,
            tenant_id,
            &category_ids,
            rustok_api::PLATFORM_FALLBACK_LOCALE,
            Some(rustok_api::PLATFORM_FALLBACK_LOCALE),
        )
        .await?;

        let local_rows = CategoryAttributeRow::find_by_statement(Statement::from_sql_and_values(
            db.get_database_backend(),
            r#"
            SELECT
                ca.category_id,
                ca.attribute_id,
                cag.code AS group_code,
                ca.binding_kind,
                ca.is_required,
                ca.is_disabled,
                ca.position,
                ca.visibility_overrides,
                ca.validation_overrides,
                ca.variant_axis_policy,
                ca.default_variant_axis
            FROM category_attributes ca
            LEFT JOIN category_attribute_groups cag ON cag.id = ca.group_id
            WHERE ca.tenant_id = $1
            "#,
            vec![tenant_id.into()],
        ))
        .all(db)
        .await?;

        let mut local_by_category: HashMap<Uuid, Vec<CategoryAttributeBinding>> = HashMap::new();
        for row in local_rows {
            local_by_category
                .entry(row.category_id)
                .or_default()
                .push(CategoryAttributeBinding {
                    attribute_id: row.attribute_id,
                    group_code: row.group_code,
                    binding_kind: CategoryAttributeBindingKind::from_storage(&row.binding_kind)
                        .map_err(map_schema_resolution_error)?,
                    is_required: row.is_required,
                    is_disabled: row.is_disabled,
                    position: row.position,
                    visibility_overrides: parse_visibility_overrides(row.visibility_overrides)?,
                    validation_overrides: row.validation_overrides,
                    variant_axis_policy: row.variant_axis_policy,
                    default_variant_axis: row.default_variant_axis,
                });
        }

        let mut categories = HashMap::new();
        for row in category_rows {
            let parent_category_id =
                *taxonomy_parent_map.get(&row.category_id).ok_or_else(|| {
                    super::taxonomy_projection_error(
                        "Product category is missing its Taxonomy hierarchy projection",
                    )
                })?;
            let mode =
                CategorySchemaMode::from_storage(&row.mode).map_err(map_schema_resolution_error)?;
            let clone_snapshot = if mode == CategorySchemaMode::CloneFromCategory {
                let snapshot = row.snapshot.ok_or_else(|| {
                    CommerceError::Core(rustok_core::error::Error::Internal(
                        "Category clone snapshot is missing".into(),
                    ))
                })?;
                serde_json::from_value(snapshot).map_err(|_| {
                    CommerceError::Core(rustok_core::error::Error::Internal(
                        "Category clone snapshot is invalid".into(),
                    ))
                })?
            } else {
                Vec::new()
            };
            categories.insert(
                row.category_id,
                CatalogCategorySchema {
                    category_id: row.category_id,
                    parent_category_id,
                    kind: CatalogCategoryKind::from_storage(&row.kind)
                        .map_err(map_schema_resolution_error)?,
                    mode,
                    schema_id: row.schema_id,
                    clone_snapshot,
                    local_attributes: local_by_category
                        .remove(&row.category_id)
                        .unwrap_or_default(),
                },
            );
        }

        Ok(categories)
    }

    pub(super) async fn load_attribute_schema_map<C>(
        db: &C,
        tenant_id: Uuid,
    ) -> CommerceResult<HashMap<Uuid, ProductAttributeSchema>>
    where
        C: ConnectionTrait,
    {
        let schema_rows = SchemaRow::find_by_statement(Statement::from_sql_and_values(
            db.get_database_backend(),
            "SELECT id, code FROM product_attribute_schemas WHERE tenant_id = $1 AND archived_at IS NULL",
            vec![tenant_id.into()],
        ))
        .all(db)
        .await?;

        let attr_rows = SchemaAttributeRow::find_by_statement(Statement::from_sql_and_values(
            db.get_database_backend(),
            r#"
            SELECT
                psa.schema_id,
                psa.attribute_id,
                psag.code AS group_code,
                psa.is_required,
                psa.is_disabled,
                psa.position,
                psa.visibility_overrides,
                psa.validation_overrides,
                psa.variant_axis_policy,
                psa.default_variant_axis
            FROM product_attribute_schema_attributes psa
            LEFT JOIN product_attribute_schema_groups psag ON psag.id = psa.group_id
            WHERE psa.tenant_id = $1
            "#,
            vec![tenant_id.into()],
        ))
        .all(db)
        .await?;

        let mut attrs_by_schema: HashMap<Uuid, Vec<AttributeBinding>> = HashMap::new();
        for row in attr_rows {
            attrs_by_schema
                .entry(row.schema_id)
                .or_default()
                .push(AttributeBinding {
                    attribute_id: row.attribute_id,
                    group_code: row.group_code,
                    scope: String::new(),
                    is_required: row.is_required,
                    is_disabled: row.is_disabled,
                    position: row.position,
                    visibility_overrides: parse_visibility_overrides(row.visibility_overrides)?,
                    validation_overrides: row.validation_overrides,
                    validation: Value::Object(Default::default()),
                    source: EffectiveAttributeSource::Schema,
                    variant_axis_policy: row
                        .variant_axis_policy
                        .unwrap_or_else(|| "forbidden".to_string()),
                    default_variant_axis: row.default_variant_axis.unwrap_or(false),
                });
        }

        Ok(schema_rows
            .into_iter()
            .map(|row| {
                (
                    row.id,
                    ProductAttributeSchema {
                        id: row.id,
                        code: row.code,
                        attributes: attrs_by_schema.remove(&row.id).unwrap_or_default(),
                    },
                )
            })
            .collect())
    }
}

const TAXONOMY_CATEGORY_READ_BATCH_SIZE: usize = 500;

async fn load_product_category_ids<C>(
    connection: &C,
    tenant_id: Uuid,
) -> CommerceResult<Vec<Uuid>>
where
    C: ConnectionTrait,
{
    let rows = ProductCategoryIdRow::find_by_statement(Statement::from_sql_and_values(
        connection.get_database_backend(),
        "SELECT id FROM catalog_categories WHERE tenant_id = $1 AND deleted_at IS NULL ORDER BY id ASC",
        vec![tenant_id.into()],
    ))
    .all(connection)
    .await?;
    Ok(rows.into_iter().map(|row| row.id).collect())
}

/// Reads canonical hierarchy in bounded batches. The owner port receives the exact
/// pool/transaction handle used by Product so a schema write never observes a different
/// Taxonomy snapshot from the transaction that is about to commit.
pub(crate) async fn load_taxonomy_category_parent_map<C>(
    connection: &C,
    taxonomy_port: &dyn ProductTaxonomyPort,
    tenant_id: Uuid,
    expected_category_ids: &[Uuid],
    locale: &str,
    fallback_locale: Option<&str>,
) -> CommerceResult<HashMap<Uuid, Option<Uuid>>>
where
    C: ConnectionTrait + AsTaxonomyConnection,
{
    if expected_category_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut requested_ids = expected_category_ids.to_vec();
    requested_ids.sort_unstable();
    if requested_ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(super::taxonomy_projection_error(
            "Product category projection contains duplicate identities",
        ));
    }

    let mut owners = Vec::with_capacity(requested_ids.len());
    for batch in requested_ids.chunks(TAXONOMY_CATEGORY_READ_BATCH_SIZE) {
        let batch_ids = batch.iter().copied().collect::<HashSet<_>>();
        let batch_owners = taxonomy_port
            .load_scoped_categories(
                connection.as_taxonomy_connection(),
                tenant_id,
                Some(batch),
                locale,
                fallback_locale,
            )
            .await?;
        if batch_owners.iter().any(|owner| !batch_ids.contains(&owner.id)) {
            return Err(super::taxonomy_projection_error(
                "Product category Taxonomy read returned an unrequested owner",
            ));
        }
        owners.extend(batch_owners);
    }

    taxonomy_category_parent_map(&requested_ids, owners)
}

/// Validates an exact Product → Taxonomy identity projection before exposing parent links
/// to the schema resolver. Missing, extra, duplicate or malformed owner rows must never be
/// interpreted as root categories.
fn taxonomy_category_parent_map(
    expected_category_ids: &[Uuid],
    owners: Vec<ProductTaxonomyCategorySnapshot>,
) -> CommerceResult<HashMap<Uuid, Option<Uuid>>> {
    let expected_ids = expected_category_ids.iter().copied().collect::<HashSet<_>>();
    if expected_ids.len() != expected_category_ids.len() {
        return Err(super::taxonomy_projection_error(
            "Product category projection contains duplicate identities",
        ));
    }

    let mut parent_map = HashMap::with_capacity(owners.len());
    for owner in owners {
        if !expected_ids.contains(&owner.id) {
            return Err(super::taxonomy_projection_error(
                "Product category Taxonomy read returned an unrequested owner",
            ));
        }
        if owner.canonical_key != format!("product-category-{}", owner.id) {
            return Err(super::taxonomy_projection_error(
                "Product category has incompatible Taxonomy canonical key",
            ));
        }
        if owner.scope_type != "module" || owner.scope_value.as_deref() != Some("product") {
            return Err(super::taxonomy_projection_error(
                "Product category has incompatible Taxonomy scope",
            ));
        }
        if parent_map.insert(owner.id, owner.parent_id).is_some() {
            return Err(super::taxonomy_projection_error(
                "Product category Taxonomy read returned duplicate owners",
            ));
        }
    }

    if parent_map.len() != expected_ids.len()
        || expected_ids.iter().any(|category_id| !parent_map.contains_key(category_id))
    {
        return Err(super::taxonomy_projection_error(
            "Product category is missing its Taxonomy owner projection",
        ));
    }

    for parent_id in parent_map.values().flatten() {
        if !parent_map.contains_key(parent_id) {
            return Err(super::taxonomy_projection_error(
                "Product category is missing its Taxonomy parent owner projection",
            ));
        }
    }

    let mut completed = HashSet::new();
    for start_id in parent_map.keys().copied() {
        let mut visiting = HashSet::new();
        let mut path = Vec::new();
        let mut current = Some(start_id);
        while let Some(category_id) = current {
            if completed.contains(&category_id) {
                break;
            }
            if !visiting.insert(category_id) {
                return Err(super::taxonomy_projection_error(
                    "Product category has a cyclic Taxonomy hierarchy projection",
                ));
            }
            path.push(category_id);
            current = *parent_map.get(&category_id).ok_or_else(|| {
                super::taxonomy_projection_error(
                    "Product category is missing its Taxonomy hierarchy projection",
                )
            })?;
        }
        completed.extend(path);
    }

    Ok(parent_map)
}

fn taxonomy_ancestor_chain(
    category_id: Uuid,
    parent_map: &HashMap<Uuid, Option<Uuid>>,
) -> CommerceResult<Vec<Uuid>> {
    let mut current = Some(category_id);
    let mut seen = HashSet::new();
    let mut chain = Vec::new();

    while let Some(current_id) = current {
        if !seen.insert(current_id) {
            return Err(super::taxonomy_projection_error(
                "Product category has a cyclic Taxonomy hierarchy projection",
            ));
        }
        chain.push(current_id);
        current = *parent_map.get(&current_id).ok_or_else(|| {
            super::taxonomy_projection_error(
                "Product category is missing its Taxonomy hierarchy projection",
            )
        })?;
    }

    chain.reverse();
    Ok(chain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_taxonomy_hierarchy_builds_root_to_leaf_ancestor_chain() {
        let root = Uuid::new_v4();
        let child = Uuid::new_v4();
        let leaf = Uuid::new_v4();
        let parents = HashMap::from([(root, None), (child, Some(root)), (leaf, Some(child))]);

        assert_eq!(
            taxonomy_ancestor_chain(leaf, &parents).expect("Taxonomy ancestry"),
            vec![root, child, leaf]
        );
    }

    #[test]
    fn category_taxonomy_hierarchy_fails_closed_on_missing_or_cyclic_owner_state() {
        let root = Uuid::new_v4();
        let leaf = Uuid::new_v4();
        let missing_parent = HashMap::from([(leaf, Some(root))]);
        assert!(matches!(
            taxonomy_ancestor_chain(leaf, &missing_parent),
            Err(CommerceError::Core(rustok_core::error::Error::Internal(_)))
        ));

        let cycle = HashMap::from([(root, Some(leaf)), (leaf, Some(root))]);
        assert!(matches!(
            taxonomy_ancestor_chain(leaf, &cycle),
            Err(CommerceError::Core(rustok_core::error::Error::Internal(_)))
        ));
    }

    fn taxonomy_category_snapshot(
        id: Uuid,
        parent_id: Option<Uuid>,
    ) -> ProductTaxonomyCategorySnapshot {
        ProductTaxonomyCategorySnapshot {
            id,
            parent_id,
            canonical_key: format!("product-category-{id}"),
            scope_type: "module".to_string(),
            scope_value: Some("product".to_string()),
            slug: format!("category-{id}"),
            name: format!("Category {id}"),
            description: None,
            position: 0,
            available_locales: vec!["en".to_string()],
        }
    }

    #[test]
    fn taxonomy_category_parent_projection_accepts_exact_owner_hierarchy() {
        let root = Uuid::new_v4();
        let child = Uuid::new_v4();
        let parents = taxonomy_category_parent_map(
            &[root, child],
            vec![
                taxonomy_category_snapshot(root, None),
                taxonomy_category_snapshot(child, Some(root)),
            ],
        )
        .expect("exact canonical Taxonomy hierarchy");
        assert_eq!(parents.get(&root), Some(&None));
        assert_eq!(parents.get(&child), Some(&Some(root)));
    }

    #[test]
    fn taxonomy_category_parent_projection_rejects_missing_extra_and_duplicate_owners() {
        let root = Uuid::new_v4();
        let child = Uuid::new_v4();

        let missing = taxonomy_category_parent_map(
            &[root, child],
            vec![taxonomy_category_snapshot(root, None)],
        )
        .expect_err("missing owner must not become a root");
        assert!(matches!(
            missing,
            CommerceError::Core(rustok_core::error::Error::Internal(_))
        ));

        let extra = taxonomy_category_parent_map(
            &[root],
            vec![
                taxonomy_category_snapshot(root, None),
                taxonomy_category_snapshot(child, None),
            ],
        )
        .expect_err("unrequested owner must be rejected");
        assert!(matches!(
            extra,
            CommerceError::Core(rustok_core::error::Error::Internal(_))
        ));

        let duplicate = taxonomy_category_parent_map(
            &[root],
            vec![
                taxonomy_category_snapshot(root, None),
                taxonomy_category_snapshot(root, None),
            ],
        )
        .expect_err("duplicate owner must not overwrite");
        assert!(matches!(
            duplicate,
            CommerceError::Core(rustok_core::error::Error::Internal(_))
        ));
    }

    #[test]
    fn taxonomy_category_parent_projection_rejects_invalid_owner_contract() {
        let id = Uuid::new_v4();

        let mut invalid_key = taxonomy_category_snapshot(id, None);
        invalid_key.canonical_key = "another-owner".to_string();
        assert!(taxonomy_category_parent_map(&[id], vec![invalid_key]).is_err());

        let mut invalid_scope = taxonomy_category_snapshot(id, None);
        invalid_scope.scope_value = Some("forum".to_string());
        assert!(taxonomy_category_parent_map(&[id], vec![invalid_scope]).is_err());

    }

    #[test]
    fn taxonomy_category_parent_projection_rejects_missing_parents_and_cycles() {
        let root = Uuid::new_v4();
        let child = Uuid::new_v4();

        let missing_parent = taxonomy_category_parent_map(
            &[child],
            vec![taxonomy_category_snapshot(child, Some(root))],
        )
        .expect_err("parent outside the Product owner projection must fail closed");
        assert!(matches!(
            missing_parent,
            CommerceError::Core(rustok_core::error::Error::Internal(_))
        ));

        let cycle = taxonomy_category_parent_map(
            &[root, child],
            vec![
                taxonomy_category_snapshot(root, Some(child)),
                taxonomy_category_snapshot(child, Some(root)),
            ],
        )
        .expect_err("cyclic canonical hierarchy must fail closed");
        assert!(matches!(
            cycle,
            CommerceError::Core(rustok_core::error::Error::Internal(_))
        ));
    }

    fn effective_form(attribute_id: Uuid) -> EffectiveProductForm {
        EffectiveProductForm {
            category_id: Uuid::new_v4(),
            attributes: vec![AttributeBinding {
                attribute_id,
                group_code: None,
                scope: String::new(),
                is_required: true,
                is_disabled: false,
                position: 0,
                visibility_overrides: Default::default(),
                validation_overrides: serde_json::json!({ "maxLength": 20 }),
                validation: Value::Object(Default::default()),
                source: EffectiveAttributeSource::Schema,
                variant_axis_policy: "forbidden".to_string(),
                default_variant_axis: false,
            }],
            detached_attribute_ids: Vec::new(),
        }
    }

    #[tokio::test]
    async fn effective_attribute_form_hydrates_definition_scope_and_validation() {
        use sea_orm::Database;

        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory SQLite");
        db.execute_unprepared(
            "CREATE TABLE product_attributes (
                id TEXT PRIMARY KEY,
                tenant_id TEXT NOT NULL,
                scope TEXT NOT NULL,
                value_type TEXT NOT NULL DEFAULT 'text',
                is_localized BOOLEAN NOT NULL DEFAULT 0,
                validation JSON NOT NULL,
                archived_at TEXT NULL
            )",
        )
        .await
        .expect("create product_attributes");
        let tenant_id = Uuid::new_v4();
        let attribute_id = Uuid::new_v4();
        db.execute_raw(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO product_attributes (id, tenant_id, scope, value_type, is_localized, validation, archived_at)
             VALUES (?, ?, 'variant', 'text', 0, '{\"required\":true}', NULL)",
            [attribute_id.into(), tenant_id.into()],
        ))
        .await
        .expect("insert attribute definition");

        let mut form = effective_form(attribute_id);
        load_effective_attribute_validation(&db, tenant_id, &mut form)
            .await
            .expect("hydrate effective attribute");

        assert_eq!(form.attributes[0].scope, "variant");
        assert_eq!(form.attributes[0].validation["required"], true);
        assert_eq!(form.attributes[0].validation["maxLength"], 20);
    }

    #[tokio::test]
    async fn effective_attribute_form_fails_closed_for_missing_or_archived_definitions() {
        use sea_orm::Database;

        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory SQLite");
        db.execute_unprepared(
            "CREATE TABLE product_attributes (
                id TEXT PRIMARY KEY,
                tenant_id TEXT NOT NULL,
                scope TEXT NOT NULL,
                value_type TEXT NOT NULL DEFAULT 'text',
                is_localized BOOLEAN NOT NULL DEFAULT 0,
                validation JSON NOT NULL,
                archived_at TEXT NULL
            )",
        )
        .await
        .expect("create product_attributes");

        let mut form = effective_form(Uuid::new_v4());
        let result = load_effective_attribute_validation(&db, Uuid::new_v4(), &mut form).await;
        assert!(matches!(
            result,
            Err(CommerceError::Core(rustok_core::error::Error::Internal(_)))
        ));
    }
}
