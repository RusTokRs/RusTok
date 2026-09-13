use super::super::*;
use crate::services::write_transaction::record_product_operation_result;
use rustok_api::TenantLocale;

#[derive(Debug, FromQueryResult)]
struct VariantOwnerRow {
    product_id: Uuid,
}

impl ProductCatalogSchemaService {
    /// Loads canonical Product-owned attribute values for one Variant.
    ///
    /// Variant identity and storage remain separate from Product-level EAV rows even when the
    /// attribute definition is shared through `scope = both`. The requested locale is exact and
    /// canonicalized through the platform `TenantLocale` contract.
    pub async fn load_variant_attribute_values(
        &self,
        tenant_id: Uuid,
        variant_id: Uuid,
        locale: &str,
    ) -> CommerceResult<Vec<ProductAttributeValueRecord>> {
        self.load_variant_attribute_values_in(&self.db, tenant_id, variant_id, locale)
            .await
    }

    pub(super) async fn load_variant_attribute_values_in<C>(
        &self,
        conn: &C,
        tenant_id: Uuid,
        variant_id: Uuid,
        locale: &str,
    ) -> CommerceResult<Vec<ProductAttributeValueRecord>>
    where
        C: ConnectionTrait,
    {
        let locale = canonical_variant_value_locale(locale)?;
        let owner = load_variant_owner(conn, tenant_id, variant_id).await?;
        let existing_value_attribute_ids = AttributeIdRow::find_by_statement(
            Statement::from_sql_and_values(
                conn.get_database_backend(),
                "SELECT attribute_id FROM product_variant_attribute_values WHERE tenant_id = $1 AND variant_id = $2",
                vec![tenant_id.into(), variant_id.into()],
            ),
        )
        .all(conn)
        .await?
        .into_iter()
        .map(|row| row.attribute_id)
        .collect::<Vec<_>>();
        let product = load_product_primary_category(conn, tenant_id, owner.product_id).await?;
        let detached_attribute_ids = match product.primary_category_id {
            Some(category_id) => Self::load_effective_form_for_category_in(
                conn,
                tenant_id,
                category_id,
                &existing_value_attribute_ids,
            )
            .await?
            .detached_attribute_ids
            .into_iter()
            .collect::<HashSet<_>>(),
            None => existing_value_attribute_ids.into_iter().collect::<HashSet<_>>(),
        };

        let rows = ProductAttributeValueRow::find_by_statement(Statement::from_sql_and_values(
            conn.get_database_backend(),
            r#"
            SELECT
                pvav.id,
                pvav.attribute_id,
                pa.value_type,
                pa.is_localized,
                pvav.value_text,
                pvav.value_integer,
                pvav.value_decimal,
                pvav.value_boolean,
                pvav.value_date,
                pvav.value_datetime,
                pvav.value_json,
                pvav.detached_at IS NOT NULL AS detached,
                pvavt.value_text AS localized_value_text
            FROM product_variant_attribute_values pvav
            JOIN product_attributes pa
              ON pa.id = pvav.attribute_id AND pa.tenant_id = pvav.tenant_id
            LEFT JOIN product_variant_attribute_value_translations pvavt
              ON pvavt.value_id = pvav.id AND pvavt.locale = $3
            WHERE pvav.tenant_id = $1 AND pvav.variant_id = $2
            ORDER BY pa.position, pa.code
            "#,
            vec![tenant_id.into(), variant_id.into(), locale.clone().into()],
        ))
        .all(conn)
        .await?;

        let option_rows = ProductAttributeValueOptionRow::find_by_statement(
            Statement::from_sql_and_values(
                conn.get_database_backend(),
                r#"
                SELECT pvavo.value_id, pvavo.option_id
                FROM product_variant_attribute_value_options pvavo
                JOIN product_variant_attribute_values pvav ON pvav.id = pvavo.value_id
                WHERE pvav.tenant_id = $1 AND pvav.variant_id = $2
                ORDER BY pvavo.option_id
                "#,
                vec![tenant_id.into(), variant_id.into()],
            ),
        )
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

    /// Saves one exact locale of Variant-scoped EAV values through the canonical Product owner.
    ///
    /// Only effective attributes with `scope = variant|both` are admitted. All writes, the
    /// aggregate `VariantUpdated` event, and an optional mounted owner-operation receipt commit in
    /// the same Product transaction.
    pub async fn save_variant_attribute_values(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        locale: &str,
        patches: Vec<ProductAttributeValuePatch>,
    ) -> CommerceResult<Vec<ProductAttributeValueRecord>> {
        let locale = canonical_variant_value_locale(locale)?;
        validate_uuid("variant_id", variant_id)?;
        let owner = load_variant_owner(&self.db, tenant_id, variant_id).await?;
        let product = load_product_primary_category(&self.db, tenant_id, owner.product_id).await?;
        let Some(primary_category_id) = product.primary_category_id else {
            return Err(CommerceError::Validation(
                "variant product must have a primary structural category before attribute values can be saved"
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
        let definitions = load_variant_patch_definitions(&self.db, tenant_id, &patch_attribute_ids)
            .await?;
        let selected_option_ids = patches
            .iter()
            .flat_map(|patch| match &patch.value {
                ProductAttributeValuePatchValue::Select(option_id) => vec![*option_id],
                ProductAttributeValuePatchValue::Multiselect(option_ids) => option_ids.clone(),
                _ => Vec::new(),
            })
            .collect::<Vec<_>>();
        let options = load_variant_patch_options(&self.db, tenant_id, &selected_option_ids).await?;

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
                    "attribute {} is outside the variant effective schema",
                    patch.attribute_id
                )));
            }
            let definition = definitions.get(&patch.attribute_id).ok_or_else(|| {
                CommerceError::Validation(format!(
                    "attribute {} is not available",
                    patch.attribute_id
                ))
            })?;
            validate_variant_value_patch(definition, patch, &options)?;
        }

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
        let locked_owner = load_variant_owner_for_update(&txn, tenant_id, variant_id).await?;
        if locked_owner.product_id != owner.product_id {
            return Err(CommerceError::Validation(
                "variant owner changed while attribute values were being saved".into(),
            ));
        }
        for patch in &patches {
            let definition = definitions
                .get(&patch.attribute_id)
                .expect("validated Variant attribute definition must exist");
            write_variant_value_patch(
                &txn,
                tenant_id,
                variant_id,
                locale.as_str(),
                definition,
                patch,
            )
            .await?;
        }
        if !patches.is_empty() {
            touch_variant_owner(&txn, tenant_id, variant_id).await?;
            txn.publish(
                tenant_id,
                Some(actor_id),
                DomainEvent::VariantUpdated {
                    variant_id,
                    product_id: owner.product_id,
                },
            )
            .await?;
        }
        let result = self
            .load_variant_attribute_values_in(&txn, tenant_id, variant_id, locale.as_str())
            .await?;
        record_product_operation_result(&result)?;
        txn.commit().await?;
        Ok(result)
    }

    /// Deletes only currently detached Variant EAV rows through the Product owner boundary.
    pub async fn clear_detached_variant_attribute_values(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        locale: &str,
        attribute_ids: Vec<Uuid>,
    ) -> CommerceResult<Vec<ProductAttributeValueRecord>> {
        let locale = canonical_variant_value_locale(locale)?;
        validate_uuid("variant_id", variant_id)?;
        let owner = load_variant_owner(&self.db, tenant_id, variant_id).await?;
        let current = self
            .load_variant_attribute_values(tenant_id, variant_id, locale.as_str())
            .await?;
        let detached_attribute_ids = current
            .iter()
            .filter(|record| record.detached)
            .map(|record| record.attribute_id)
            .collect::<HashSet<_>>();
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
                        "attribute {} is not detached for this variant",
                        attribute_id
                    )));
                }
            }
            attribute_ids
        };

        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
        let locked_owner = load_variant_owner_for_update(&txn, tenant_id, variant_id).await?;
        if locked_owner.product_id != owner.product_id {
            return Err(CommerceError::Validation(
                "variant owner changed while detached attribute values were being cleared".into(),
            ));
        }
        if !target_attribute_ids.is_empty() {
            let (placeholders, mut values) = uuid_filter_values(tenant_id, &target_attribute_ids);
            let variant_placeholder = format!("${}", values.len() + 1);
            values.push(variant_id.into());
            txn.execute_raw(Statement::from_sql_and_values(
                txn.get_database_backend(),
                format!(
                    r#"
                    DELETE FROM product_variant_attribute_values
                    WHERE tenant_id = $1
                      AND attribute_id IN ({placeholders})
                      AND variant_id = {variant_placeholder}
                    "#
                ),
                values,
            ))
            .await?;
            touch_variant_owner(&txn, tenant_id, variant_id).await?;
            txn.publish(
                tenant_id,
                Some(actor_id),
                DomainEvent::VariantUpdated {
                    variant_id,
                    product_id: owner.product_id,
                },
            )
            .await?;
        }
        let result = self
            .load_variant_attribute_values_in(&txn, tenant_id, variant_id, locale.as_str())
            .await?;
        record_product_operation_result(&result)?;
        txn.commit().await?;
        Ok(result)
    }
}

async fn load_variant_owner<C>(
    conn: &C,
    tenant_id: Uuid,
    variant_id: Uuid,
) -> CommerceResult<VariantOwnerRow>
where
    C: ConnectionTrait,
{
    VariantOwnerRow::find_by_statement(Statement::from_sql_and_values(
        conn.get_database_backend(),
        "SELECT product_id FROM product_variants WHERE tenant_id = $1 AND id = $2",
        vec![tenant_id.into(), variant_id.into()],
    ))
    .one(conn)
    .await?
    .ok_or(CommerceError::VariantNotFound(variant_id))
}

async fn load_variant_owner_for_update<C>(
    conn: &C,
    tenant_id: Uuid,
    variant_id: Uuid,
) -> CommerceResult<VariantOwnerRow>
where
    C: ConnectionTrait,
{
    VariantOwnerRow::find_by_statement(Statement::from_sql_and_values(
        conn.get_database_backend(),
        "SELECT product_id FROM product_variants WHERE tenant_id = $1 AND id = $2 FOR UPDATE",
        vec![tenant_id.into(), variant_id.into()],
    ))
    .one(conn)
    .await?
    .ok_or(CommerceError::VariantNotFound(variant_id))
}

async fn touch_variant_owner<C>(
    conn: &C,
    tenant_id: Uuid,
    variant_id: Uuid,
) -> CommerceResult<()>
where
    C: ConnectionTrait,
{
    let result = conn
        .execute_raw(Statement::from_sql_and_values(
            conn.get_database_backend(),
            "UPDATE product_variants SET updated_at = CURRENT_TIMESTAMP WHERE tenant_id = $1 AND id = $2",
            vec![tenant_id.into(), variant_id.into()],
        ))
        .await?;
    if result.rows_affected() != 1 {
        return Err(CommerceError::VariantNotFound(variant_id));
    }
    Ok(())
}

async fn load_variant_patch_definitions<C>(
    conn: &C,
    tenant_id: Uuid,
    attribute_ids: &[Uuid],
) -> CommerceResult<HashMap<Uuid, ProductAttributeWriteDefinitionRow>>
where
    C: ConnectionTrait,
{
    if attribute_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let (placeholders, values) = uuid_filter_values(tenant_id, attribute_ids);
    Ok(ProductAttributeWriteDefinitionRow::find_by_statement(
        Statement::from_sql_and_values(
            conn.get_database_backend(),
            format!(
                "SELECT id, value_type, scope, is_localized FROM product_attributes WHERE tenant_id = $1 AND archived_at IS NULL AND id IN ({placeholders})"
            ),
            values,
        ),
    )
    .all(conn)
    .await?
    .into_iter()
    .map(|row| (row.id, row))
    .collect())
}

async fn load_variant_patch_options<C>(
    conn: &C,
    tenant_id: Uuid,
    option_ids: &[Uuid],
) -> CommerceResult<HashMap<Uuid, Uuid>>
where
    C: ConnectionTrait,
{
    if option_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let (placeholders, values) = uuid_filter_values(tenant_id, option_ids);
    Ok(ProductAttributeOptionWriteRow::find_by_statement(Statement::from_sql_and_values(
        conn.get_database_backend(),
        format!(
            "SELECT id, attribute_id FROM product_attribute_options WHERE tenant_id = $1 AND archived_at IS NULL AND id IN ({placeholders})"
        ),
        values,
    ))
    .all(conn)
    .await?
    .into_iter()
    .map(|row| (row.id, row.attribute_id))
    .collect())
}

fn validate_variant_value_patch(
    definition: &ProductAttributeWriteDefinitionRow,
    patch: &ProductAttributeValuePatch,
    options: &HashMap<Uuid, Uuid>,
) -> CommerceResult<()> {
    if !matches!(definition.scope.as_str(), "variant" | "both") {
        return Err(CommerceError::Validation(format!(
            "attribute {} is product-only",
            patch.attribute_id
        )));
    }
    let value_type = AttributeValueType::from_storage(&definition.value_type)
        .map_err(map_schema_resolution_error)?;
    let type_matches = matches!(&patch.value, ProductAttributeValuePatchValue::Clear)
        || matches!(
            (&value_type, &patch.value),
            (
                AttributeValueType::Text
                    | AttributeValueType::Textarea
                    | AttributeValueType::Richtext,
                ProductAttributeValuePatchValue::Text(_)
            ) | (
                AttributeValueType::Integer,
                ProductAttributeValuePatchValue::Integer(_)
            ) | (
                AttributeValueType::Decimal,
                ProductAttributeValuePatchValue::Decimal(_)
            ) | (
                AttributeValueType::Boolean,
                ProductAttributeValuePatchValue::Boolean(_)
            ) | (
                AttributeValueType::Date,
                ProductAttributeValuePatchValue::Date(_)
            ) | (
                AttributeValueType::Datetime,
                ProductAttributeValuePatchValue::Datetime(_)
            ) | (
                AttributeValueType::Select,
                ProductAttributeValuePatchValue::Select(_)
            ) | (
                AttributeValueType::Multiselect,
                ProductAttributeValuePatchValue::Multiselect(_)
            ) | (
                AttributeValueType::Json,
                ProductAttributeValuePatchValue::Json(_)
            )
        );
    if !type_matches {
        return Err(CommerceError::Validation(format!(
            "attribute {} expects {} value",
            patch.attribute_id,
            value_type.as_str()
        )));
    }
    if let ProductAttributeValuePatchValue::Json(value) = &patch.value {
        validate_bounded_json("attribute JSON value", value)?;
    }

    let selected_options: &[Uuid] = match &patch.value {
        ProductAttributeValuePatchValue::Select(option_id) => std::slice::from_ref(option_id),
        ProductAttributeValuePatchValue::Multiselect(option_ids) => option_ids,
        _ => &[],
    };
    let mut seen = HashSet::new();
    for option_id in selected_options {
        if !seen.insert(*option_id) {
            return Err(CommerceError::Validation(format!(
                "option {} occurs more than once",
                option_id
            )));
        }
        if options.get(option_id) != Some(&patch.attribute_id) {
            return Err(CommerceError::Validation(format!(
                "option {} does not belong to attribute {} or is archived",
                option_id, patch.attribute_id
            )));
        }
    }
    Ok(())
}

async fn write_variant_value_patch<C>(
    conn: &C,
    tenant_id: Uuid,
    variant_id: Uuid,
    locale: &str,
    definition: &ProductAttributeWriteDefinitionRow,
    patch: &ProductAttributeValuePatch,
) -> CommerceResult<()>
where
    C: ConnectionTrait,
{
    if matches!(&patch.value, ProductAttributeValuePatchValue::Clear)
        || matches!(
            &patch.value,
            ProductAttributeValuePatchValue::Multiselect(option_ids) if option_ids.is_empty()
        )
    {
        conn.execute_raw(Statement::from_sql_and_values(
            conn.get_database_backend(),
            "DELETE FROM product_variant_attribute_values WHERE tenant_id = $1 AND variant_id = $2 AND attribute_id = $3",
            vec![tenant_id.into(), variant_id.into(), patch.attribute_id.into()],
        ))
        .await?;
        return Ok(());
    }

    let mut value_text = None;
    let mut value_integer = None;
    let mut value_decimal = None;
    let mut value_boolean = None;
    let mut value_date = None;
    let mut value_datetime = None;
    let mut value_json = None;
    let mut option_ids = Vec::new();
    let mut localized_text = None;

    match &patch.value {
        ProductAttributeValuePatchValue::Clear => unreachable!(),
        ProductAttributeValuePatchValue::Text(value) if definition.is_localized => {
            localized_text = Some(value.clone())
        }
        ProductAttributeValuePatchValue::Text(value) => value_text = Some(value.clone()),
        ProductAttributeValuePatchValue::Integer(value) => value_integer = Some(*value),
        ProductAttributeValuePatchValue::Decimal(value) => value_decimal = Some(*value),
        ProductAttributeValuePatchValue::Boolean(value) => value_boolean = Some(*value),
        ProductAttributeValuePatchValue::Date(value) => value_date = Some(*value),
        ProductAttributeValuePatchValue::Datetime(value) => value_datetime = Some(*value),
        ProductAttributeValuePatchValue::Select(option_id) => option_ids.push(*option_id),
        ProductAttributeValuePatchValue::Multiselect(values) => option_ids.extend(values),
        ProductAttributeValuePatchValue::Json(value) => value_json = Some(value.clone()),
    }

    let value_id = IdRow::find_by_statement(Statement::from_sql_and_values(
        conn.get_database_backend(),
        r#"
        INSERT INTO product_variant_attribute_values (
            id, tenant_id, variant_id, attribute_id, value_text, value_integer,
            value_decimal, value_boolean, value_date, value_datetime, value_json,
            detached_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, NULL)
        ON CONFLICT (tenant_id, variant_id, attribute_id) DO UPDATE SET
            value_text = EXCLUDED.value_text,
            value_integer = EXCLUDED.value_integer,
            value_decimal = EXCLUDED.value_decimal,
            value_boolean = EXCLUDED.value_boolean,
            value_date = EXCLUDED.value_date,
            value_datetime = EXCLUDED.value_datetime,
            value_json = EXCLUDED.value_json,
            detached_at = NULL,
            updated_at = now()
        RETURNING id
        "#,
        vec![
            generate_id().into(),
            tenant_id.into(),
            variant_id.into(),
            patch.attribute_id.into(),
            value_text.into(),
            value_integer.into(),
            value_decimal.into(),
            value_boolean.into(),
            value_date.into(),
            value_datetime.into(),
            value_json.into(),
        ],
    ))
    .one(conn)
    .await?
    .expect("INSERT RETURNING id must return a row")
    .id;

    conn.execute_raw(Statement::from_sql_and_values(
        conn.get_database_backend(),
        "DELETE FROM product_variant_attribute_value_options WHERE value_id = $1",
        vec![value_id.into()],
    ))
    .await?;
    for option_id in option_ids {
        conn.execute_raw(Statement::from_sql_and_values(
            conn.get_database_backend(),
            "INSERT INTO product_variant_attribute_value_options (tenant_id, value_id, option_id) VALUES ($1, $2, $3)",
            vec![tenant_id.into(), value_id.into(), option_id.into()],
        ))
        .await?;
    }

    if let Some(value) = localized_text {
        conn.execute_raw(Statement::from_sql_and_values(
            conn.get_database_backend(),
            r#"
            INSERT INTO product_variant_attribute_value_translations (id, value_id, locale, value_text)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (value_id, locale) DO UPDATE SET value_text = EXCLUDED.value_text
            "#,
            vec![
                generate_id().into(),
                value_id.into(),
                locale.into(),
                value.into(),
            ],
        ))
        .await?;
    }
    Ok(())
}

fn canonical_variant_value_locale(locale: &str) -> CommerceResult<String> {
    TenantLocale::new(locale)
        .map(TenantLocale::into_inner)
        .map_err(|error| CommerceError::Validation(error.to_string()))
}
