use super::*;

impl ProductCatalogSchemaService {
    pub async fn load_effective_form_for_product(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
    ) -> CommerceResult<Option<EffectiveProductForm>> {
        Self::load_effective_form_for_product_in(&self.db, tenant_id, product_id).await
    }

    pub(super) async fn load_effective_form_for_product_in<C>(
        db: &C,
        tenant_id: Uuid,
        product_id: Uuid,
    ) -> CommerceResult<Option<EffectiveProductForm>>
    where
        C: ConnectionTrait,
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
        Self::load_effective_form_for_category_in(
            &self.db,
            tenant_id,
            category_id,
            existing_value_attribute_ids,
        )
        .await
    }

    async fn load_effective_form_for_category_in<C>(
        db: &C,
        tenant_id: Uuid,
        category_id: Uuid,
        existing_value_attribute_ids: &[Uuid],
    ) -> CommerceResult<EffectiveProductForm>
    where
        C: ConnectionTrait,
    {
        let categories = Self::load_category_schema_map(db, tenant_id).await?;
        let schemas = Self::load_attribute_schema_map(db, tenant_id).await?;
        resolve_effective_product_form(
            category_id,
            &categories,
            &schemas,
            existing_value_attribute_ids,
        )
        .map_err(map_schema_resolution_error)
    }

    pub async fn load_effective_form_group_labels(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        locale: &str,
    ) -> CommerceResult<HashMap<String, String>> {
        validate_locale(locale)?;
        let mut labels = HashMap::new();
        let category_ids = CategoryAncestorRow::find_by_statement(Statement::from_sql_and_values(
            self.db.get_database_backend(),
            r#"
            SELECT ancestor_id AS category_id
            FROM catalog_category_closure
            WHERE tenant_id = $1 AND descendant_id = $2
            ORDER BY depth DESC
            "#,
            vec![tenant_id.into(), category_id.into()],
        ))
        .all(&self.db)
        .await?
        .into_iter()
        .map(|row| row.category_id)
        .collect::<Vec<_>>();

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

    async fn load_category_schema_map<C>(
        db: &C,
        tenant_id: Uuid,
    ) -> CommerceResult<HashMap<Uuid, CatalogCategorySchema>>
    where
        C: ConnectionTrait,
    {
        let category_rows = CategorySchemaRow::find_by_statement(Statement::from_sql_and_values(
            db.get_database_backend(),
            r#"
            SELECT
                c.id AS category_id,
                c.parent_id AS parent_category_id,
                c.kind,
                COALESCE(a.mode, 'inherit') AS mode,
                a.schema_id,
                COALESCE(a.snapshot, '{}'::jsonb) AS snapshot
            FROM catalog_categories c
            LEFT JOIN category_attribute_schema_assignments a
                ON a.category_id = c.id AND a.tenant_id = c.tenant_id
            WHERE c.tenant_id = $1 AND c.deleted_at IS NULL
            "#,
            vec![tenant_id.into()],
        ))
        .all(db)
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
                ca.validation_overrides
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
                });
        }

        let mut categories = HashMap::new();
        for row in category_rows {
            let clone_snapshot = serde_json::from_value(row.snapshot.clone()).unwrap_or_default();
            categories.insert(
                row.category_id,
                CatalogCategorySchema {
                    category_id: row.category_id,
                    parent_category_id: row.parent_category_id,
                    kind: CatalogCategoryKind::from_storage(&row.kind)
                        .map_err(map_schema_resolution_error)?,
                    mode: CategorySchemaMode::from_storage(&row.mode)
                        .map_err(map_schema_resolution_error)?,
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

    async fn load_attribute_schema_map<C>(
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
                psa.validation_overrides
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
                    is_required: row.is_required,
                    is_disabled: row.is_disabled,
                    position: row.position,
                    visibility_overrides: parse_visibility_overrides(row.visibility_overrides)?,
                    validation_overrides: row.validation_overrides,
                    source: EffectiveAttributeSource::Schema,
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
