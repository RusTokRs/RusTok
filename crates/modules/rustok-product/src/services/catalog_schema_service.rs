use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sea_orm::{ConnectionTrait, DatabaseConnection, FromQueryResult, Statement};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::error::{CommerceError, CommerceResult};
use rustok_core::generate_id;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

mod attributes;
mod categories;
mod effective_forms;
mod schemas;
mod values;
mod virtual_categories;

pub(super) use virtual_categories::validate_virtual_category_rule_references;

use super::catalog_schema::{
    AttributeBinding, AttributeValueType, AttributeVisibilityOverrides, CatalogCategoryKind,
    CatalogCategorySchema, CategoryAttributeBinding, CategoryAttributeBindingKind,
    CategorySchemaMode, EffectiveAttributeSource, EffectiveProductForm, ProductAttributeSchema,
    SchemaResolutionError, parse_virtual_category_rule_v1, resolve_effective_product_form,
};
use super::write_transaction::ProductWriteTransaction;

#[derive(Clone)]
pub struct ProductCatalogSchemaService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl ProductCatalogSchemaService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttributeTranslationInput {
    pub locale: String,
    pub label: String,
    pub help_text: Option<String>,
    pub facet_label: Option<String>,
    pub seo_label: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateProductAttributeInput {
    pub code: String,
    pub value_type: AttributeValueType,
    pub scope: String,
    pub is_localized: bool,
    pub is_filterable: bool,
    pub is_searchable: bool,
    pub is_sortable: bool,
    pub is_comparable: bool,
    pub show_on_storefront: bool,
    pub show_in_admin_grid: bool,
    pub search_weight: i32,
    pub filter_display: Option<String>,
    pub facet_mode: Option<String>,
    pub position: i32,
    pub validation: Value,
    pub default_value: Option<Value>,
    pub metadata: Value,
    pub translations: Vec<AttributeTranslationInput>,
}

impl CreateProductAttributeInput {
    fn validate(&self) -> CommerceResult<()> {
        validate_code("attribute code", &self.code)?;
        validate_bounded_json_object("validation", &self.validation)?;
        if let Some(default_value) = &self.default_value {
            validate_bounded_json("default_value", default_value)?;
        }
        validate_bounded_json_object("metadata", &self.metadata)?;
        if !matches!(self.scope.as_str(), "product" | "variant" | "both") {
            return Err(CommerceError::Validation(
                "attribute scope must be product, variant, or both".into(),
            ));
        }
        if self.is_localized
            && !matches!(
                self.value_type,
                AttributeValueType::Text
                    | AttributeValueType::Textarea
                    | AttributeValueType::Richtext
            )
        {
            return Err(CommerceError::Validation(
                "only text, textarea, and richtext attributes can be localized".into(),
            ));
        }
        if self.translations.is_empty() {
            return Err(CommerceError::Validation(
                "attribute requires at least one translation".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProductAttributeRecord {
    pub id: Uuid,
    pub code: String,
    pub value_type: AttributeValueType,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProductAttributeListRecord {
    pub id: Uuid,
    pub code: String,
    pub value_type: AttributeValueType,
    pub is_localized: bool,
    pub is_filterable: bool,
    pub is_searchable: bool,
    pub is_sortable: bool,
    pub show_on_storefront: bool,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttributeOptionTranslationInput {
    pub locale: String,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateProductAttributeOptionInput {
    pub attribute_id: Uuid,
    pub code: String,
    pub position: i32,
    pub metadata: Value,
    pub translations: Vec<AttributeOptionTranslationInput>,
}

impl CreateProductAttributeOptionInput {
    fn validate(&self) -> CommerceResult<()> {
        validate_uuid("attribute_id", self.attribute_id)?;
        validate_code("attribute option code", &self.code)?;
        validate_bounded_json_object("metadata", &self.metadata)?;
        if self.translations.is_empty() {
            return Err(CommerceError::Validation(
                "attribute option requires at least one translation".into(),
            ));
        }
        for translation in &self.translations {
            validate_locale(&translation.locale)?;
            if translation.label.trim().is_empty() {
                return Err(CommerceError::Validation(
                    "attribute option label must not be empty".into(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProductAttributeOptionRecord {
    pub id: Uuid,
    pub attribute_id: Uuid,
    pub code: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProductAttributeOptionListRecord {
    pub id: Uuid,
    pub attribute_id: Uuid,
    pub code: String,
    pub label: String,
    pub position: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CategoryTranslationInput {
    pub locale: String,
    pub name: String,
    pub description: Option<String>,
    pub meta_title: Option<String>,
    pub meta_description: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateCatalogCategoryInput {
    pub parent_id: Option<Uuid>,
    pub code: String,
    pub slug: String,
    pub kind: CatalogCategoryKind,
    pub position: i32,
    pub rule_config: Value,
    pub metadata: Value,
    pub translations: Vec<CategoryTranslationInput>,
}

impl CreateCatalogCategoryInput {
    fn validate(&self) -> CommerceResult<()> {
        validate_code("category code", &self.code)?;
        validate_slug("category slug", &self.slug)?;
        validate_bounded_json_object("metadata", &self.metadata)?;
        validate_bounded_json_object("rule_config", &self.rule_config)?;
        if self.translations.is_empty() {
            return Err(CommerceError::Validation(
                "category requires at least one translation".into(),
            ));
        }
        match self.kind {
            CatalogCategoryKind::Virtual => {
                parse_virtual_category_rule_v1(&self.rule_config)
                    .map_err(CommerceError::Validation)?;
            }
            CatalogCategoryKind::Structural | CatalogCategoryKind::Collection
                if !self.rule_config.is_null()
                    && self
                        .rule_config
                        .as_object()
                        .is_none_or(|config| !config.is_empty()) =>
            {
                return Err(CommerceError::Validation(
                    "rule_config is only allowed for virtual categories".into(),
                ));
            }
            _ => {}
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogCategoryRecord {
    pub id: Uuid,
    pub code: String,
    pub slug: String,
    pub path: String,
    pub kind: CatalogCategoryKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogCategoryListRecord {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub code: String,
    pub slug: String,
    pub path: String,
    pub kind: CatalogCategoryKind,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SchemaTranslationInput {
    pub locale: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateProductAttributeSchemaInput {
    pub code: String,
    pub metadata: Value,
    pub translations: Vec<SchemaTranslationInput>,
}

impl CreateProductAttributeSchemaInput {
    fn validate(&self) -> CommerceResult<()> {
        validate_code("schema code", &self.code)?;
        validate_bounded_json_object("metadata", &self.metadata)?;
        if self.translations.is_empty() {
            return Err(CommerceError::Validation(
                "schema requires at least one translation".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProductAttributeSchemaRecord {
    pub id: Uuid,
    pub code: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProductAttributeSchemaListRecord {
    pub id: Uuid,
    pub code: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttributeGroupTranslationInput {
    pub locale: String,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateProductAttributeSchemaGroupInput {
    pub schema_id: Uuid,
    pub code: String,
    pub position: i32,
    pub metadata: Value,
    pub translations: Vec<AttributeGroupTranslationInput>,
}

impl CreateProductAttributeSchemaGroupInput {
    fn validate(&self) -> CommerceResult<()> {
        validate_uuid("schema_id", self.schema_id)?;
        validate_code("group code", &self.code)?;
        validate_bounded_json_object("metadata", &self.metadata)?;
        validate_group_translations(&self.translations)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateCategoryAttributeGroupInput {
    pub category_id: Uuid,
    pub code: String,
    pub position: i32,
    pub metadata: Value,
    pub translations: Vec<AttributeGroupTranslationInput>,
}

impl CreateCategoryAttributeGroupInput {
    fn validate(&self) -> CommerceResult<()> {
        validate_uuid("category_id", self.category_id)?;
        validate_code("group code", &self.code)?;
        validate_bounded_json_object("metadata", &self.metadata)?;
        validate_group_translations(&self.translations)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProductAttributeGroupRecord {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub code: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetCategorySchemaModeInput {
    pub category_id: Uuid,
    pub mode: CategorySchemaMode,
    pub schema_id: Option<Uuid>,
    pub clone_from_category_id: Option<Uuid>,
}

impl SetCategorySchemaModeInput {
    fn validate(&self) -> CommerceResult<()> {
        match self.mode {
            CategorySchemaMode::UseSchema if self.schema_id.is_none() => {
                Err(CommerceError::Validation("schema_id is required".into()))
            }
            CategorySchemaMode::CloneFromCategory if self.clone_from_category_id.is_none() => Err(
                CommerceError::Validation("clone_from_category_id is required".into()),
            ),
            _ => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BindSchemaAttributeInput {
    pub schema_id: Uuid,
    pub attribute_id: Uuid,
    pub group_code: Option<String>,
    pub is_required: bool,
    pub is_disabled: bool,
    pub position: i32,
    pub visibility_overrides: Value,
    pub validation_overrides: Value,
    pub metadata: Value,
}

impl BindSchemaAttributeInput {
    fn validate(&self) -> CommerceResult<()> {
        validate_uuid("schema_id", self.schema_id)?;
        validate_uuid("attribute_id", self.attribute_id)?;
        if let Some(group_code) = self.group_code.as_deref() {
            validate_code("group_code", group_code)?;
        }
        parse_visibility_overrides(self.visibility_overrides.clone())?;
        validate_bounded_json_object("visibility_overrides", &self.visibility_overrides)?;
        validate_override_object("validation_overrides", &self.validation_overrides)?;
        validate_bounded_json_object("metadata", &self.metadata)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BindCategoryAttributeInput {
    pub category_id: Uuid,
    pub attribute_id: Uuid,
    pub group_code: Option<String>,
    pub binding_kind: CategoryAttributeBindingKind,
    pub is_required: Option<bool>,
    pub is_disabled: bool,
    pub position: Option<i32>,
    pub visibility_overrides: Value,
    pub validation_overrides: Value,
    pub metadata: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ProductAttributeValuePatchValue {
    Clear,
    Text(String),
    Integer(i64),
    Decimal(Decimal),
    Boolean(bool),
    Date(NaiveDate),
    Datetime(DateTime<Utc>),
    Select(Uuid),
    Multiselect(Vec<Uuid>),
    Json(Value),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProductAttributeValuePatch {
    pub attribute_id: Uuid,
    pub value: ProductAttributeValuePatchValue,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ProductAttributeValue {
    Text(String),
    Integer(i64),
    Decimal(Decimal),
    Boolean(bool),
    Date(NaiveDate),
    Datetime(DateTime<Utc>),
    Select(Uuid),
    Multiselect(Vec<Uuid>),
    Json(Value),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProductAttributeValueRecord {
    pub attribute_id: Uuid,
    pub value: Option<ProductAttributeValue>,
    pub detached: bool,
}

impl BindCategoryAttributeInput {
    fn validate(&self) -> CommerceResult<()> {
        validate_uuid("category_id", self.category_id)?;
        validate_uuid("attribute_id", self.attribute_id)?;
        if let Some(group_code) = self.group_code.as_deref() {
            validate_code("group_code", group_code)?;
        }
        parse_visibility_overrides(self.visibility_overrides.clone())?;
        validate_bounded_json_object("visibility_overrides", &self.visibility_overrides)?;
        validate_override_object("validation_overrides", &self.validation_overrides)?;
        validate_bounded_json_object("metadata", &self.metadata)?;
        Ok(())
    }
}

#[derive(FromQueryResult)]
struct CategoryParentRow {
    path: String,
    level: i32,
}

#[derive(FromQueryResult)]
struct ProductPrimaryCategoryRow {
    primary_category_id: Option<Uuid>,
}

#[derive(FromQueryResult)]
struct AttributeIdRow {
    attribute_id: Uuid,
}

#[derive(FromQueryResult)]
struct ProductAttributeWriteDefinitionRow {
    id: Uuid,
    value_type: String,
    scope: String,
    is_localized: bool,
}

#[derive(FromQueryResult)]
struct ProductAttributeOptionWriteRow {
    id: Uuid,
    attribute_id: Uuid,
}

#[derive(FromQueryResult)]
struct ProductAttributeValueRow {
    id: Uuid,
    attribute_id: Uuid,
    value_type: String,
    is_localized: bool,
    value_text: Option<String>,
    value_integer: Option<i64>,
    value_decimal: Option<Decimal>,
    value_boolean: Option<bool>,
    value_date: Option<NaiveDate>,
    value_datetime: Option<DateTime<Utc>>,
    value_json: Option<Value>,
    detached: bool,
    localized_value_text: Option<String>,
}

impl ProductAttributeValueRow {
    fn into_record(self, option_ids: Vec<Uuid>) -> CommerceResult<ProductAttributeValueRecord> {
        let value_type = AttributeValueType::from_storage(&self.value_type)
            .map_err(map_schema_resolution_error)?;
        let missing = || {
            CommerceError::Validation(format!(
                "stored value for attribute {} does not match type {}",
                self.attribute_id,
                value_type.as_str()
            ))
        };
        let value = match value_type {
            AttributeValueType::Text
            | AttributeValueType::Textarea
            | AttributeValueType::Richtext
                if self.is_localized =>
            {
                self.localized_value_text.map(ProductAttributeValue::Text)
            }
            AttributeValueType::Text
            | AttributeValueType::Textarea
            | AttributeValueType::Richtext => Some(ProductAttributeValue::Text(
                self.value_text.ok_or_else(missing)?,
            )),
            AttributeValueType::Integer => Some(ProductAttributeValue::Integer(
                self.value_integer.ok_or_else(missing)?,
            )),
            AttributeValueType::Decimal => Some(ProductAttributeValue::Decimal(
                self.value_decimal.ok_or_else(missing)?,
            )),
            AttributeValueType::Boolean => Some(ProductAttributeValue::Boolean(
                self.value_boolean.ok_or_else(missing)?,
            )),
            AttributeValueType::Date => Some(ProductAttributeValue::Date(
                self.value_date.ok_or_else(missing)?,
            )),
            AttributeValueType::Datetime => Some(ProductAttributeValue::Datetime(
                self.value_datetime.ok_or_else(missing)?,
            )),
            AttributeValueType::Select => {
                if option_ids.len() != 1 {
                    return Err(missing());
                }
                Some(ProductAttributeValue::Select(option_ids[0]))
            }
            AttributeValueType::Multiselect => Some(ProductAttributeValue::Multiselect(option_ids)),
            AttributeValueType::Json => Some(ProductAttributeValue::Json(
                self.value_json.ok_or_else(missing)?,
            )),
        };
        Ok(ProductAttributeValueRecord {
            attribute_id: self.attribute_id,
            value,
            detached: self.detached,
        })
    }
}

#[derive(FromQueryResult)]
struct ProductAttributeValueOptionRow {
    value_id: Uuid,
    option_id: Uuid,
}

#[derive(FromQueryResult)]
struct ProductPublishRequirementRow {
    attribute_id: Uuid,
    code: String,
    value_type: String,
    is_localized: bool,
    value_text: Option<String>,
    value_integer: Option<i64>,
    value_decimal: Option<Decimal>,
    value_boolean: Option<bool>,
    value_date: Option<NaiveDate>,
    value_datetime: Option<DateTime<Utc>>,
    value_json: Option<Value>,
    has_option: bool,
    has_localized_text: bool,
}

impl ProductPublishRequirementRow {
    fn is_filled(&self) -> CommerceResult<bool> {
        let value_type = AttributeValueType::from_storage(&self.value_type)
            .map_err(map_schema_resolution_error)?;
        let filled = match value_type {
            AttributeValueType::Text
            | AttributeValueType::Textarea
            | AttributeValueType::Richtext
                if self.is_localized =>
            {
                self.has_localized_text
            }
            AttributeValueType::Text
            | AttributeValueType::Textarea
            | AttributeValueType::Richtext => self
                .value_text
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()),
            AttributeValueType::Integer => self.value_integer.is_some(),
            AttributeValueType::Decimal => self.value_decimal.is_some(),
            AttributeValueType::Boolean => self.value_boolean.is_some(),
            AttributeValueType::Date => self.value_date.is_some(),
            AttributeValueType::Datetime => self.value_datetime.is_some(),
            AttributeValueType::Select | AttributeValueType::Multiselect => self.has_option,
            AttributeValueType::Json => self.value_json.is_some(),
        };
        Ok(filled)
    }
}

#[derive(FromQueryResult)]
struct IdRow {
    id: Uuid,
}

#[derive(FromQueryResult)]
struct ProductAttributeListRow {
    id: Uuid,
    code: String,
    value_type: String,
    is_localized: bool,
    is_filterable: bool,
    is_searchable: bool,
    is_sortable: bool,
    show_on_storefront: bool,
    label: String,
}

impl TryFrom<ProductAttributeListRow> for ProductAttributeListRecord {
    type Error = CommerceError;

    fn try_from(row: ProductAttributeListRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            code: row.code,
            value_type: AttributeValueType::from_storage(&row.value_type)
                .map_err(map_schema_resolution_error)?,
            is_localized: row.is_localized,
            is_filterable: row.is_filterable,
            is_searchable: row.is_searchable,
            is_sortable: row.is_sortable,
            show_on_storefront: row.show_on_storefront,
            label: row.label,
        })
    }
}

#[derive(FromQueryResult)]
struct ProductAttributeOptionListRow {
    id: Uuid,
    attribute_id: Uuid,
    code: String,
    label: String,
    position: i32,
}

impl From<ProductAttributeOptionListRow> for ProductAttributeOptionListRecord {
    fn from(row: ProductAttributeOptionListRow) -> Self {
        Self {
            id: row.id,
            attribute_id: row.attribute_id,
            code: row.code,
            label: row.label,
            position: row.position,
        }
    }
}

#[derive(FromQueryResult)]
struct CatalogCategoryListRow {
    id: Uuid,
    parent_id: Option<Uuid>,
    code: String,
    slug: String,
    path: String,
    kind: String,
    name: String,
}

impl TryFrom<CatalogCategoryListRow> for CatalogCategoryListRecord {
    type Error = CommerceError;

    fn try_from(row: CatalogCategoryListRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            parent_id: row.parent_id,
            code: row.code,
            slug: row.slug,
            path: row.path,
            kind: CatalogCategoryKind::from_storage(&row.kind)
                .map_err(map_schema_resolution_error)?,
            name: row.name,
        })
    }
}

#[derive(FromQueryResult)]
struct ProductAttributeSchemaListRow {
    id: Uuid,
    code: String,
    name: String,
}

impl From<ProductAttributeSchemaListRow> for ProductAttributeSchemaListRecord {
    fn from(row: ProductAttributeSchemaListRow) -> Self {
        Self {
            id: row.id,
            code: row.code,
            name: row.name,
        }
    }
}

#[derive(FromQueryResult)]
struct CategorySchemaRow {
    category_id: Uuid,
    parent_category_id: Option<Uuid>,
    kind: String,
    mode: String,
    schema_id: Option<Uuid>,
    snapshot: Value,
}

#[derive(FromQueryResult)]
struct CategoryAttributeRow {
    category_id: Uuid,
    attribute_id: Uuid,
    group_code: Option<String>,
    binding_kind: String,
    is_required: Option<bool>,
    is_disabled: bool,
    position: Option<i32>,
    visibility_overrides: Value,
    validation_overrides: Value,
}

#[derive(FromQueryResult)]
struct CategoryAncestorRow {
    category_id: Uuid,
}

#[derive(FromQueryResult)]
struct EffectiveGroupLabelRow {
    owner_id: Uuid,
    code: String,
    label: String,
}

#[derive(FromQueryResult)]
struct SchemaRow {
    id: Uuid,
    code: String,
}

#[derive(FromQueryResult)]
struct SchemaAttributeRow {
    schema_id: Uuid,
    attribute_id: Uuid,
    group_code: Option<String>,
    is_required: bool,
    is_disabled: bool,
    position: i32,
    visibility_overrides: Value,
    validation_overrides: Value,
}

fn parse_visibility_overrides(value: Value) -> CommerceResult<AttributeVisibilityOverrides> {
    serde_json::from_value(value).map_err(|error| {
        CommerceError::Validation(format!("invalid attribute visibility overrides: {error}"))
    })
}

fn validate_override_object(field: &str, value: &Value) -> CommerceResult<()> {
    if value.is_object() {
        Ok(())
    } else {
        Err(CommerceError::Validation(format!(
            "{field} must be a JSON object"
        )))
    }
}

const MAX_PRODUCT_JSON_BYTES: usize = 64 * 1024;
const MAX_PRODUCT_JSON_DEPTH: usize = 32;

fn validate_bounded_json_object(field: &str, value: &Value) -> CommerceResult<()> {
    if !value.is_object() {
        return Err(CommerceError::Validation(format!(
            "{field} must be a JSON object"
        )));
    }
    validate_bounded_json(field, value)
}

fn validate_bounded_json(field: &str, value: &Value) -> CommerceResult<()> {
    let serialized = serde_json::to_vec(value).map_err(|error| {
        CommerceError::Validation(format!("{field} is not serializable: {error}"))
    })?;
    if serialized.len() > MAX_PRODUCT_JSON_BYTES {
        return Err(CommerceError::Validation(format!(
            "{field} must not exceed {MAX_PRODUCT_JSON_BYTES} bytes"
        )));
    }
    if json_depth(value) > MAX_PRODUCT_JSON_DEPTH {
        return Err(CommerceError::Validation(format!(
            "{field} must not exceed {MAX_PRODUCT_JSON_DEPTH} nesting levels"
        )));
    }
    Ok(())
}

fn json_depth(value: &Value) -> usize {
    match value {
        Value::Array(items) => 1 + items.iter().map(json_depth).max().unwrap_or(0),
        Value::Object(values) => 1 + values.values().map(json_depth).max().unwrap_or(0),
        _ => 0,
    }
}

async fn load_category_parent<C>(
    conn: &C,
    tenant_id: Uuid,
    category_id: Uuid,
) -> CommerceResult<CategoryParentRow>
where
    C: ConnectionTrait,
{
    CategoryParentRow::find_by_statement(Statement::from_sql_and_values(
        conn.get_database_backend(),
        "SELECT path, level FROM catalog_categories WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        vec![tenant_id.into(), category_id.into()],
    ))
    .one(conn)
    .await?
    .ok_or_else(|| CommerceError::Validation("parent category not found".into()))
}

async fn ensure_structural_category<C>(
    conn: &C,
    tenant_id: Uuid,
    category_id: Uuid,
) -> CommerceResult<()>
where
    C: ConnectionTrait,
{
    #[derive(FromQueryResult)]
    struct Row {
        kind: String,
    }

    let row = Row::find_by_statement(Statement::from_sql_and_values(
        conn.get_database_backend(),
        "SELECT kind FROM catalog_categories WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        vec![tenant_id.into(), category_id.into()],
    ))
    .one(conn)
    .await?
    .ok_or_else(|| CommerceError::Validation("category not found".into()))?;

    if row.kind != CatalogCategoryKind::Structural.as_str() {
        return Err(CommerceError::Validation(
            "only structural categories can define product forms".into(),
        ));
    }
    Ok(())
}

async fn ensure_attribute<C>(conn: &C, tenant_id: Uuid, attribute_id: Uuid) -> CommerceResult<()>
where
    C: ConnectionTrait,
{
    #[derive(FromQueryResult)]
    struct Row {
        _id: Uuid,
    }

    let found = Row::find_by_statement(Statement::from_sql_and_values(
        conn.get_database_backend(),
        "SELECT id AS _id FROM product_attributes WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL",
        vec![tenant_id.into(), attribute_id.into()],
    ))
    .one(conn)
    .await?;

    if found.is_none() {
        return Err(CommerceError::Validation("attribute not found".into()));
    }
    Ok(())
}

async fn load_schema_group_id<C>(
    conn: &C,
    tenant_id: Uuid,
    schema_id: Uuid,
    code: &str,
) -> CommerceResult<Option<Uuid>>
where
    C: ConnectionTrait,
{
    Ok(IdRow::find_by_statement(Statement::from_sql_and_values(
        conn.get_database_backend(),
        r#"
        SELECT id
        FROM product_attribute_schema_groups
        WHERE tenant_id = $1 AND schema_id = $2 AND code = $3
        "#,
        vec![tenant_id.into(), schema_id.into(), code.to_string().into()],
    ))
    .one(conn)
    .await?
    .map(|row| row.id))
}

async fn insert_schema_group_translation<C>(
    conn: &C,
    group_id: Uuid,
    translation: &AttributeGroupTranslationInput,
) -> CommerceResult<()>
where
    C: ConnectionTrait,
{
    conn.execute_raw(Statement::from_sql_and_values(
        conn.get_database_backend(),
        r#"
        INSERT INTO product_attribute_schema_group_translations (
            id, group_id, locale, label
        ) VALUES ($1, $2, $3, $4)
        "#,
        vec![
            generate_id().into(),
            group_id.into(),
            translation.locale.clone().into(),
            translation.label.clone().into(),
        ],
    ))
    .await?;
    Ok(())
}

async fn insert_category_group_translation<C>(
    conn: &C,
    group_id: Uuid,
    translation: &AttributeGroupTranslationInput,
) -> CommerceResult<()>
where
    C: ConnectionTrait,
{
    conn.execute_raw(Statement::from_sql_and_values(
        conn.get_database_backend(),
        r#"
        INSERT INTO category_attribute_group_translations (
            id, group_id, locale, label
        ) VALUES ($1, $2, $3, $4)
        "#,
        vec![
            generate_id().into(),
            group_id.into(),
            translation.locale.clone().into(),
            translation.label.clone().into(),
        ],
    ))
    .await?;
    Ok(())
}

async fn load_category_group_id<C>(
    conn: &C,
    tenant_id: Uuid,
    category_id: Uuid,
    code: &str,
) -> CommerceResult<Option<Uuid>>
where
    C: ConnectionTrait,
{
    Ok(IdRow::find_by_statement(Statement::from_sql_and_values(
        conn.get_database_backend(),
        r#"
        SELECT id
        FROM category_attribute_groups
        WHERE tenant_id = $1 AND category_id = $2 AND code = $3
        "#,
        vec![
            tenant_id.into(),
            category_id.into(),
            code.to_string().into(),
        ],
    ))
    .one(conn)
    .await?
    .map(|row| row.id))
}

async fn load_attribute_write_definition<C>(
    conn: &C,
    tenant_id: Uuid,
    attribute_id: Uuid,
) -> CommerceResult<ProductAttributeWriteDefinitionRow>
where
    C: ConnectionTrait,
{
    ProductAttributeWriteDefinitionRow::find_by_statement(Statement::from_sql_and_values(
        conn.get_database_backend(),
        r#"
        SELECT id, value_type, scope, is_localized
        FROM product_attributes
        WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL
        "#,
        vec![tenant_id.into(), attribute_id.into()],
    ))
    .one(conn)
    .await?
    .ok_or_else(|| CommerceError::Validation("attribute not found".into()))
}

async fn load_attribute_codes<C>(
    conn: &C,
    tenant_id: Uuid,
    attribute_ids: &[Uuid],
) -> CommerceResult<Vec<String>>
where
    C: ConnectionTrait,
{
    if attribute_ids.is_empty() {
        return Ok(Vec::new());
    }
    let (placeholders, values) = uuid_filter_values(tenant_id, attribute_ids);
    let codes_by_id = ProductAttributeCodeRow::find_by_statement(Statement::from_sql_and_values(
        conn.get_database_backend(),
        format!(
            r#"
            SELECT id, code
            FROM product_attributes
            WHERE tenant_id = $1
              AND archived_at IS NULL
              AND id IN ({placeholders})
            ORDER BY code ASC
            "#
        ),
        values,
    ))
    .all(conn)
    .await?
    .into_iter()
    .map(|row| (row.id, row.code))
    .collect::<HashMap<_, _>>();
    Ok(attribute_ids
        .iter()
        .map(|attribute_id| {
            codes_by_id
                .get(attribute_id)
                .cloned()
                .unwrap_or_else(|| attribute_id.to_string())
        })
        .collect())
}

#[derive(FromQueryResult)]
struct ProductAttributeCodeRow {
    id: Uuid,
    code: String,
}

async fn ensure_schema<C>(conn: &C, tenant_id: Uuid, schema_id: Uuid) -> CommerceResult<()>
where
    C: ConnectionTrait,
{
    #[derive(FromQueryResult)]
    struct Row {
        _id: Uuid,
    }

    let found = Row::find_by_statement(Statement::from_sql_and_values(
        conn.get_database_backend(),
        "SELECT id AS _id FROM product_attribute_schemas WHERE tenant_id = $1 AND id = $2 AND archived_at IS NULL",
        vec![tenant_id.into(), schema_id.into()],
    ))
    .one(conn)
    .await?;

    if found.is_none() {
        return Err(CommerceError::Validation(
            "attribute schema not found".into(),
        ));
    }
    Ok(())
}

async fn load_product_primary_category<C>(
    conn: &C,
    tenant_id: Uuid,
    product_id: Uuid,
) -> CommerceResult<ProductPrimaryCategoryRow>
where
    C: ConnectionTrait,
{
    ProductPrimaryCategoryRow::find_by_statement(Statement::from_sql_and_values(
        conn.get_database_backend(),
        "SELECT primary_category_id FROM products WHERE tenant_id = $1 AND id = $2 AND deleted_at IS NULL",
        vec![tenant_id.into(), product_id.into()],
    ))
    .one(conn)
    .await?
    .ok_or(CommerceError::ProductNotFound(product_id))
}

async fn ensure_product<C>(conn: &C, tenant_id: Uuid, product_id: Uuid) -> CommerceResult<()>
where
    C: ConnectionTrait,
{
    load_product_primary_category(conn, tenant_id, product_id)
        .await
        .map(|_| ())
}

fn validate_product_value_patch(
    definition: &ProductAttributeWriteDefinitionRow,
    patch: &ProductAttributeValuePatch,
    options: &HashMap<Uuid, Uuid>,
) -> CommerceResult<()> {
    if !matches!(definition.scope.as_str(), "product" | "both") {
        return Err(CommerceError::Validation(format!(
            "attribute {} is variant-only",
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

async fn write_product_value_patch<C>(
    conn: &C,
    tenant_id: Uuid,
    product_id: Uuid,
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
            "DELETE FROM product_attribute_values WHERE tenant_id = $1 AND product_id = $2 AND attribute_id = $3",
            vec![tenant_id.into(), product_id.into(), patch.attribute_id.into()],
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
        INSERT INTO product_attribute_values (
            id, tenant_id, product_id, attribute_id, value_text, value_integer,
            value_decimal, value_boolean, value_date, value_datetime, value_json,
            detached_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, NULL)
        ON CONFLICT (tenant_id, product_id, attribute_id) DO UPDATE SET
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
            product_id.into(),
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
        "DELETE FROM product_attribute_value_options WHERE value_id = $1",
        vec![value_id.into()],
    ))
    .await?;
    for option_id in option_ids {
        conn.execute_raw(Statement::from_sql_and_values(
            conn.get_database_backend(),
            "INSERT INTO product_attribute_value_options (tenant_id, value_id, option_id) VALUES ($1, $2, $3)",
            vec![tenant_id.into(), value_id.into(), option_id.into()],
        ))
        .await?;
    }

    if let Some(value) = localized_text {
        conn.execute_raw(Statement::from_sql_and_values(
            conn.get_database_backend(),
            r#"
            INSERT INTO product_attribute_value_translations (id, value_id, locale, value_text)
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

fn validate_locale(locale: &str) -> CommerceResult<()> {
    let locale = locale.trim();
    if locale.is_empty() || locale.len() > 32 {
        return Err(CommerceError::Validation(
            "locale must be 1..32 characters".into(),
        ));
    }
    Ok(())
}

fn uuid_filter_values(tenant_id: Uuid, ids: &[Uuid]) -> (String, Vec<sea_orm::Value>) {
    let placeholders = (0..ids.len())
        .map(|index| format!("${}", index + 2))
        .collect::<Vec<_>>()
        .join(", ");
    let mut values = Vec::with_capacity(ids.len() + 1);
    values.push(tenant_id.into());
    values.extend(ids.iter().copied().map(Into::into));
    (placeholders, values)
}

fn validate_code(field: &str, value: &str) -> CommerceResult<()> {
    if value.is_empty() || value.len() > 128 {
        return Err(CommerceError::Validation(format!(
            "{field} must be 1..128 characters"
        )));
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
    {
        return Err(CommerceError::Validation(format!(
            "{field} must use lowercase ascii letters, digits, underscore or dash"
        )));
    }
    Ok(())
}

fn validate_slug(field: &str, value: &str) -> CommerceResult<()> {
    if value.is_empty() || value.len() > 255 {
        return Err(CommerceError::Validation(format!(
            "{field} must be 1..255 characters"
        )));
    }
    if value.contains('/') {
        return Err(CommerceError::Validation(format!(
            "{field} must not contain slash"
        )));
    }
    Ok(())
}

fn validate_uuid(field: &str, value: Uuid) -> CommerceResult<()> {
    if value.is_nil() {
        return Err(CommerceError::Validation(format!(
            "{field} must not be nil"
        )));
    }
    Ok(())
}

fn validate_group_translations(
    translations: &[AttributeGroupTranslationInput],
) -> CommerceResult<()> {
    if translations.is_empty() {
        return Err(CommerceError::Validation(
            "attribute group requires at least one translation".into(),
        ));
    }
    for translation in translations {
        validate_locale(&translation.locale)?;
        if translation.label.trim().is_empty() || translation.label.len() > 255 {
            return Err(CommerceError::Validation(
                "attribute group label must be 1..255 characters".into(),
            ));
        }
    }
    Ok(())
}

fn map_schema_resolution_error(error: SchemaResolutionError) -> CommerceError {
    CommerceError::Validation(format!("schema resolution failed: {error:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_json_rejects_excessive_size_and_depth() {
        let oversized = Value::String("x".repeat(MAX_PRODUCT_JSON_BYTES + 1));
        assert!(validate_bounded_json("metadata", &oversized).is_err());

        let mut nested = Value::Null;
        for _ in 0..=MAX_PRODUCT_JSON_DEPTH {
            nested = serde_json::json!({ "nested": nested });
        }
        assert!(validate_bounded_json("metadata", &nested).is_err());
    }

    #[test]
    fn bounded_json_object_requires_an_object() {
        assert!(validate_bounded_json_object("metadata", &Value::Array(Vec::new())).is_err());
        assert!(validate_bounded_json_object("metadata", &serde_json::json!({})).is_ok());
    }
}
