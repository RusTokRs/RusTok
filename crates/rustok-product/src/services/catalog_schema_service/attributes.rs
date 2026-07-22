use super::{
    AttributeValueType, CreateProductAttributeInput, CreateProductAttributeOptionInput,
    ProductAttributeListRecord, ProductAttributeListRow, ProductAttributeOptionListRecord,
    ProductAttributeOptionListRow, ProductAttributeOptionRecord, ProductAttributeRecord,
    ProductCatalogSchemaService, load_attribute_write_definition, map_schema_resolution_error,
    uuid_filter_values, validate_locale,
};
use sea_orm::{ConnectionTrait, FromQueryResult, Statement, TransactionTrait};
use uuid::Uuid;

use rustok_commerce_foundation::error::{CommerceError, CommerceResult};
use rustok_core::generate_id;
use rustok_events::DomainEvent;

impl ProductCatalogSchemaService {
    pub async fn create_attribute(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: CreateProductAttributeInput,
    ) -> CommerceResult<ProductAttributeRecord> {
        input.validate()?;
        let attribute_id = generate_id();
        let txn = self.db.begin().await?;

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

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductAttributeCreated { attribute_id },
            )
            .await?;
        txn.commit().await?;

        Ok(ProductAttributeRecord {
            id: attribute_id,
            code: input.code,
            value_type: input.value_type,
        })
    }

    pub async fn create_attribute_option(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: CreateProductAttributeOptionInput,
    ) -> CommerceResult<ProductAttributeOptionRecord> {
        input.validate()?;
        let txn = self.db.begin().await?;
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

        let option_id = generate_id();
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
        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductAttributeOptionCreated {
                    option_id,
                    attribute_id: input.attribute_id,
                },
            )
            .await?;
        txn.commit().await?;
        Ok(ProductAttributeOptionRecord {
            id: option_id,
            attribute_id: input.attribute_id,
            code: input.code,
        })
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
}
