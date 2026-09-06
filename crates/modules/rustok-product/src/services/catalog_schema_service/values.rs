use super::*;
use crate::services::write_transaction::record_product_operation_result;

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
        validate_locale(locale)?;
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
            vec![tenant_id.into(), product_id.into(), locale.trim().into()],
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

    pub async fn validate_product_publish_requirements(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
    ) -> CommerceResult<()> {
        validate_uuid("product_id", product_id)?;
        let Some(form) = self
            .load_effective_form_for_product(tenant_id, product_id)
            .await?
        else {
            return Ok(());
        };
        let required_attribute_ids = form
            .attributes
            .iter()
            .filter(|binding| binding.is_required && !binding.is_disabled)
            .map(|binding| binding.attribute_id)
            .collect::<Vec<_>>();
        if required_attribute_ids.is_empty() {
            return Ok(());
        }

        let (placeholders, mut values) = uuid_filter_values(tenant_id, &required_attribute_ids);
        let product_placeholder = format!("${}", values.len() + 1);
        values.push(product_id.into());
        let rows = ProductPublishRequirementRow::find_by_statement(Statement::from_sql_and_values(
            self.db.get_database_backend(),
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
                          AND NULLIF(BTRIM(pavt.value_text), '') IS NOT NULL
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
        ))
        .all(&self.db)
        .await?;

        let present_rows = rows
            .iter()
            .map(|row| row.attribute_id)
            .collect::<HashSet<_>>();
        let mut missing = required_attribute_ids
            .iter()
            .filter(|attribute_id| !present_rows.contains(attribute_id))
            .map(|attribute_id| attribute_id.to_string())
            .collect::<Vec<_>>();
        for row in rows {
            if !row.is_filled()? {
                missing.push(row.code);
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
        Ok(())
    }

    pub async fn validate_new_product_publish_requirements(
        &self,
        tenant_id: Uuid,
        primary_category_id: Option<Uuid>,
    ) -> CommerceResult<()> {
        let Some(category_id) = primary_category_id else {
            return Ok(());
        };
        let form = self
            .load_effective_form_for_category(tenant_id, category_id, &[])
            .await?;
        let required_attribute_ids = form
            .attributes
            .iter()
            .filter(|binding| binding.is_required && !binding.is_disabled)
            .map(|binding| binding.attribute_id)
            .collect::<Vec<_>>();
        if required_attribute_ids.is_empty() {
            return Ok(());
        }
        let missing = load_attribute_codes(&self.db, tenant_id, &required_attribute_ids).await?;
        Err(CommerceError::Validation(format!(
            "required product attributes are missing: {}",
            missing.join(", ")
        )))
    }

    pub async fn save_product_attribute_values(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        product_id: Uuid,
        locale: &str,
        patches: Vec<ProductAttributeValuePatch>,
    ) -> CommerceResult<Vec<ProductAttributeValueRecord>> {
        validate_locale(locale)?;
        validate_uuid("product_id", product_id)?;

        let product = load_product_primary_category(&self.db, tenant_id, product_id).await?;
        let Some(primary_category_id) = product.primary_category_id else {
            return Err(CommerceError::Validation(
                "product must have a primary structural category before attribute values can be saved"
                    .into(),
            ));
        };
        let form = self
            .load_effective_form_for_category(tenant_id, primary_category_id, &[])
            .await?;
        let effective_attribute_ids = form
            .attributes
            .iter()
            .filter(|binding| !binding.is_disabled)
            .map(|binding| binding.attribute_id)
            .collect::<HashSet<_>>();

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
                    self.db.get_database_backend(),
                    format!(
                        "SELECT id, value_type, scope, is_localized FROM product_attributes WHERE tenant_id = $1 AND archived_at IS NULL AND id IN ({placeholders})"
                    ),
                    values,
                ),
            )
            .all(&self.db)
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
                self.db.get_database_backend(),
                format!(
                    "SELECT id, attribute_id FROM product_attribute_options WHERE tenant_id = $1 AND archived_at IS NULL AND id IN ({placeholders})"
                ),
                values,
            ))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|row| (row.id, row.attribute_id))
            .collect::<HashMap<_, _>>()
        };

        let mut seen = HashSet::new();
        for patch in &patches {
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
            validate_product_value_patch(definition, patch, &options)?;
        }

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
        ensure_product(&txn, tenant_id, product_id).await?;
        for patch in &patches {
            let definition = definitions
                .get(&patch.attribute_id)
                .expect("validated attribute definition must exist");
            write_product_value_patch(
                &txn,
                tenant_id,
                product_id,
                locale.trim(),
                definition,
                patch,
            )
            .await?;
        }
        if !patches.is_empty() {
            txn.publish(
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductAttributeValuesChanged { product_id },
            )
            .await?;
        }
        let result =
            Self::load_product_attribute_values_in(&txn, tenant_id, product_id, locale).await?;
        record_product_operation_result(&result)?;
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
        validate_locale(locale)?;
        validate_uuid("product_id", product_id)?;
        ensure_product(&self.db, tenant_id, product_id).await?;
        let detached_attribute_ids = match self
            .load_effective_form_for_product(tenant_id, product_id)
            .await?
        {
            Some(form) => form
                .detached_attribute_ids
                .into_iter()
                .collect::<HashSet<_>>(),
            None => AttributeIdRow::find_by_statement(Statement::from_sql_and_values(
                self.db.get_database_backend(),
                "SELECT attribute_id FROM product_attribute_values WHERE tenant_id = $1 AND product_id = $2",
                vec![tenant_id.into(), product_id.into()],
            ))
            .all(&self.db)
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

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
        ensure_product(&txn, tenant_id, product_id).await?;
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
            txn.publish(
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductAttributeValuesChanged { product_id },
            )
            .await?;
        }
        let result =
            Self::load_product_attribute_values_in(&txn, tenant_id, product_id, locale).await?;
        record_product_operation_result(&result)?;
        txn.commit().await?;
        Ok(result)
    }
}
