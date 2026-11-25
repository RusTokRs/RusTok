mod translation;
mod variant;
mod variant_translation;

use super::*;
use crate::services::catalog_references::lock_sqlite_product_in_tx;
use crate::services::write_transaction::record_product_operation_result;
use rustok_api::TenantLocale;
use sea_orm::{ColumnTrait, ConnectionTrait};

async fn lock_product_for_update_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
) -> CommerceResult<crate::entities::product::Model> {
    super::policy::lock_schema_for_product_write(txn, tenant_id).await?;
    let query = crate::entities::product::Entity::find_by_id(product_id)
        .filter(crate::entities::product::Column::TenantId.eq(tenant_id));
    let product = match txn.get_database_backend() {
        DatabaseBackend::Postgres | DatabaseBackend::MySql => {
            query.lock_exclusive().one(txn).await?
        }
        DatabaseBackend::Sqlite => {
            if !lock_sqlite_product_in_tx(txn, tenant_id, product_id).await? {
                return Err(CommerceError::ProductNotFound(product_id));
            }
            query.one(txn).await?
        }
        _ => query.one(txn).await?,
    };
    product.ok_or(CommerceError::ProductNotFound(product_id))
}

fn attribute_scope_includes_product(scope: &str) -> bool {
    matches!(scope, "product" | "both")
}

fn attribute_scope_includes_variant(scope: &str) -> bool {
    matches!(scope, "variant" | "both")
}

/// Standalone value commands invalidate document drafts too. Catalog composition
/// does not call this helper: the enclosing document command advances once.
async fn advance_product_value_revision_in_tx(
    txn: &DatabaseTransaction,
    product: &crate::entities::product::Model,
) -> CommerceResult<()> {
    let mut active: crate::entities::product::ActiveModel = product.clone().into();
    active.revision = sea_orm::Set(crate::services::catalog::concurrency::next_revision(
        product.revision,
    )?);
    active.updated_at = sea_orm::Set(chrono::Utc::now().into());
    sea_orm::ActiveModelTrait::update(active, txn).await?;
    Ok(())
}

impl ProductCatalogSchemaService {
    pub async fn load_product_attribute_values(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: &str,
    ) -> CommerceResult<Vec<ProductAttributeValueRecord>> {
        Self::load_product_attribute_values_in(&self.db, tenant_id, product_id, locale).await
    }

    pub(super) async fn load_product_attribute_values_in<C>(
        conn: &C,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: &str,
    ) -> CommerceResult<Vec<ProductAttributeValueRecord>>
    where
        C: ConnectionTrait,
    {
        let locale = canonical_value_locale(locale)?;
        ensure_product(conn, tenant_id, product_id).await?;
        let detached_attribute_ids = match Self::load_effective_form_for_product_in(
            conn,
            tenant_id,
            product_id,
        )
        .await?
        {
            Some(form) => form
                .detached_attribute_ids
                .into_iter()
                .collect::<HashSet<_>>(),
            None => AttributeIdRow::find_by_statement(Statement::from_sql_and_values(
                conn.get_database_backend(),
                "SELECT attribute_id FROM product_attribute_values WHERE tenant_id = $1 AND product_id = $2",
                vec![tenant_id.into(), product_id.into()],
            ))
            .all(conn)
            .await?
            .into_iter()
            .map(|row| row.attribute_id)
            .collect(),
        };

        let rows = ProductAttributeValueRow::find_by_statement(Statement::from_sql_and_values(
            conn.get_database_backend(),
            r#"
            SELECT
                pav.id,
                pav.attribute_id,
                pa.value_type,
                pa.is_localized,
                pav.value_text,
                pav.value_integer,
                pav.value_decimal,
                pav.value_boolean,
                pav.value_date,
                pav.value_datetime,
                pav.value_json,
                pav.detached_at IS NOT NULL AS detached,
                pavt.value_text AS localized_value_text
            FROM product_attribute_values pav
            JOIN product_attributes pa
              ON pa.id = pav.attribute_id AND pa.tenant_id = pav.tenant_id
            LEFT JOIN product_attribute_value_translations pavt
              ON pavt.value_id = pav.id AND pavt.locale = $3
            WHERE pav.tenant_id = $1 AND pav.product_id = $2
            ORDER BY pa.position, pa.code
            "#,
            vec![tenant_id.into(), product_id.into(), locale.clone().into()],
        ))
        .all(conn)
        .await?;

        let option_rows =
            ProductAttributeValueOptionRow::find_by_statement(Statement::from_sql_and_values(
                conn.get_database_backend(),
                r#"
                SELECT pavo.value_id, pavo.option_id
                FROM product_attribute_value_options pavo
                JOIN product_attribute_values pav ON pav.id = pavo.value_id
                WHERE pav.tenant_id = $1 AND pav.product_id = $2
                ORDER BY pavo.option_id
                "#,
                vec![tenant_id.into(), product_id.into()],
            ))
            .all(conn)
            .await?;
        let mut options_by_value: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        for row in option_rows {
            options_by_value
                .entry(row.value_id)
                .or_default()
                .push(row.option_id);
        }

        rows.into_iter()
            .map(|row| {
                let option_ids = options_by_value.remove(&row.id).unwrap_or_default();
                let mut record = row.into_record(option_ids)?;
                record.detached = detached_attribute_ids.contains(&record.attribute_id);
                Ok(record)
            })
            .collect()
    }

    pub(crate) async fn validate_product_publish_requirements_in<C>(
        &self,
        conn: &C,
        tenant_id: Uuid,
        product_id: Uuid,
    ) -> CommerceResult<()>
    where
        C: ConnectionTrait,
    {
        validate_uuid("product_id", product_id)?;
        let Some(form) =
            Self::load_effective_form_for_product_in(conn, tenant_id, product_id).await?
        else {
            return Ok(());
        };

        let mut required_product_attribute_ids = Vec::new();
        let mut required_variant_attribute_ids = Vec::new();
        let mut required_product_locales: HashMap<Uuid, Vec<String>> = HashMap::new();
        let mut required_variant_locales: HashMap<Uuid, Vec<String>> = HashMap::new();

        for binding in form
            .attributes
            .iter()
            .filter(|binding| !binding.is_disabled)
        {
            let rules =
                attribute_validation::parse_product_attribute_validation(&binding.validation)?;
            let is_required =
                binding.is_required || rules.required || !rules.required_locales.is_empty();

            if !matches!(binding.scope.as_str(), "product" | "variant" | "both") {
                return Err(CommerceError::Validation(format!(
                    "attribute {} has an unsupported scope",
                    binding.attribute_id
                )));
            }

            if is_required && attribute_scope_includes_product(binding.scope.as_str()) {
                required_product_attribute_ids.push(binding.attribute_id);
            }
            if is_required && attribute_scope_includes_variant(binding.scope.as_str()) {
                required_variant_attribute_ids.push(binding.attribute_id);
            }
            if !rules.required_locales.is_empty() {
                if attribute_scope_includes_product(binding.scope.as_str()) {
                    required_product_locales
                        .insert(binding.attribute_id, rules.required_locales.clone());
                }
                if attribute_scope_includes_variant(binding.scope.as_str()) {
                    required_variant_locales
                        .insert(binding.attribute_id, rules.required_locales);
                }
            }
        }

        required_product_attribute_ids.sort();
        required_product_attribute_ids.dedup();
        required_variant_attribute_ids.sort();
        required_variant_attribute_ids.dedup();

        if !required_product_attribute_ids.is_empty() {
            let (placeholders, mut values) =
                uuid_filter_values(tenant_id, &required_product_attribute_ids);
            let product_placeholder = format!("${}", values.len() + 1);
            values.push(product_id.into());
            let rows = ProductPublishRequirementRow::find_by_statement(
                Statement::from_sql_and_values(
                    conn.get_database_backend(),
                    format!(
                        r#"
                        SELECT
                            pa.id AS attribute_id,
                            pa.code,
                            pa.value_type,
                            pa.is_localized,
                            pav.value_text,
                            pav.value_integer,
                            pav.value_decimal,
                            pav.value_boolean,
                            pav.value_date,
                            pav.value_datetime,
                            pav.value_json,
                            EXISTS (
                                SELECT 1
                                FROM product_attribute_value_options pavo
                                WHERE pavo.value_id = pav.id
                            ) AS has_option,
                            EXISTS (
                                SELECT 1
                                FROM product_attribute_value_translations pavt
                                WHERE pavt.value_id = pav.id
                                  AND NULLIF(TRIM(pavt.value_text), '') IS NOT NULL
                            ) AS has_localized_text
                        FROM product_attributes pa
                        LEFT JOIN product_attribute_values pav
                          ON pav.tenant_id = pa.tenant_id
                         AND pav.attribute_id = pa.id
                         AND pav.product_id = {product_placeholder}
                        WHERE pa.tenant_id = $1
                          AND pa.archived_at IS NULL
                          AND pa.id IN ({placeholders})
                        "#
                    ),
                    values,
                ),
            )
            .all(conn)
            .await?;

            let present_rows = rows
                .iter()
                .map(|row| row.attribute_id)
                .collect::<HashSet<_>>();
            let mut missing = required_product_attribute_ids
                .iter()
                .filter(|attribute_id| !present_rows.contains(attribute_id))
                .map(|attribute_id| attribute_id.to_string())
                .collect::<Vec<_>>();
            for row in &rows {
                if !row.is_filled()? {
                    missing.push(row.code.clone());
                }
            }
            missing.sort();
            missing.dedup();
            if !missing.is_empty() {
                return Err(CommerceError::Validation(format!(
                    "required product attributes are missing: {}",
                    missing.join(", ")
                )));
            }

            if !required_product_locales.is_empty() {
                let missing_locales = load_missing_required_locales(
                    conn,
                    tenant_id,
                    product_id,
                    &required_product_locales,
                    &present_rows,
                )
                .await?;
                if let Some((attribute_id, locales)) = missing_locales {
                    return Err(attribute_validation::rule_failure(
                        attribute_id,
                        "requiredLocales",
                        format!("missing locales {}", locales.join(", ")),
                    ));
                }
            }
        }

        validate_variant_publish_requirements_in(
            conn,
            tenant_id,
            product_id,
            &required_variant_attribute_ids,
            &required_variant_locales,
        )
        .await
    }

    pub async fn validate_new_product_publish_requirements(
        &self,
        tenant_id: Uuid,
        primary_category_id: Option<Uuid>,
    ) -> CommerceResult<()> {
        Self::validate_new_product_publish_requirements_in(&self.db, tenant_id, primary_category_id)
            .await
    }

    /// Validates publish requirements on the caller's connection, so a Product create can check
    /// them inside the same transaction that inserts the product.
    pub(crate) async fn validate_new_product_publish_requirements_in<C>(
        db: &C,
        tenant_id: Uuid,
        primary_category_id: Option<Uuid>,
    ) -> CommerceResult<()>
    where
        C: ConnectionTrait,
    {
        let Some(category_id) = primary_category_id else {
            return Ok(());
        };
        let form =
            Self::load_effective_form_for_category_in(db, tenant_id, category_id, &[]).await?;
        let mut required_attribute_ids = Vec::new();
        for binding in form
            .attributes
            .iter()
            .filter(|binding| {
                !binding.is_disabled && attribute_scope_includes_product(binding.scope.as_str())
            })
        {
            let rules =
                attribute_validation::parse_product_attribute_validation(&binding.validation)?;
            if binding.is_required || rules.required || !rules.required_locales.is_empty() {
                required_attribute_ids.push(binding.attribute_id);
            }
        }
        if required_attribute_ids.is_empty() {
            return Ok(());
        }
        required_attribute_ids.sort();
        required_attribute_ids.dedup();
        let missing = load_attribute_codes(db, tenant_id, &required_attribute_ids).await?;
        Err(CommerceError::Validation(format!(
            "required product attributes are missing: {}",
            missing.join(", ")
        )))
    }

    /// The caller owns the transaction and final publication/receipt boundary.
    /// Shared by standalone value writes and atomic catalog create/update.
    pub(crate) async fn apply_product_attribute_values_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: &str,
        patches: &[ProductAttributeValuePatch],
        seen_nonlocalized: &mut HashSet<Uuid>,
    ) -> CommerceResult<()> {
        let locale = canonical_value_locale(locale)?;
        let product = lock_product_for_update_in_tx(txn, tenant_id, product_id).await?;
        let Some(primary_category_id) = product.primary_category_id else {
            return Err(CommerceError::Validation(
                "product must have a primary structural category before attribute values can be saved"
                    .into(),
            ));
        };
        let form =
            Self::load_effective_form_for_category_in(txn, tenant_id, primary_category_id, &[])
                .await?;
        let effective_attribute_ids = form
            .attributes
            .iter()
            .filter(|binding| !binding.is_disabled)
            .map(|binding| binding.attribute_id)
            .collect::<HashSet<_>>();
        let validation_by_attribute = form
            .attributes
            .iter()
            .map(|binding| (binding.attribute_id, binding.validation.clone()))
            .collect::<HashMap<_, _>>();

        let patch_attribute_ids = patches
            .iter()
            .map(|patch| patch.attribute_id)
            .collect::<Vec<_>>();
        let definitions = if patch_attribute_ids.is_empty() {
            HashMap::new()
        } else {
            let (placeholders, values) = uuid_filter_values(tenant_id, &patch_attribute_ids);
            ProductAttributeWriteDefinitionRow::find_by_statement(
                Statement::from_sql_and_values(
                    txn.get_database_backend(),
                    format!(
                        "SELECT id, value_type, scope, is_localized, validation FROM product_attributes WHERE tenant_id = $1 AND archived_at IS NULL AND id IN ({placeholders})"
                    ),
                    values,
                ),
            )
            .all(txn)
            .await?
            .into_iter()
            .map(|row| (row.id, row))
            .collect::<HashMap<_, _>>()
        };

        let selected_option_ids = patches
            .iter()
            .flat_map(|patch| match &patch.value {
                ProductAttributeValuePatchValue::Select(option_id) => vec![*option_id],
                ProductAttributeValuePatchValue::Multiselect(option_ids) => option_ids.clone(),
                _ => Vec::new(),
            })
            .collect::<Vec<_>>();
        let options = if selected_option_ids.is_empty() {
            HashMap::new()
        } else {
            let (placeholders, values) = uuid_filter_values(tenant_id, &selected_option_ids);
            ProductAttributeOptionWriteRow::find_by_statement(Statement::from_sql_and_values(
                txn.get_database_backend(),
                format!(
                    "SELECT id, attribute_id FROM product_attribute_options WHERE tenant_id = $1 AND archived_at IS NULL AND id IN ({placeholders})"
                ),
                values,
            ))
            .all(txn)
            .await?
            .into_iter()
            .map(|row| (row.id, row.attribute_id))
            .collect::<HashMap<_, _>>()
        };

        let no_rules = Value::Object(Default::default());
        let mut seen = HashSet::new();
        for patch in patches {
            validate_uuid("attribute_id", patch.attribute_id)?;
            if !seen.insert(patch.attribute_id) {
                return Err(CommerceError::Validation(format!(
                    "attribute {} occurs more than once in one patch request",
                    patch.attribute_id
                )));
            }
            if !effective_attribute_ids.contains(&patch.attribute_id) {
                return Err(CommerceError::Validation(format!(
                    "attribute {} is outside the product effective schema",
                    patch.attribute_id
                )));
            }
            let definition = definitions.get(&patch.attribute_id).ok_or_else(|| {
                CommerceError::Validation(format!(
                    "attribute {} is not available",
                    patch.attribute_id
                ))
            })?;
            if !definition.is_localized && !seen_nonlocalized.insert(patch.attribute_id) {
                return Err(CommerceError::Validation(
                    "nonlocalized attribute occurs in more than one locale batch".into(),
                ));
            }
            let validation = validation_by_attribute
                .get(&patch.attribute_id)
                .unwrap_or(&no_rules);
            validate_product_value_patch(definition, patch, &options, validation)?;
        }

        for patch in patches {
            let definition = definitions.get(&patch.attribute_id).ok_or_else(|| {
                CommerceError::Validation("validated attribute definition must exist".into())
            })?;
            write_product_value_patch(
                txn,
                tenant_id,
                product_id,
                locale.as_str(),
                definition,
                patch,
            )
            .await?;
        }
        Ok(())
    }

    pub(crate) async fn apply_product_attribute_batches_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        product_id: Uuid,
        batches: &[crate::dto::ProductAttributeValueBatch],
    ) -> CommerceResult<bool> {
        let mut seen_locales = HashSet::new();
        let mut seen_nonlocalized = HashSet::new();
        let mut changed = false;
        for batch in batches {
            let locale = canonical_value_locale(&batch.locale)?;
            if !seen_locales.insert(locale.clone()) {
                return Err(CommerceError::Validation(
                    "attribute value locale occurs more than once in one catalog command".into(),
                ));
            }
            self.apply_product_attribute_values_in_tx(
                txn,
                tenant_id,
                product_id,
                &locale,
                &batch.patches,
                &mut seen_nonlocalized,
            )
            .await?;
            changed |= !batch.patches.is_empty();
        }
        Ok(changed)
    }

    pub async fn save_product_attribute_values(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        product_id: Uuid,
        locale: &str,
        patches: Vec<ProductAttributeValuePatch>,
    ) -> CommerceResult<Vec<ProductAttributeValueRecord>> {
        let locale = canonical_value_locale(locale)?;
        validate_uuid("product_id", product_id)?;

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
        let product = lock_product_for_update_in_tx(&txn, tenant_id, product_id).await?;
        self.apply_product_attribute_values_in_tx(
            &txn,
            tenant_id,
            product_id,
            &locale,
            &patches,
            &mut HashSet::new(),
        )
        .await?;
        if product.status == crate::entities::product::ProductStatus::Active {
            self.validate_product_publish_requirements_in(&txn, tenant_id, product_id)
                .await?;
        }
        if !patches.is_empty() {
            advance_product_value_revision_in_tx(&txn, &product).await?;
            txn.publish_in_tx(
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductAttributeValuesChanged { product_id },
            )
            .await?;
        }
        let result =
            Self::load_product_attribute_values_in(&txn, tenant_id, product_id, locale.as_str())
                .await?;
        record_product_operation_result(&result);
        txn.commit().await?;
        Ok(result)
    }

    pub async fn clear_detached_product_attribute_values(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        product_id: Uuid,
        locale: &str,
        attribute_ids: Vec<Uuid>,
    ) -> CommerceResult<Vec<ProductAttributeValueRecord>> {
        let locale = canonical_value_locale(locale)?;
        validate_uuid("product_id", product_id)?;
        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
        let product = lock_product_for_update_in_tx(&txn, tenant_id, product_id).await?;
        let detached_attribute_ids = match Self::load_effective_form_for_product_in(
            &txn, tenant_id, product_id,
        )
        .await?
        {
            Some(form) => form
                .detached_attribute_ids
                .into_iter()
                .collect::<HashSet<_>>(),
            None => AttributeIdRow::find_by_statement(Statement::from_sql_and_values(
                txn.get_database_backend(),
                "SELECT attribute_id FROM product_attribute_values WHERE tenant_id = $1 AND product_id = $2",
                vec![tenant_id.into(), product_id.into()],
            ))
            .all(&txn)
            .await?
            .into_iter()
            .map(|row| row.attribute_id)
            .collect(),
        };
        let target_attribute_ids = if attribute_ids.is_empty() {
            detached_attribute_ids.iter().copied().collect::<Vec<_>>()
        } else {
            let mut seen = HashSet::new();
            for attribute_id in &attribute_ids {
                validate_uuid("attribute_id", *attribute_id)?;
                if !seen.insert(*attribute_id) {
                    return Err(CommerceError::Validation(format!(
                        "attribute {} occurs more than once",
                        attribute_id
                    )));
                }
                if !detached_attribute_ids.contains(attribute_id) {
                    return Err(CommerceError::Validation(format!(
                        "attribute {} is not detached for this product",
                        attribute_id
                    )));
                }
            }
            attribute_ids
        };

        if !target_attribute_ids.is_empty() {
            let (placeholders, mut values) = uuid_filter_values(tenant_id, &target_attribute_ids);
            let product_placeholder = format!("${}", values.len() + 1);
            values.push(product_id.into());
            txn.execute_raw(Statement::from_sql_and_values(
                txn.get_database_backend(),
                format!(
                    r#"
                    DELETE FROM product_attribute_values
                    WHERE tenant_id = $1
                      AND attribute_id IN ({placeholders})
                      AND product_id = {product_placeholder}
                    "#
                ),
                values,
            ))
            .await?;
            advance_product_value_revision_in_tx(&txn, &product).await?;
            txn.publish_in_tx(
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductAttributeValuesChanged { product_id },
            )
            .await?;
        }
        let result =
            Self::load_product_attribute_values_in(&txn, tenant_id, product_id, locale.as_str())
                .await?;
        record_product_operation_result(&result);
        txn.commit().await?;
        Ok(result)
    }
}

fn canonical_value_locale(locale: &str) -> CommerceResult<String> {
    TenantLocale::new(locale)
        .map(TenantLocale::into_inner)
        .map_err(|error| CommerceError::Validation(error.to_string()))
}

#[derive(FromQueryResult)]
struct MissingVariantAttributeRow {
    variant_label: String,
    attribute_code: String,
}

#[derive(FromQueryResult)]
struct MissingVariantRequiredLocaleRow {
    variant_label: String,
    attribute_code: String,
    locale: String,
}

async fn validate_variant_publish_requirements_in<C>(
    conn: &C,
    tenant_id: Uuid,
    product_id: Uuid,
    required_attribute_ids: &[Uuid],
    required_locales_by_attribute: &HashMap<Uuid, Vec<String>>,
) -> CommerceResult<()>
where
    C: ConnectionTrait,
{
    if !required_attribute_ids.is_empty() {
        let (placeholders, mut values) = uuid_filter_values(tenant_id, required_attribute_ids);
        let product_placeholder = format!("${}", values.len() + 1);
        values.push(product_id.into());
        let missing = MissingVariantAttributeRow::find_by_statement(
            Statement::from_sql_and_values(
                conn.get_database_backend(),
                format!(
                    r#"
                    SELECT
                        COALESCE(NULLIF(TRIM(pv.sku), ''), CAST(pv.id AS TEXT)) AS variant_label,
                        pa.code AS attribute_code
                    FROM product_variants pv
                    JOIN product_attributes pa
                      ON pa.tenant_id = pv.tenant_id
                     AND pa.archived_at IS NULL
                     AND pa.id IN ({placeholders})
                    LEFT JOIN product_variant_attribute_values pvav
                      ON pvav.tenant_id = pv.tenant_id
                     AND pvav.variant_id = pv.id
                     AND pvav.attribute_id = pa.id
                    WHERE pv.tenant_id = $1
                      AND pv.product_id = {product_placeholder}
                      AND NOT (
                          CASE
                              WHEN pa.is_localized THEN EXISTS (
                                  SELECT 1
                                  FROM product_variant_attribute_value_translations pvavt
                                  WHERE pvavt.value_id = pvav.id
                                    AND NULLIF(TRIM(pvavt.value_text), '') IS NOT NULL
                              )
                              WHEN pa.value_type IN ('text', 'textarea', 'richtext')
                                  THEN NULLIF(TRIM(pvav.value_text), '') IS NOT NULL
                              WHEN pa.value_type = 'integer'
                                  THEN pvav.value_integer IS NOT NULL
                              WHEN pa.value_type = 'decimal'
                                  THEN pvav.value_decimal IS NOT NULL
                              WHEN pa.value_type = 'boolean'
                                  THEN pvav.value_boolean IS NOT NULL
                              WHEN pa.value_type = 'date'
                                  THEN pvav.value_date IS NOT NULL
                              WHEN pa.value_type = 'datetime'
                                  THEN pvav.value_datetime IS NOT NULL
                              WHEN pa.value_type IN ('select', 'multiselect') THEN EXISTS (
                                  SELECT 1
                                  FROM product_variant_attribute_value_options pvaavo
                                  WHERE pvaavo.value_id = pvav.id
                              )
                              WHEN pa.value_type = 'json'
                                  THEN pvav.value_json IS NOT NULL
                              ELSE FALSE
                          END
                      )
                    ORDER BY pv.position, pv.id, pa.position, pa.code
                    LIMIT 1
                    "#
                ),
                values,
            ),
        )
        .one(conn)
        .await?;
        if let Some(row) = missing {
            return Err(CommerceError::Validation(format!(
                "required variant attribute `{}` is missing for variant `{}`",
                row.attribute_code, row.variant_label
            )));
        }
    }

    let mut required_pairs = required_locales_by_attribute
        .iter()
        .flat_map(|(attribute_id, locales)| {
            locales
                .iter()
                .map(move |locale| (*attribute_id, locale.clone()))
        })
        .collect::<Vec<_>>();
    required_pairs.sort_by(|left, right| {
        left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1))
    });
    required_pairs.dedup();

    if !required_pairs.is_empty() {
        let mut values: Vec<sea_orm::Value> = vec![tenant_id.into(), product_id.into()];
        let mut required_rows = Vec::with_capacity(required_pairs.len());
        for (attribute_id, locale) in required_pairs {
            let attribute_placeholder = format!("${}", values.len() + 1);
            values.push(attribute_id.to_string().into());
            let locale_placeholder = format!("${}", values.len() + 1);
            values.push(locale.into());
            required_rows.push(format!("({attribute_placeholder}, {locale_placeholder})"));
        }

        let missing = MissingVariantRequiredLocaleRow::find_by_statement(
            Statement::from_sql_and_values(
                conn.get_database_backend(),
                format!(
                    r#"
                    WITH required_locales(attribute_id, locale) AS (
                        VALUES {}
                    )
                    SELECT
                        COALESCE(NULLIF(TRIM(pv.sku), ''), CAST(pv.id AS TEXT)) AS variant_label,
                        pa.code AS attribute_code,
                        required_locales.locale
                    FROM product_variants pv
                    JOIN required_locales ON TRUE
                    JOIN product_attributes pa
                      ON pa.tenant_id = pv.tenant_id
                     AND CAST(pa.id AS TEXT) = required_locales.attribute_id
                     AND pa.archived_at IS NULL
                    LEFT JOIN product_variant_attribute_values pvav
                      ON pvav.tenant_id = pv.tenant_id
                     AND pvav.variant_id = pv.id
                     AND pvav.attribute_id = pa.id
                    WHERE pv.tenant_id = $1
                      AND pv.product_id = $2
                      AND NOT EXISTS (
                          SELECT 1
                          FROM product_variant_attribute_value_translations pvavt
                          WHERE pvavt.value_id = pvav.id
                            AND pvavt.locale = required_locales.locale
                            AND NULLIF(TRIM(pvavt.value_text), '') IS NOT NULL
                      )
                    ORDER BY pv.position, pv.id, pa.position, pa.code, required_locales.locale
                    LIMIT 1
                    "#,
                    required_rows.join(", ")
                ),
                values,
            ),
        )
        .one(conn)
        .await?;
        if let Some(row) = missing {
            return Err(CommerceError::Validation(format!(
                "required variant attribute `{}` is missing locale `{}` for variant `{}`",
                row.attribute_code, row.locale, row.variant_label
            )));
        }
    }

    Ok(())
}

#[derive(FromQueryResult)]
struct ProductAttributeValueLocaleRow {
    attribute_id: Uuid,
    locale: String,
}

/// Returns the first declared-locale violation of a required-locale rule, ordered by attribute id.
///
/// The check runs only when a rule declares `requiredLocales`, so tenants without the rule keep
/// the single existing publish-requirement query.
async fn load_missing_required_locales<C>(
    conn: &C,
    tenant_id: Uuid,
    product_id: Uuid,
    required_locales_by_attribute: &HashMap<Uuid, Vec<String>>,
    present_rows: &HashSet<Uuid>,
) -> CommerceResult<Option<(Uuid, Vec<String>)>>
where
    C: ConnectionTrait,
{
    let mut attribute_ids = required_locales_by_attribute
        .keys()
        .copied()
        .filter(|attribute_id| present_rows.contains(attribute_id))
        .collect::<Vec<_>>();
    if attribute_ids.is_empty() {
        return Ok(None);
    }
    attribute_ids.sort();

    let (placeholders, mut values) = uuid_filter_values(tenant_id, &attribute_ids);
    let product_placeholder = format!("${}", values.len() + 1);
    values.push(product_id.into());
    let filled = ProductAttributeValueLocaleRow::find_by_statement(Statement::from_sql_and_values(
        conn.get_database_backend(),
        format!(
            r#"
                SELECT pav.attribute_id, pavt.locale
                FROM product_attribute_values pav
                JOIN product_attribute_value_translations pavt
                  ON pavt.value_id = pav.id
                WHERE pav.tenant_id = $1
                  AND pav.attribute_id IN ({placeholders})
                  AND pav.product_id = {product_placeholder}
                  AND NULLIF(TRIM(pavt.value_text), '') IS NOT NULL
                "#
        ),
        values,
    ))
    .all(conn)
    .await?;

    let mut filled_locales: HashMap<Uuid, HashSet<String>> = HashMap::new();
    for row in filled {
        filled_locales
            .entry(row.attribute_id)
            .or_default()
            .insert(row.locale);
    }

    for attribute_id in attribute_ids {
        let Some(required) = required_locales_by_attribute.get(&attribute_id) else {
            continue;
        };
        let filled = filled_locales.get(&attribute_id);
        let missing = required
            .iter()
            .filter(|locale| !filled.is_some_and(|locales| locales.contains(locale.as_str())))
            .cloned()
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Ok(Some((attribute_id, missing)));
        }
    }
    Ok(None)
}


#[cfg(test)]
mod variant_publish_requirement_tests {
    use super::*;
    use sea_orm::Database;

    async fn publish_requirement_database() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(
            "CREATE TABLE product_variants (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, product_id TEXT NOT NULL, sku TEXT, position INTEGER NOT NULL)",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "CREATE TABLE product_attributes (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, code TEXT NOT NULL, value_type TEXT NOT NULL, is_localized BOOLEAN NOT NULL, archived_at TEXT NULL, position INTEGER NOT NULL)",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "CREATE TABLE product_variant_attribute_values (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, variant_id TEXT NOT NULL, attribute_id TEXT NOT NULL, value_text TEXT NULL, value_integer INTEGER NULL, value_decimal NUMERIC NULL, value_boolean BOOLEAN NULL, value_date TEXT NULL, value_datetime TEXT NULL, value_json TEXT NULL)",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "CREATE TABLE product_variant_attribute_value_options (value_id TEXT NOT NULL, option_id TEXT NOT NULL)",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "CREATE TABLE product_variant_attribute_value_translations (value_id TEXT NOT NULL, locale TEXT NOT NULL, value_text TEXT NOT NULL)",
        )
        .await
        .unwrap();
        db
    }

    async fn insert_two_variants(
        db: &sea_orm::DatabaseConnection,
        tenant_id: Uuid,
        product_id: Uuid,
        first_variant_id: Uuid,
        second_variant_id: Uuid,
    ) {
        db.execute_unprepared(&format!(
            "INSERT INTO product_variants (id, tenant_id, product_id, sku, position) VALUES ('{first_variant_id}', '{tenant_id}', '{product_id}', 'SKU-1', 0), ('{second_variant_id}', '{tenant_id}', '{product_id}', 'SKU-2', 1)"
        ))
        .await
        .unwrap();
    }

    #[test]
    fn publish_requirements_are_scoped_to_the_correct_owner() {
        assert!(attribute_scope_includes_product("product"));
        assert!(!attribute_scope_includes_product("variant"));
        assert!(attribute_scope_includes_product("both"));
        assert!(!attribute_scope_includes_variant("product"));
        assert!(attribute_scope_includes_variant("variant"));
        assert!(attribute_scope_includes_variant("both"));
    }

    #[tokio::test]
    async fn required_variant_value_must_exist_for_every_variant() {
        let db = publish_requirement_database().await;
        let tenant_id = Uuid::new_v4();
        let product_id = Uuid::new_v4();
        let first_variant_id = Uuid::new_v4();
        let second_variant_id = Uuid::new_v4();
        let attribute_id = Uuid::new_v4();
        insert_two_variants(&db, tenant_id, product_id, first_variant_id, second_variant_id)
            .await;
        db.execute_unprepared(&format!(
            "INSERT INTO product_attributes (id, tenant_id, code, value_type, is_localized, archived_at, position) VALUES ('{attribute_id}', '{tenant_id}', 'material', 'text', 0, NULL, 0)"
        ))
        .await
        .unwrap();
        db.execute_unprepared(&format!(
            "INSERT INTO product_variant_attribute_values (id, tenant_id, variant_id, attribute_id, value_text) VALUES ('{}', '{tenant_id}', '{first_variant_id}', '{attribute_id}', 'cotton')",
            Uuid::new_v4()
        ))
        .await
        .unwrap();

        let error = validate_variant_publish_requirements_in(
            &db,
            tenant_id,
            product_id,
            &[attribute_id],
            &HashMap::new(),
        )
        .await
        .expect_err("a required Variant value cannot be satisfied by another SKU");
        assert!(error.to_string().contains("SKU-2"));
        assert!(error.to_string().contains("material"));
    }

    #[tokio::test]
    async fn required_variant_locales_are_checked_for_every_variant() {
        let db = publish_requirement_database().await;
        let tenant_id = Uuid::new_v4();
        let product_id = Uuid::new_v4();
        let first_variant_id = Uuid::new_v4();
        let second_variant_id = Uuid::new_v4();
        let attribute_id = Uuid::new_v4();
        let first_value_id = Uuid::new_v4();
        let second_value_id = Uuid::new_v4();
        insert_two_variants(&db, tenant_id, product_id, first_variant_id, second_variant_id)
            .await;
        db.execute_unprepared(&format!(
            "INSERT INTO product_attributes (id, tenant_id, code, value_type, is_localized, archived_at, position) VALUES ('{attribute_id}', '{tenant_id}', 'care', 'text', 1, NULL, 0)"
        ))
        .await
        .unwrap();
        db.execute_unprepared(&format!(
            "INSERT INTO product_variant_attribute_values (id, tenant_id, variant_id, attribute_id) VALUES ('{first_value_id}', '{tenant_id}', '{first_variant_id}', '{attribute_id}'), ('{second_value_id}', '{tenant_id}', '{second_variant_id}', '{attribute_id}')"
        ))
        .await
        .unwrap();
        db.execute_unprepared(&format!(
            "INSERT INTO product_variant_attribute_value_translations (value_id, locale, value_text) VALUES ('{first_value_id}', 'en', 'care en'), ('{first_value_id}', 'fr', 'care fr'), ('{second_value_id}', 'en', 'care en')"
        ))
        .await
        .unwrap();

        let required_locales = HashMap::from([(
            attribute_id,
            vec!["en".to_owned(), "fr".to_owned()],
        )]);
        let error = validate_variant_publish_requirements_in(
            &db,
            tenant_id,
            product_id,
            &[attribute_id],
            &required_locales,
        )
        .await
        .expect_err("each Variant must contain every required locale");
        assert!(error.to_string().contains("SKU-2"));
        assert!(error.to_string().contains("fr"));
    }
}

#[cfg(test)]
mod localized_clear_tests {
    use super::*;
    use sea_orm::{Database, DatabaseBackend, Statement};

    async fn localized_value_database(
        locales: &[&str],
    ) -> (DatabaseConnection, Uuid, Uuid, Uuid, Uuid) {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(
            "CREATE TABLE product_attribute_values (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, product_id TEXT NOT NULL, attribute_id TEXT NOT NULL)",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "CREATE TABLE product_attribute_value_translations (tenant_id TEXT NOT NULL, value_id TEXT NOT NULL, locale TEXT NOT NULL, value_text TEXT NOT NULL, PRIMARY KEY (value_id, locale))",
        )
        .await
        .unwrap();

        let tenant_id = Uuid::from_u128(1);
        let product_id = Uuid::from_u128(2);
        let attribute_id = Uuid::from_u128(3);
        let value_id = Uuid::from_u128(4);
        db.execute_unprepared(&format!(
            "INSERT INTO product_attribute_values (id, tenant_id, product_id, attribute_id) VALUES ('{value_id}', '{tenant_id}', '{product_id}', '{attribute_id}')"
        ))
        .await
        .unwrap();
        for locale in locales {
            db.execute_unprepared(&format!(
                "INSERT INTO product_attribute_value_translations (tenant_id, value_id, locale, value_text) VALUES ('{tenant_id}', '{value_id}', '{locale}', 'value-{locale}')"
            ))
            .await
            .unwrap();
        }

        (db, tenant_id, product_id, attribute_id, value_id)
    }

    async fn count_rows(db: &DatabaseConnection, sql: &str) -> i64 {
        db.query_one(&Statement::from_string(
            DatabaseBackend::Sqlite,
            sql.to_owned(),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "total")
        .unwrap()
    }

    fn localized_definition(attribute_id: Uuid) -> ProductAttributeWriteDefinitionRow {
        ProductAttributeWriteDefinitionRow {
            id: attribute_id,
            value_type: "text".to_owned(),
            scope: "product".to_owned(),
            is_localized: true,
        }
    }

    #[tokio::test]
    async fn clearing_one_locale_preserves_other_localized_values() {
        let (db, tenant_id, product_id, attribute_id, value_id) =
            localized_value_database(&["en-US", "ru-RU"]).await;
        let definition = localized_definition(attribute_id);
        let patch = ProductAttributeValuePatch {
            attribute_id,
            value: ProductAttributeValuePatchValue::Clear,
        };

        write_product_value_patch(
            &db,
            tenant_id,
            product_id,
            "ru-RU",
            &definition,
            &patch,
        )
        .await
        .unwrap();

        assert_eq!(
            count_rows(
                &db,
                &format!(
                    "SELECT COUNT(*) AS total FROM product_attribute_value_translations WHERE value_id = '{value_id}' AND locale = 'en-US'"
                ),
            )
            .await,
            1
        );
        assert_eq!(
            count_rows(
                &db,
                &format!(
                    "SELECT COUNT(*) AS total FROM product_attribute_value_translations WHERE value_id = '{value_id}' AND locale = 'ru-RU'"
                ),
            )
            .await,
            0
        );
        assert_eq!(
            count_rows(
                &db,
                &format!(
                    "SELECT COUNT(*) AS total FROM product_attribute_values WHERE id = '{value_id}'"
                ),
            )
            .await,
            1
        );
    }

    #[tokio::test]
    async fn clearing_last_locale_removes_empty_localized_value_row() {
        let (db, tenant_id, product_id, attribute_id, value_id) =
            localized_value_database(&["ru-RU"]).await;
        let definition = localized_definition(attribute_id);
        let patch = ProductAttributeValuePatch {
            attribute_id,
            value: ProductAttributeValuePatchValue::Clear,
        };

        write_product_value_patch(
            &db,
            tenant_id,
            product_id,
            "ru-RU",
            &definition,
            &patch,
        )
        .await
        .unwrap();

        assert_eq!(
            count_rows(
                &db,
                &format!(
                    "SELECT COUNT(*) AS total FROM product_attribute_value_translations WHERE value_id = '{value_id}'"
                ),
            )
            .await,
            0
        );
        assert_eq!(
            count_rows(
                &db,
                &format!(
                    "SELECT COUNT(*) AS total FROM product_attribute_values WHERE id = '{value_id}'"
                ),
            )
            .await,
            0
        );
    }
}
