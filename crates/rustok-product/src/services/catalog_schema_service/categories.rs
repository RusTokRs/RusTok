use super::{
    BindCategoryAttributeInput, CatalogCategoryKind, CatalogCategoryListRecord,
    CatalogCategoryListRow, CatalogCategoryRecord, CreateCatalogCategoryInput,
    CreateCategoryAttributeGroupInput, ProductAttributeGroupRecord, ProductCatalogSchemaService,
    SetCategorySchemaModeInput, ensure_attribute, ensure_schema, ensure_structural_category,
    insert_category_group_translation, load_category_group_id, load_category_parent,
    parse_virtual_category_rule_v1, validate_virtual_category_rule_references,
};
use sea_orm::{ConnectionTrait, FromQueryResult, Statement, TransactionTrait};
use serde_json::Value;
use uuid::Uuid;

use rustok_commerce_foundation::error::{CommerceError, CommerceResult};
use rustok_core::generate_id;
use rustok_events::DomainEvent;

impl ProductCatalogSchemaService {
    pub async fn create_category(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: CreateCatalogCategoryInput,
    ) -> CommerceResult<CatalogCategoryRecord> {
        input.validate()?;
        let category_id = generate_id();
        let txn = self.db.begin().await?;

        if input.kind == CatalogCategoryKind::Virtual {
            let rule = parse_virtual_category_rule_v1(&input.rule_config)
                .map_err(CommerceError::Validation)?;
            validate_virtual_category_rule_references(&txn, tenant_id, &rule).await?;
        }

        let parent = match input.parent_id {
            Some(parent_id) => Some(load_category_parent(&txn, tenant_id, parent_id).await?),
            None => None,
        };
        let level = parent.as_ref().map(|row| row.level + 1).unwrap_or(0);
        let path = parent
            .as_ref()
            .map(|row| format!("{}/{}", row.path, input.slug))
            .unwrap_or_else(|| input.slug.clone());

        txn.execute(Statement::from_sql_and_values(
            txn.get_database_backend(),
            r#"
            INSERT INTO catalog_categories (
                id, tenant_id, parent_id, code, slug, kind, path, level, position,
                is_active, rule_config, metadata
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, TRUE, $10, $11)
            "#,
            vec![
                category_id.into(),
                tenant_id.into(),
                input.parent_id.into(),
                input.code.clone().into(),
                input.slug.clone().into(),
                input.kind.as_str().into(),
                path.clone().into(),
                level.into(),
                input.position.into(),
                input.rule_config.clone().into(),
                input.metadata.clone().into(),
            ],
        ))
        .await?;

        txn.execute(Statement::from_sql_and_values(
            txn.get_database_backend(),
            r#"
            INSERT INTO catalog_category_closure (tenant_id, ancestor_id, descendant_id, depth)
            VALUES ($1, $2, $2, 0)
            "#,
            vec![tenant_id.into(), category_id.into()],
        ))
        .await?;

        if let Some(parent_id) = input.parent_id {
            txn.execute(Statement::from_sql_and_values(
                txn.get_database_backend(),
                r#"
                INSERT INTO catalog_category_closure (
                    tenant_id, ancestor_id, descendant_id, depth
                )
                SELECT tenant_id, ancestor_id, $3, depth + 1
                FROM catalog_category_closure
                WHERE tenant_id = $1 AND descendant_id = $2
                "#,
                vec![tenant_id.into(), parent_id.into(), category_id.into()],
            ))
            .await?;
        }

        for translation in &input.translations {
            txn.execute(Statement::from_sql_and_values(
                txn.get_database_backend(),
                r#"
                INSERT INTO catalog_category_translations (
                    id, category_id, locale, name, description, meta_title, meta_description
                ) VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
                vec![
                    generate_id().into(),
                    category_id.into(),
                    translation.locale.clone().into(),
                    translation.name.clone().into(),
                    translation.description.clone().into(),
                    translation.meta_title.clone().into(),
                    translation.meta_description.clone().into(),
                ],
            ))
            .await?;
        }

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                Some(actor_id),
                DomainEvent::CatalogCategoryCreated { category_id },
            )
            .await?;
        txn.commit().await?;

        Ok(CatalogCategoryRecord {
            id: category_id,
            code: input.code,
            slug: input.slug,
            path,
            kind: input.kind,
        })
    }

    pub async fn list_categories(
        &self,
        tenant_id: Uuid,
        locale: &str,
    ) -> CommerceResult<Vec<CatalogCategoryListRecord>> {
        CatalogCategoryListRow::find_by_statement(Statement::from_sql_and_values(
            self.db.get_database_backend(),
            r#"
            SELECT
                c.id,
                c.parent_id,
                c.code,
                c.slug,
                c.path,
                c.kind,
                COALESCE(t.name, c.code) AS name
            FROM catalog_categories c
            LEFT JOIN catalog_category_translations t
                ON t.category_id = c.id AND t.locale = $2
            WHERE c.tenant_id = $1 AND c.deleted_at IS NULL
            ORDER BY c.path ASC
            "#,
            vec![tenant_id.into(), locale.to_string().into()],
        ))
        .all(&self.db)
        .await
        .map_err(Into::into)
        .and_then(|rows| rows.into_iter().map(TryInto::try_into).collect())
    }

    pub async fn create_category_group(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: CreateCategoryAttributeGroupInput,
    ) -> CommerceResult<ProductAttributeGroupRecord> {
        input.validate()?;
        let txn = self.db.begin().await?;
        ensure_structural_category(&txn, tenant_id, input.category_id).await?;
        let group_id = generate_id();
        txn.execute(Statement::from_sql_and_values(
            txn.get_database_backend(),
            r#"
            INSERT INTO category_attribute_groups (
                id, tenant_id, category_id, code, inherited_from_group_id, position, metadata
            ) VALUES ($1, $2, $3, $4, NULL, $5, $6)
            "#,
            vec![
                group_id.into(),
                tenant_id.into(),
                input.category_id.into(),
                input.code.clone().into(),
                input.position.into(),
                input.metadata.clone().into(),
            ],
        ))
        .await?;
        for translation in &input.translations {
            insert_category_group_translation(&txn, group_id, translation).await?;
        }
        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                Some(actor_id),
                DomainEvent::CatalogCategoryAttributesChanged {
                    category_id: input.category_id,
                },
            )
            .await?;
        txn.commit().await?;

        Ok(ProductAttributeGroupRecord {
            id: group_id,
            owner_id: input.category_id,
            code: input.code,
        })
    }

    pub async fn set_category_schema_mode(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: SetCategorySchemaModeInput,
    ) -> CommerceResult<()> {
        input.validate()?;
        let txn = self.db.begin().await?;
        ensure_structural_category(&txn, tenant_id, input.category_id).await?;
        if let Some(schema_id) = input.schema_id {
            ensure_schema(&txn, tenant_id, schema_id).await?;
        }

        let snapshot = if let Some(source_category_id) = input.clone_from_category_id {
            let form = self
                .load_effective_form_for_category(tenant_id, source_category_id, &[])
                .await?;
            serde_json::to_value(form.attributes)
                .map_err(|error| CommerceError::Validation(error.to_string()))?
        } else {
            Value::Object(Default::default())
        };

        txn.execute(Statement::from_sql_and_values(
            txn.get_database_backend(),
            r#"
            INSERT INTO category_attribute_schema_assignments (
                id, tenant_id, category_id, mode, schema_id, cloned_from_category_id, snapshot
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (tenant_id, category_id) DO UPDATE SET
                mode = EXCLUDED.mode,
                schema_id = EXCLUDED.schema_id,
                cloned_from_category_id = EXCLUDED.cloned_from_category_id,
                snapshot = EXCLUDED.snapshot,
                updated_at = now()
            "#,
            vec![
                generate_id().into(),
                tenant_id.into(),
                input.category_id.into(),
                input.mode.as_str().into(),
                input.schema_id.into(),
                input.clone_from_category_id.into(),
                snapshot.into(),
            ],
        ))
        .await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                Some(actor_id),
                DomainEvent::CatalogCategorySchemaModeChanged {
                    category_id: input.category_id,
                },
            )
            .await?;
        txn.commit().await?;
        Ok(())
    }

    pub async fn bind_category_attribute(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: BindCategoryAttributeInput,
    ) -> CommerceResult<()> {
        input.validate()?;
        let txn = self.db.begin().await?;
        ensure_structural_category(&txn, tenant_id, input.category_id).await?;
        ensure_attribute(&txn, tenant_id, input.attribute_id).await?;
        let group_id = match input.group_code.as_deref() {
            Some(code) => Some(
                load_category_group_id(&txn, tenant_id, input.category_id, code)
                    .await?
                    .ok_or_else(|| CommerceError::Validation("category group not found".into()))?,
            ),
            None => None,
        };

        txn.execute(Statement::from_sql_and_values(
            txn.get_database_backend(),
            r#"
            INSERT INTO category_attributes (
                id, tenant_id, category_id, attribute_id, group_id, binding_kind,
                is_required, is_disabled, position, visibility_overrides,
                validation_overrides, metadata
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (category_id, attribute_id) DO UPDATE SET
                group_id = EXCLUDED.group_id,
                binding_kind = EXCLUDED.binding_kind,
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
                input.category_id.into(),
                input.attribute_id.into(),
                group_id.into(),
                input.binding_kind.as_str().into(),
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
                DomainEvent::CatalogCategoryAttributesChanged {
                    category_id: input.category_id,
                },
            )
            .await?;
        txn.commit().await?;
        Ok(())
    }
}
