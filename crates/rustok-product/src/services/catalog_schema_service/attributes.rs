use super::{
    AttributeValueType, CreateProductAttributeInput, CreateProductAttributeOptionInput,
    ProductAttributeListRecord, ProductAttributeListRow, ProductAttributeOptionListRecord,
    ProductAttributeOptionListRow, ProductAttributeOptionRecord, ProductAttributeRecord,
    ProductCatalogSchemaService, load_attribute_write_definition, map_schema_resolution_error,
    uuid_filter_values, validate_locale,
};
use rustok_api::PortError;
use rustok_outbox::idempotency;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, FromQueryResult, Statement};
use serde::Serialize;
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::{CommerceError, CommerceResult};
use crate::services::catalog::types::validate_product_attribute_filters;
use crate::services::catalog_attribute_terms::{
    ProductAttributeFilterValue, parse_product_attribute_filter_value,
};
use crate::services::write_transaction::{
    ProductWriteTransaction, current_product_operation_id, record_product_operation_result,
};
use crate::services::{
    ProductAttributeFilter, ProductAttributeTermError, ProductAttributeTermExpr,
    ProductResolvedAttributeFilter, product_attribute_boolean_term, product_attribute_date_term,
    product_attribute_datetime_term, product_attribute_decimal_term,
    product_attribute_integer_term, product_attribute_localized_text_expr,
    product_attribute_option_term, product_attribute_text_term,
};
use rustok_core::generate_id;
use rustok_events::DomainEvent;

const PRODUCT_SCHEMA_RECEIPT_OWNER: &str = "product";

impl ProductCatalogSchemaService {
    pub async fn create_attribute(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: CreateProductAttributeInput,
    ) -> CommerceResult<ProductAttributeRecord> {
        input.validate()?;
        let attribute_id = current_product_operation_id().unwrap_or_else(generate_id);
        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;

        txn.execute(Statement::from_sql_and_values(
            txn.get_database_backend(),
            r#"
            INSERT INTO product_attributes (
                id, tenant_id, code, value_type, scope, is_localized,
                is_filterable, is_searchable, is_sortable, is_comparable,
                show_on_storefront, show_in_admin_grid, search_weight,
                filter_display, facet_mode, position, validation, default_value, metadata
            ) VALUES (
                $1, $2, $3, $4, $5, $6,
                $7, $8, $9, $10,
                $11, $12, $13,
                $14, $15, $16, $17, $18, $19
            )
            "#,
            vec![
                attribute_id.into(),
                tenant_id.into(),
                input.code.clone().into(),
                input.value_type.as_str().into(),
                input.scope.clone().into(),
                input.is_localized.into(),
                input.is_filterable.into(),
                input.is_searchable.into(),
                input.is_sortable.into(),
                input.is_comparable.into(),
                input.show_on_storefront.into(),
                input.show_in_admin_grid.into(),
                input.search_weight.into(),
                input.filter_display.clone().into(),
                input.facet_mode.clone().into(),
                input.position.into(),
                input.validation.clone().into(),
                input.default_value.clone().into(),
                input.metadata.clone().into(),
            ],
        ))
        .await?;

        for translation in &input.translations {
            txn.execute(Statement::from_sql_and_values(
                txn.get_database_backend(),
                r#"
                INSERT INTO product_attribute_translations (
                    id, attribute_id, locale, label, help_text, facet_label, seo_label
                ) VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
                vec![
                    generate_id().into(),
                    attribute_id.into(),
                    translation.locale.clone().into(),
                    translation.label.clone().into(),
                    translation.help_text.clone().into(),
                    translation.facet_label.clone().into(),
                    translation.seo_label.clone().into(),
                ],
            ))
            .await?;
        }

        txn.publish(
            idempotency::OwnerOperationScope::Tenant(tenant_id),
            Some(actor_id),
            DomainEvent::ProductAttributeCreated { attribute_id },
        )
        .await?;
        let result = ProductAttributeRecord {
            id: attribute_id,
            code: input.code,
            value_type: input.value_type,
        };
        record_product_operation_result(&result)?;
        txn.commit().await?;
        Ok(result)
    }

    pub async fn create_attribute_option(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: CreateProductAttributeOptionInput,
    ) -> CommerceResult<ProductAttributeOptionRecord> {
        input.validate()?;
        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
        let attribute =
            load_attribute_write_definition(&txn, tenant_id, input.attribute_id).await?;
        let value_type = AttributeValueType::from_storage(&attribute.value_type)
            .map_err(map_schema_resolution_error)?;
        if !matches!(
            value_type,
            AttributeValueType::Select | AttributeValueType::Multiselect
        ) {
            return Err(CommerceError::Validation(
                "options can only be created for select or multiselect attributes".into(),
            ));
        }

        let option_id = current_product_operation_id().unwrap_or_else(generate_id);
        txn.execute(Statement::from_sql_and_values(
            txn.get_database_backend(),
            r#"
            INSERT INTO product_attribute_options (
                id, tenant_id, attribute_id, code, position, metadata
            ) VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            vec![
                option_id.into(),
                tenant_id.into(),
                input.attribute_id.into(),
                input.code.clone().into(),
                input.position.into(),
                input.metadata.clone().into(),
            ],
        ))
        .await?;
        for translation in &input.translations {
            txn.execute(Statement::from_sql_and_values(
                txn.get_database_backend(),
                r#"
                INSERT INTO product_attribute_option_translations (
                    id, option_id, locale, label
                ) VALUES ($1, $2, $3, $4)
                "#,
                vec![
                    generate_id().into(),
                    option_id.into(),
                    translation.locale.clone().into(),
                    translation.label.clone().into(),
                ],
            ))
            .await?;
        }
        txn.publish(
            tenant_id,
            Some(actor_id),
            DomainEvent::ProductAttributeOptionCreated {
                option_id,
                attribute_id: input.attribute_id,
            },
        )
        .await?;
        let result = ProductAttributeOptionRecord {
            id: option_id,
            attribute_id: input.attribute_id,
            code: input.code,
        };
        record_product_operation_result(&result)?;
        txn.commit().await?;
        Ok(result)
    }

    pub(crate) async fn admit_schema_operation_receipt<T: Serialize>(
        &self,
        tenant_id: Uuid,
        idempotency_key: &str,
        operation: &str,
        request: &T,
    ) -> Result<idempotency::Admission, PortError> {
        idempotency::admit(
            &self.db,
            tenant_id,
            PRODUCT_SCHEMA_RECEIPT_OWNER,
            idempotency_key,
            operation,
            request,
        )
        .await
    }

    pub(crate) async fn fail_schema_operation_receipt(
        &self,
        lease: idempotency::Lease,
        error: &PortError,
    ) -> Result<(), PortError> {
        idempotency::fail(&self.db, lease, error).await
    }

    pub async fn list_attributes(
        &self,
        tenant_id: Uuid,
        locale: &str,
    ) -> CommerceResult<Vec<ProductAttributeListRecord>> {
        ProductAttributeListRow::find_by_statement(Statement::from_sql_and_values(
            self.db.get_database_backend(),
            r#"
            SELECT
                a.id,
                a.code,
                a.value_type,
                a.is_localized,
                a.is_filterable,
                a.is_searchable,
                a.is_sortable,
                a.show_on_storefront,
                COALESCE(t.label, a.code) AS label
            FROM product_attributes a
            LEFT JOIN product_attribute_translations t
                ON t.attribute_id = a.id AND t.locale = $2
            WHERE a.tenant_id = $1 AND a.archived_at IS NULL
            ORDER BY a.position ASC, a.code ASC
            "#,
            vec![tenant_id.into(), locale.to_string().into()],
        ))
        .all(&self.db)
        .await
        .map_err(Into::into)
        .and_then(|rows| rows.into_iter().map(TryInto::try_into).collect())
    }

    pub async fn list_attribute_options(
        &self,
        tenant_id: Uuid,
        attribute_ids: &[Uuid],
        locale: &str,
    ) -> CommerceResult<Vec<ProductAttributeOptionListRecord>> {
        validate_locale(locale)?;
        if attribute_ids.is_empty() {
            return Ok(Vec::new());
        }
        let (placeholders, mut values) = uuid_filter_values(tenant_id, attribute_ids);
        let locale_placeholder = format!("${}", values.len() + 1);
        values.push(locale.trim().to_string().into());
        ProductAttributeOptionListRow::find_by_statement(Statement::from_sql_and_values(
            self.db.get_database_backend(),
            format!(
                r#"
                SELECT o.id, o.attribute_id, o.code, o.position,
                       COALESCE(t.label, o.code) AS label
                FROM product_attribute_options o
                LEFT JOIN product_attribute_option_translations t
                  ON t.option_id = o.id AND t.locale = {locale_placeholder}
                WHERE o.tenant_id = $1
                  AND o.archived_at IS NULL
                  AND o.attribute_id IN ({placeholders})
                ORDER BY o.attribute_id, o.position, o.code
                "#
            ),
            values,
        ))
        .all(&self.db)
        .await
        .map(|rows| rows.into_iter().map(Into::into).collect())
        .map_err(Into::into)
    }

    /// Resolve public Storefront attribute filters into the canonical Product-owned term grammar.
    ///
    /// This is an owner capability: consumers do not read Product attribute/option tables directly.
    /// Missing option codes resolve to `Never`, matching the existing owner SQL's empty-result behavior.
    pub async fn resolve_storefront_attribute_filter_terms(
        &self,
        tenant_id: Uuid,
        requested_locale: &str,
        fallback_locale: &str,
        filters: &[ProductAttributeFilter],
    ) -> CommerceResult<Vec<ProductResolvedAttributeFilter>> {
        validate_product_attribute_filters(filters)?;
        if filters.is_empty() {
            return Ok(Vec::new());
        }

        let definitions = load_storefront_filter_definitions(&self.db, tenant_id, filters).await?;
        let mut resolved = Vec::with_capacity(filters.len());
        for filter in filters {
            let definition = definitions
                .get(&filter.code.to_ascii_lowercase())
                .ok_or_else(|| {
                    CommerceError::Validation(format!(
                        "attribute {} is not available as a product filter",
                        filter.code
                    ))
                })?;
            let value_type = AttributeValueType::from_storage(definition.value_type.as_str())
                .map_err(|_| {
                    CommerceError::Validation(format!(
                        "attribute {} has an unsupported stored value type",
                        definition.code
                    ))
                })?;
            let predicate = resolve_storefront_filter_predicate(
                &self.db,
                tenant_id,
                definition,
                value_type,
                filter.value.as_str(),
                requested_locale,
                fallback_locale,
            )
            .await?;
            resolved.push(ProductResolvedAttributeFilter {
                code: filter.code.clone(),
                predicate,
            });
        }
        Ok(resolved)
    }
}

#[derive(Debug, FromQueryResult)]
struct StorefrontAttributeFilterDefinitionRow {
    id: Uuid,
    code: String,
    value_type: String,
    is_localized: bool,
}

#[derive(Debug, FromQueryResult)]
struct StorefrontAttributeFilterOptionRow {
    id: Uuid,
}

async fn load_storefront_filter_definitions(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    filters: &[ProductAttributeFilter],
) -> CommerceResult<HashMap<String, StorefrontAttributeFilterDefinitionRow>> {
    let backend = db.get_database_backend();
    let mut values = Vec::<sea_orm::Value>::with_capacity(filters.len() + 1);
    values.push(tenant_id.into());
    let placeholders = filters
        .iter()
        .enumerate()
        .map(|(index, filter)| {
            values.push(filter.code.to_ascii_lowercase().into());
            storefront_sql_placeholder(backend, index + 2)
        })
        .collect::<Vec<_>>()
        .join(", ");
    let tenant_placeholder = storefront_sql_placeholder(backend, 1);
    StorefrontAttributeFilterDefinitionRow::find_by_statement(Statement::from_sql_and_values(
        backend,
        format!(
            r#"
            SELECT id, code, value_type, is_localized
            FROM product_attributes
            WHERE tenant_id = {tenant_placeholder}
              AND archived_at IS NULL
              AND is_filterable = TRUE
              AND scope IN ('product', 'both')
              AND LOWER(code) IN ({placeholders})
            "#
        ),
        values,
    ))
    .all(db)
    .await
    .map(|rows| {
        rows.into_iter()
            .map(|definition| (definition.code.to_ascii_lowercase(), definition))
            .collect()
    })
    .map_err(Into::into)
}

async fn resolve_storefront_filter_predicate(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    definition: &StorefrontAttributeFilterDefinitionRow,
    value_type: AttributeValueType,
    raw_value: &str,
    requested_locale: &str,
    fallback_locale: &str,
) -> CommerceResult<ProductAttributeTermExpr> {
    let value =
        parse_product_attribute_filter_value(definition.code.as_str(), value_type, raw_value)?;
    let term = match value {
        ProductAttributeFilterValue::Text(value) if definition.is_localized => {
            return product_attribute_localized_text_expr(
                definition.id,
                requested_locale,
                fallback_locale,
                value.as_str(),
            )
            .map_err(|error| map_term_error(definition.code.as_str(), error));
        }
        ProductAttributeFilterValue::Text(value) => {
            product_attribute_text_term(definition.id, value.as_str())
        }
        ProductAttributeFilterValue::Integer(value) => {
            product_attribute_integer_term(definition.id, value)
        }
        ProductAttributeFilterValue::Decimal(value) => {
            product_attribute_decimal_term(definition.id, value)
        }
        ProductAttributeFilterValue::Boolean(value) => {
            product_attribute_boolean_term(definition.id, value)
        }
        ProductAttributeFilterValue::Date(value) => {
            product_attribute_date_term(definition.id, value)
        }
        ProductAttributeFilterValue::Datetime(value) => {
            product_attribute_datetime_term(definition.id, value)
        }
        ProductAttributeFilterValue::Option(raw_value) => {
            let option_id = match Uuid::parse_str(raw_value.as_str()) {
                Ok(option_id) if option_id.is_nil() => return Ok(ProductAttributeTermExpr::Never),
                Ok(option_id) => option_id,
                Err(_) => {
                    let Some(option_id) =
                        load_active_option_id(db, tenant_id, definition.id, raw_value.as_str())
                            .await?
                    else {
                        return Ok(ProductAttributeTermExpr::Never);
                    };
                    option_id
                }
            };
            product_attribute_option_term(definition.id, option_id)
        }
    }
    .map_err(|error| map_term_error(definition.code.as_str(), error))?;

    Ok(ProductAttributeTermExpr::Term(term))
}

async fn load_active_option_id(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    attribute_id: Uuid,
    code: &str,
) -> CommerceResult<Option<Uuid>> {
    let backend = db.get_database_backend();
    let tenant = storefront_sql_placeholder(backend, 1);
    let attribute = storefront_sql_placeholder(backend, 2);
    let code_placeholder = storefront_sql_placeholder(backend, 3);
    let row =
        StorefrontAttributeFilterOptionRow::find_by_statement(Statement::from_sql_and_values(
            backend,
            format!(
                r#"
            SELECT id
            FROM product_attribute_options
            WHERE tenant_id = {tenant}
              AND attribute_id = {attribute}
              AND archived_at IS NULL
              AND code = {code_placeholder}
            LIMIT 1
            "#
            ),
            vec![
                tenant_id.into(),
                attribute_id.into(),
                code.to_string().into(),
            ],
        ))
        .one(db)
        .await?;
    Ok(row.map(|row| row.id))
}

fn storefront_sql_placeholder(backend: DbBackend, index: usize) -> String {
    match backend {
        DbBackend::Sqlite => "?".to_string(),
        _ => format!("${index}"),
    }
}

fn map_term_error(code: &str, error: ProductAttributeTermError) -> CommerceError {
    CommerceError::Validation(format!(
        "attribute filter {code} cannot be represented as a canonical term: {error}"
    ))
}
