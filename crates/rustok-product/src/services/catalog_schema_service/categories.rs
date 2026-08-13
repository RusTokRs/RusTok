use std::collections::HashSet;

use super::{
    BindCategoryAttributeInput, CatalogCategoryKind, CatalogCategoryListRecord,
    CatalogCategoryListRow, CatalogCategoryRecord, CategoryTranslationInput, CreateCatalogCategoryInput,
    CreateCategoryAttributeGroupInput, ProductAttributeGroupRecord, ProductCatalogSchemaService,
    SetCategorySchemaModeInput, ensure_attribute, ensure_schema, ensure_structural_category,
    insert_category_group_translation, load_category_group_id, load_category_parent,
    parse_virtual_category_rule_v1, validate_virtual_category_rule_references,
};
use sea_orm::{ConnectionTrait, FromQueryResult, Statement};
use serde_json::Value;
use uuid::Uuid;

use crate::error::{CommerceError, CommerceResult};
use crate::services::write_transaction::{
    ProductWriteTransaction, current_product_operation_id, record_product_operation_result,
};
use rustok_api::{PLATFORM_FALLBACK_LOCALE, normalize_locale_tag};
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
        let translations = normalize_category_translations(&input.translations)?;
        let category_id = current_product_operation_id().unwrap_or_else(generate_id);
        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;

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

        for translation in &translations {
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

        txn.publish(
            tenant_id,
            Some(actor_id),
            DomainEvent::CatalogCategoryCreated { category_id },
        )
        .await?;
        let result = CatalogCategoryRecord {
            id: category_id,
            code: input.code,
            slug: input.slug,
            path,
            kind: input.kind,
        };
        record_product_operation_result(&result)?;
        txn.commit().await?;
        Ok(result)
    }

    pub async fn list_categories(
        &self,
        tenant_id: Uuid,
        locale: &str,
    ) -> CommerceResult<Vec<CatalogCategoryListRecord>> {
        let locale = normalize_locale_tag(locale).ok_or_else(|| {
            CommerceError::Validation("category locale must be a valid locale tag".into())
        })?;
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
            LEFT JOIN LATERAL (
                SELECT translation.name
                FROM catalog_category_translations translation
                WHERE translation.category_id = c.id
                ORDER BY
                    CASE
                        WHEN translation.locale = $2 THEN 0
                        WHEN translation.locale = $3 THEN 1
                        ELSE 2
                    END,
                    translation.locale ASC,
                    translation.id ASC
                LIMIT 1
            ) t ON TRUE
            WHERE c.tenant_id = $1 AND c.deleted_at IS NULL
            ORDER BY c.path ASC
            "#,
            vec![
                tenant_id.into(),
                locale.into(),
                PLATFORM_FALLBACK_LOCALE.to_string().into(),
            ],
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
        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
        ensure_structural_category(&txn, tenant_id, input.category_id).await?;
        let group_id = current_product_operation_id().unwrap_or_else(generate_id);
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
        txn.publish(
            tenant_id,
            Some(actor_id),
            DomainEvent::CatalogCategoryAttributesChanged {
                category_id: input.category_id,
            },
        )
        .await?;
        let result = ProductAttributeGroupRecord {
            id: group_id,
            owner_id: input.category_id,
            code: input.code,
        };
        record_product_operation_result(&result)?;
        txn.commit().await?;
        Ok(result)
    }

    pub async fn set_category_schema_mode(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: SetCategorySchemaModeInput,
    ) -> CommerceResult<()> {
        input.validate()?;
        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
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

        txn.publish(
            tenant_id,
            Some(actor_id),
            DomainEvent::CatalogCategorySchemaModeChanged {
                category_id: input.category_id,
            },
        )
        .await?;
        record_product_operation_result(&())?;
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
        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
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

        txn.publish(
            tenant_id,
            Some(actor_id),
            DomainEvent::CatalogCategoryAttributesChanged {
                category_id: input.category_id,
            },
        )
        .await?;
        record_product_operation_result(&())?;
        txn.commit().await?;
        Ok(())
    }
}

fn normalize_category_translations(
    translations: &[CategoryTranslationInput],
) -> CommerceResult<Vec<CategoryTranslationInput>> {
    let mut seen_locales = HashSet::new();
    translations
        .iter()
        .map(|translation| {
            let locale = normalize_locale_tag(&translation.locale).ok_or_else(|| {
                CommerceError::Validation("category translation locale is invalid".into())
            })?;
            if !seen_locales.insert(locale.clone()) {
                return Err(CommerceError::Validation(format!(
                    "category translation locale {locale} occurs more than once after normalization"
                )));
            }

            let name = translation.name.trim();
            if name.is_empty() || name.chars().count() > 255 {
                return Err(CommerceError::Validation(
                    "category translation name must be 1..255 characters".into(),
                ));
            }
            if translation
                .meta_title
                .as_deref()
                .is_some_and(|value| value.chars().count() > 255)
            {
                return Err(CommerceError::Validation(
                    "category translation meta_title must not exceed 255 characters".into(),
                ));
            }
            if translation
                .meta_description
                .as_deref()
                .is_some_and(|value| value.chars().count() > 500)
            {
                return Err(CommerceError::Validation(
                    "category translation meta_description must not exceed 500 characters".into(),
                ));
            }

            Ok(CategoryTranslationInput {
                locale,
                name: name.to_string(),
                description: translation.description.clone(),
                meta_title: translation.meta_title.clone(),
                meta_description: translation.meta_description.clone(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn translation(locale: &str, name: &str) -> CategoryTranslationInput {
        CategoryTranslationInput {
            locale: locale.to_string(),
            name: name.to_string(),
            description: None,
            meta_title: None,
            meta_description: None,
        }
    }

    #[test]
    fn category_translations_are_canonicalized() {
        let normalized = normalize_category_translations(&[
            translation(" EN_us ", " English "),
            translation("fr_FR", "Français"),
        ])
        .expect("valid category translations");

        assert_eq!(normalized[0].locale, "en-US");
        assert_eq!(normalized[0].name, "English");
        assert_eq!(normalized[1].locale, "fr-FR");
    }

    #[test]
    fn category_translations_reject_normalized_locale_duplicates() {
        let error = normalize_category_translations(&[
            translation("en_us", "English"),
            translation("en-US", "English duplicate"),
        ])
        .expect_err("equivalent locale tags must not coexist");

        assert!(error.to_string().contains("occurs more than once"));
    }

    #[test]
    fn category_translations_reject_invalid_locale_and_empty_name() {
        assert!(normalize_category_translations(&[translation(" ", "English")]).is_err());
        assert!(normalize_category_translations(&[translation("en", "   ")]).is_err());
    }
}
