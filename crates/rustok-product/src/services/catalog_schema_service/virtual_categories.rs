use super::super::catalog_schema::{VirtualCategoryAttributeCondition, VirtualCategoryRuleV1};
use super::{AttributeValueType, CommerceError, CommerceResult, map_schema_resolution_error};
use sea_orm::{ConnectionTrait, DatabaseTransaction, FromQueryResult, Statement};
use uuid::Uuid;

pub(crate) async fn validate_virtual_category_rule_references(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    rule: &VirtualCategoryRuleV1,
) -> CommerceResult<()> {
    #[derive(FromQueryResult)]
    struct AttributeRuleDefinitionRow {
        value_type: String,
        scope: String,
        is_localized: bool,
    }

    if let Some(category_id) = rule.primary_category_subtree_id {
        let category_exists = txn
            .query_one_raw(Statement::from_sql_and_values(
                txn.get_database_backend(),
                r#"
                SELECT 1
                FROM catalog_categories
                WHERE tenant_id = $1 AND id = $2 AND kind = 'structural'
                  AND is_active = TRUE AND deleted_at IS NULL
                "#,
                vec![tenant_id.into(), category_id.into()],
            ))
            .await?
            .is_some();
        if !category_exists {
            return Err(CommerceError::Validation(format!(
                "virtual category subtree {} is not an active structural category",
                category_id
            )));
        }
    }

    for attribute in &rule.attributes {
        let definition =
            AttributeRuleDefinitionRow::find_by_statement(Statement::from_sql_and_values(
                txn.get_database_backend(),
                r#"
                SELECT value_type, scope, is_localized
                FROM product_attributes
                WHERE tenant_id = $1 AND code = $2 AND archived_at IS NULL
                "#,
                vec![tenant_id.into(), attribute.code.trim().into()],
            ))
            .one(txn)
            .await?
            .ok_or_else(|| {
                CommerceError::Validation(format!(
                    "virtual category attribute {} does not exist",
                    attribute.code
                ))
            })?;
        if definition.scope == "variant" {
            return Err(CommerceError::Validation(format!(
                "virtual category attribute {} must support product scope",
                attribute.code
            )));
        }
        if definition.is_localized {
            return Err(CommerceError::Validation(format!(
                "localized attribute {} cannot be used by locale-neutral virtual category rules",
                attribute.code
            )));
        }
        let value_type = AttributeValueType::from_storage(&definition.value_type)
            .map_err(map_schema_resolution_error)?;
        match &attribute.condition {
            VirtualCategoryAttributeCondition::Range { .. }
                if !matches!(
                    value_type,
                    AttributeValueType::Integer | AttributeValueType::Decimal
                ) =>
            {
                return Err(CommerceError::Validation(format!(
                    "virtual category range attribute {} must be integer or decimal",
                    attribute.code
                )));
            }
            VirtualCategoryAttributeCondition::Eq { .. }
                if value_type == AttributeValueType::Json =>
            {
                return Err(CommerceError::Validation(format!(
                    "JSON attribute {} cannot be used by virtual category V1 rules",
                    attribute.code
                )));
            }
            _ => {}
        }
    }
    Ok(())
}
