use super::{
    BindSchemaAttributeInput, CreateProductAttributeSchemaGroupInput,
    CreateProductAttributeSchemaInput, ProductAttributeGroupRecord,
    ProductAttributeSchemaListRecord, ProductAttributeSchemaListRow, ProductAttributeSchemaRecord,
    ProductCatalogSchemaService, ensure_attribute, ensure_schema, insert_schema_group_translation,
    load_schema_group_id,
};
use sea_orm::{ConnectionTrait, FromQueryResult, Statement, TransactionTrait};
use uuid::Uuid;

use rustok_commerce_foundation::error::{CommerceError, CommerceResult};
use rustok_core::generate_id;
use rustok_events::DomainEvent;

impl ProductCatalogSchemaService {
    pub async fn create_schema(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: CreateProductAttributeSchemaInput,
    ) -> CommerceResult<ProductAttributeSchemaRecord> {
        input.validate()?;
        let schema_id = generate_id();
        let txn = self.db.begin().await?;

        txn.execute(Statement::from_sql_and_values(
            txn.get_database_backend(),
            r#"
            INSERT INTO product_attribute_schemas (id, tenant_id, code, metadata)
            VALUES ($1, $2, $3, $4)
            "#,
            vec![
                schema_id.into(),
                tenant_id.into(),
                input.code.clone().into(),
                input.metadata.clone().into(),
            ],
        ))
        .await?;

        for translation in &input.translations {
            txn.execute(Statement::from_sql_and_values(
                txn.get_database_backend(),
                r#"
                INSERT INTO product_attribute_schema_translations (
                    id, schema_id, locale, name, description
                ) VALUES ($1, $2, $3, $4, $5)
                "#,
                vec![
                    generate_id().into(),
                    schema_id.into(),
                    translation.locale.clone().into(),
                    translation.name.clone().into(),
                    translation.description.clone().into(),
                ],
            ))
            .await?;
        }

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductAttributeSchemaCreated { schema_id },
            )
            .await?;
        txn.commit().await?;

        Ok(ProductAttributeSchemaRecord {
            id: schema_id,
            code: input.code,
        })
    }

    pub async fn list_schemas(
        &self,
        tenant_id: Uuid,
        locale: &str,
    ) -> CommerceResult<Vec<ProductAttributeSchemaListRecord>> {
        ProductAttributeSchemaListRow::find_by_statement(Statement::from_sql_and_values(
            self.db.get_database_backend(),
            r#"
            SELECT
                s.id,
                s.code,
                COALESCE(t.name, s.code) AS name
            FROM product_attribute_schemas s
            LEFT JOIN product_attribute_schema_translations t
                ON t.schema_id = s.id AND t.locale = $2
            WHERE s.tenant_id = $1 AND s.archived_at IS NULL
            ORDER BY s.code ASC
            "#,
            vec![tenant_id.into(), locale.to_string().into()],
        ))
        .all(&self.db)
        .await
        .map(|rows| rows.into_iter().map(Into::into).collect())
        .map_err(Into::into)
    }

    pub async fn create_schema_group(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: CreateProductAttributeSchemaGroupInput,
    ) -> CommerceResult<ProductAttributeGroupRecord> {
        input.validate()?;
        let txn = self.db.begin().await?;
        ensure_schema(&txn, tenant_id, input.schema_id).await?;
        let group_id = generate_id();
        txn.execute(Statement::from_sql_and_values(
            txn.get_database_backend(),
            r#"
            INSERT INTO product_attribute_schema_groups (
                id, tenant_id, schema_id, code, position, metadata
            ) VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            vec![
                group_id.into(),
                tenant_id.into(),
                input.schema_id.into(),
                input.code.clone().into(),
                input.position.into(),
                input.metadata.clone().into(),
            ],
        ))
        .await?;
        for translation in &input.translations {
            insert_schema_group_translation(&txn, group_id, translation).await?;
        }
        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductAttributeSchemaBindingsChanged {
                    schema_id: input.schema_id,
                },
            )
            .await?;
        txn.commit().await?;
        Ok(ProductAttributeGroupRecord {
            id: group_id,
            owner_id: input.schema_id,
            code: input.code,
        })
    }

    pub async fn bind_schema_attribute(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: BindSchemaAttributeInput,
    ) -> CommerceResult<()> {
        input.validate()?;
        let txn = self.db.begin().await?;
        ensure_attribute(&txn, tenant_id, input.attribute_id).await?;
        ensure_schema(&txn, tenant_id, input.schema_id).await?;
        let group_id = match input.group_code.as_deref() {
            Some(code) => Some(
                load_schema_group_id(&txn, tenant_id, input.schema_id, code)
                    .await?
                    .ok_or_else(|| CommerceError::Validation("schema group not found".into()))?,
            ),
            None => None,
        };

        txn.execute(Statement::from_sql_and_values(
            txn.get_database_backend(),
            r#"
            INSERT INTO product_attribute_schema_attributes (
                id, tenant_id, schema_id, attribute_id, group_id, is_required,
                is_disabled, position, visibility_overrides, validation_overrides, metadata
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (schema_id, attribute_id) DO UPDATE SET
                group_id = EXCLUDED.group_id,
                is_required = EXCLUDED.is_required,
                is_disabled = EXCLUDED.is_disabled,
                position = EXCLUDED.position,
                visibility_overrides = EXCLUDED.visibility_overrides,
                validation_overrides = EXCLUDED.validation_overrides,
                metadata = EXCLUDED.metadata
            "#,
            vec![
                generate_id().into(),
                tenant_id.into(),
                input.schema_id.into(),
                input.attribute_id.into(),
                group_id.into(),
                input.is_required.into(),
                input.is_disabled.into(),
                input.position.into(),
                input.visibility_overrides.clone().into(),
                input.validation_overrides.clone().into(),
                input.metadata.clone().into(),
            ],
        ))
        .await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                Some(actor_id),
                DomainEvent::ProductAttributeSchemaBindingsChanged {
                    schema_id: input.schema_id,
                },
            )
            .await?;
        txn.commit().await?;
        Ok(())
    }
}
