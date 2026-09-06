use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VirtualCategoryRuleV1 {
    pub version: u8,
    #[serde(default)]
    pub statuses: Vec<String>,
    pub primary_category_subtree_id: Option<Uuid>,
    pub price_min: Option<i64>,
    pub price_max: Option<i64>,
    pub in_stock: Option<bool>,
    #[serde(default)]
    pub attributes: Vec<VirtualCategoryAttributeRule>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VirtualCategoryAttributeRule {
    pub code: String,
    #[serde(flatten)]
    pub condition: VirtualCategoryAttributeCondition,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operator", rename_all = "snake_case")]
pub enum VirtualCategoryAttributeCondition {
    Eq {
        value: String,
    },
    Range {
        min: Option<Decimal>,
        max: Option<Decimal>,
    },
}

pub fn parse_virtual_category_rule_v1(value: &Value) -> Result<VirtualCategoryRuleV1, String> {
    let rule: VirtualCategoryRuleV1 = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid virtual category V1 rule: {error}"))?;
    if rule.version != 1 {
        return Err("virtual category rule version must be 1".into());
    }
    if rule.statuses.is_empty()
        && rule.primary_category_subtree_id.is_none()
        && rule.price_min.is_none()
        && rule.price_max.is_none()
        && rule.in_stock.is_none()
        && rule.attributes.is_empty()
    {
        return Err("virtual category rule must contain at least one predicate".into());
    }
    if rule
        .statuses
        .iter()
        .any(|status| !matches!(status.as_str(), "draft" | "active" | "archived"))
    {
        return Err("virtual category statuses must be draft, active, or archived".into());
    }
    if rule
        .price_min
        .zip(rule.price_max)
        .is_some_and(|(min, max)| min > max)
    {
        return Err("virtual category price_min must not exceed price_max".into());
    }
    let mut attribute_codes = HashSet::new();
    for attribute in &rule.attributes {
        let code = attribute.code.trim();
        if code.is_empty() || code.len() > 128 {
            return Err("virtual category attribute code must contain 1..128 characters".into());
        }
        if !attribute_codes.insert(code) {
            return Err(format!(
                "virtual category attribute {} occurs more than once",
                attribute.code
            ));
        }
        match &attribute.condition {
            VirtualCategoryAttributeCondition::Eq { value } if value.trim().is_empty() => {
                return Err(format!(
                    "virtual category attribute {} equality value must not be empty",
                    attribute.code
                ));
            }
            VirtualCategoryAttributeCondition::Range {
                min: None,
                max: None,
            } => {
                return Err(format!(
                    "virtual category attribute {} range requires min or max",
                    attribute.code
                ));
            }
            VirtualCategoryAttributeCondition::Range {
                min: Some(min),
                max: Some(max),
            } if min > max => {
                return Err(format!(
                    "virtual category attribute {} min must not exceed max",
                    attribute.code
                ));
            }
            _ => {}
        }
    }
    Ok(rule)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogCategoryKind {
    Structural,
    Collection,
    Virtual,
}

impl CatalogCategoryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Structural => "structural",
            Self::Collection => "collection",
            Self::Virtual => "virtual",
        }
    }

    pub fn from_storage(value: &str) -> Result<Self, SchemaResolutionError> {
        match value {
            "structural" => Ok(Self::Structural),
            "collection" => Ok(Self::Collection),
            "virtual" => Ok(Self::Virtual),
            _ => Err(SchemaResolutionError::InvalidStorageValue {
                field: "catalog_categories.kind",
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributeValueType {
    Text,
    Textarea,
    Richtext,
    Integer,
    Decimal,
    Boolean,
    Date,
    Datetime,
    Select,
    Multiselect,
    Json,
}

impl AttributeValueType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Textarea => "textarea",
            Self::Richtext => "richtext",
            Self::Integer => "integer",
            Self::Decimal => "decimal",
            Self::Boolean => "boolean",
            Self::Date => "date",
            Self::Datetime => "datetime",
            Self::Select => "select",
            Self::Multiselect => "multiselect",
            Self::Json => "json",
        }
    }

    pub fn from_storage(value: &str) -> Result<Self, SchemaResolutionError> {
        match value {
            "text" => Ok(Self::Text),
            "textarea" => Ok(Self::Textarea),
            "richtext" => Ok(Self::Richtext),
            "integer" => Ok(Self::Integer),
            "decimal" => Ok(Self::Decimal),
            "boolean" => Ok(Self::Boolean),
            "date" => Ok(Self::Date),
            "datetime" => Ok(Self::Datetime),
            "select" => Ok(Self::Select),
            "multiselect" => Ok(Self::Multiselect),
            "json" => Ok(Self::Json),
            _ => Err(SchemaResolutionError::InvalidStorageValue {
                field: "product_attributes.value_type",
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CategorySchemaMode {
    Inherit,
    UseSchema,
    CloneFromCategory,
    Custom,
}

impl CategorySchemaMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inherit => "inherit",
            Self::UseSchema => "use_schema",
            Self::CloneFromCategory => "clone_from_category",
            Self::Custom => "custom",
        }
    }

    pub fn from_storage(value: &str) -> Result<Self, SchemaResolutionError> {
        match value {
            "inherit" => Ok(Self::Inherit),
            "use_schema" => Ok(Self::UseSchema),
            "clone_from_category" => Ok(Self::CloneFromCategory),
            "custom" => Ok(Self::Custom),
            _ => Err(SchemaResolutionError::InvalidStorageValue {
                field: "category_attribute_schema_assignments.mode",
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CategoryAttributeBindingKind {
    Addition,
    Override,
    Removal,
}

impl CategoryAttributeBindingKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Addition => "addition",
            Self::Override => "override",
            Self::Removal => "removal",
        }
    }

    pub fn from_storage(value: &str) -> Result<Self, SchemaResolutionError> {
        match value {
            "addition" => Ok(Self::Addition),
            "override" => Ok(Self::Override),
            "removal" => Ok(Self::Removal),
            _ => Err(SchemaResolutionError::InvalidStorageValue {
                field: "category_attributes.binding_kind",
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveAttributeSource {
    Schema,
    Inherited,
    CloneSnapshot,
    CategoryLocal,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttributeVisibilityOverrides {
    pub is_filterable: Option<bool>,
    pub is_searchable: Option<bool>,
    pub is_sortable: Option<bool>,
    pub is_comparable: Option<bool>,
    pub show_on_storefront: Option<bool>,
    pub show_in_admin_grid: Option<bool>,
}

impl AttributeVisibilityOverrides {
    fn apply(&mut self, local: &Self) {
        if local.is_filterable.is_some() {
            self.is_filterable = local.is_filterable;
        }
        if local.is_searchable.is_some() {
            self.is_searchable = local.is_searchable;
        }
        if local.is_sortable.is_some() {
            self.is_sortable = local.is_sortable;
        }
        if local.is_comparable.is_some() {
            self.is_comparable = local.is_comparable;
        }
        if local.show_on_storefront.is_some() {
            self.show_on_storefront = local.show_on_storefront;
        }
        if local.show_in_admin_grid.is_some() {
            self.show_in_admin_grid = local.show_in_admin_grid;
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProductAttributeDefinition {
    pub id: Uuid,
    pub code: String,
    pub value_type: AttributeValueType,
    pub is_filterable: bool,
    pub is_searchable: bool,
    pub is_sortable: bool,
    pub is_comparable: bool,
    pub show_on_storefront: bool,
    pub show_in_admin_grid: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttributeBinding {
    pub attribute_id: Uuid,
    pub group_code: Option<String>,
    pub is_required: bool,
    pub is_disabled: bool,
    pub position: i32,
    #[serde(default)]
    pub visibility_overrides: AttributeVisibilityOverrides,
    #[serde(default = "empty_json_object")]
    pub validation_overrides: Value,
    pub source: EffectiveAttributeSource,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CategoryAttributeBinding {
    pub attribute_id: Uuid,
    pub group_code: Option<String>,
    pub binding_kind: CategoryAttributeBindingKind,
    pub is_required: Option<bool>,
    pub is_disabled: bool,
    pub position: Option<i32>,
    #[serde(default)]
    pub visibility_overrides: AttributeVisibilityOverrides,
    #[serde(default = "empty_json_object")]
    pub validation_overrides: Value,
}

fn empty_json_object() -> Value {
    Value::Object(Default::default())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProductAttributeSchema {
    pub id: Uuid,
    pub code: String,
    pub attributes: Vec<AttributeBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CatalogCategorySchema {
    pub category_id: Uuid,
    pub parent_category_id: Option<Uuid>,
    pub kind: CatalogCategoryKind,
    pub mode: CategorySchemaMode,
    pub schema_id: Option<Uuid>,
    pub clone_snapshot: Vec<AttributeBinding>,
    pub local_attributes: Vec<CategoryAttributeBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EffectiveProductForm {
    pub category_id: Uuid,
    pub attributes: Vec<AttributeBinding>,
    pub detached_attribute_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchemaResolutionError {
    CategoryNotFound(Uuid),
    ParentCycle(Uuid),
    SchemaNotFound(Uuid),
    NonStructuralPrimaryCategory(Uuid),
    InvalidStorageValue { field: &'static str, value: String },
}

pub fn resolve_effective_product_form(
    primary_category_id: Uuid,
    categories: &HashMap<Uuid, CatalogCategorySchema>,
    schemas: &HashMap<Uuid, ProductAttributeSchema>,
    existing_value_attribute_ids: &[Uuid],
) -> Result<EffectiveProductForm, SchemaResolutionError> {
    let category = categories
        .get(&primary_category_id)
        .ok_or(SchemaResolutionError::CategoryNotFound(primary_category_id))?;
    if category.kind != CatalogCategoryKind::Structural {
        return Err(SchemaResolutionError::NonStructuralPrimaryCategory(
            primary_category_id,
        ));
    }

    let mut visiting = HashSet::new();
    let mut attributes =
        resolve_category_attributes(primary_category_id, categories, schemas, &mut visiting)?;
    attributes.sort_by(|left, right| {
        left.position
            .cmp(&right.position)
            .then_with(|| left.attribute_id.cmp(&right.attribute_id))
    });

    let effective_ids = attributes
        .iter()
        .filter(|binding| !binding.is_disabled)
        .map(|binding| binding.attribute_id)
        .collect::<HashSet<_>>();
    let detached_attribute_ids = existing_value_attribute_ids
        .iter()
        .copied()
        .filter(|attribute_id| !effective_ids.contains(attribute_id))
        .collect::<Vec<_>>();

    Ok(EffectiveProductForm {
        category_id: primary_category_id,
        attributes,
        detached_attribute_ids,
    })
}

fn resolve_category_attributes(
    category_id: Uuid,
    categories: &HashMap<Uuid, CatalogCategorySchema>,
    schemas: &HashMap<Uuid, ProductAttributeSchema>,
    visiting: &mut HashSet<Uuid>,
) -> Result<Vec<AttributeBinding>, SchemaResolutionError> {
    if !visiting.insert(category_id) {
        return Err(SchemaResolutionError::ParentCycle(category_id));
    }

    let category = categories
        .get(&category_id)
        .ok_or(SchemaResolutionError::CategoryNotFound(category_id))?;

    let mut bindings = match category.mode {
        CategorySchemaMode::Inherit => match category.parent_category_id {
            Some(parent_id) => {
                resolve_category_attributes(parent_id, categories, schemas, visiting)?
                    .into_iter()
                    .map(|mut binding| {
                        binding.source = EffectiveAttributeSource::Inherited;
                        binding
                    })
                    .collect()
            }
            None => Vec::new(),
        },
        CategorySchemaMode::UseSchema => {
            let schema_id = category
                .schema_id
                .ok_or(SchemaResolutionError::SchemaNotFound(Uuid::nil()))?;
            schemas
                .get(&schema_id)
                .ok_or(SchemaResolutionError::SchemaNotFound(schema_id))?
                .attributes
                .clone()
                .into_iter()
                .map(|mut binding| {
                    binding.source = EffectiveAttributeSource::Schema;
                    binding
                })
                .collect()
        }
        CategorySchemaMode::CloneFromCategory => category
            .clone_snapshot
            .clone()
            .into_iter()
            .map(|mut binding| {
                binding.source = EffectiveAttributeSource::CloneSnapshot;
                binding
            })
            .collect(),
        CategorySchemaMode::Custom => Vec::new(),
    };

    apply_local_category_bindings(&mut bindings, &category.local_attributes);
    visiting.remove(&category_id);
    Ok(bindings)
}

fn apply_local_category_bindings(
    bindings: &mut Vec<AttributeBinding>,
    local_bindings: &[CategoryAttributeBinding],
) {
    for local in local_bindings {
        match local.binding_kind {
            CategoryAttributeBindingKind::Addition => {
                if bindings
                    .iter()
                    .any(|binding| binding.attribute_id == local.attribute_id)
                {
                    apply_override(bindings, local);
                } else {
                    bindings.push(AttributeBinding {
                        attribute_id: local.attribute_id,
                        group_code: local.group_code.clone(),
                        is_required: local.is_required.unwrap_or(false),
                        is_disabled: local.is_disabled,
                        position: local.position.unwrap_or(0),
                        visibility_overrides: local.visibility_overrides.clone(),
                        validation_overrides: local.validation_overrides.clone(),
                        source: EffectiveAttributeSource::CategoryLocal,
                    });
                }
            }
            CategoryAttributeBindingKind::Override => apply_override(bindings, local),
            CategoryAttributeBindingKind::Removal => {
                if let Some(binding) = bindings
                    .iter_mut()
                    .find(|binding| binding.attribute_id == local.attribute_id)
                {
                    binding.is_disabled = true;
                    binding.source = EffectiveAttributeSource::CategoryLocal;
                }
            }
        }
    }
}

fn apply_override(bindings: &mut [AttributeBinding], local: &CategoryAttributeBinding) {
    if let Some(binding) = bindings
        .iter_mut()
        .find(|binding| binding.attribute_id == local.attribute_id)
    {
        if let Some(group_code) = &local.group_code {
            binding.group_code = Some(group_code.clone());
        }
        if let Some(is_required) = local.is_required {
            binding.is_required = is_required;
        }
        if let Some(position) = local.position {
            binding.position = position;
        }
        binding.is_disabled = local.is_disabled;
        binding
            .visibility_overrides
            .apply(&local.visibility_overrides);
        if local
            .validation_overrides
            .as_object()
            .is_some_and(|overrides| !overrides.is_empty())
        {
            merge_json_object(
                &mut binding.validation_overrides,
                &local.validation_overrides,
            );
        }
        binding.source = EffectiveAttributeSource::CategoryLocal;
    }
}

fn merge_json_object(base: &mut Value, local: &Value) {
    let Some(local) = local.as_object() else {
        return;
    };
    if !base.is_object() {
        *base = Value::Object(Default::default());
    }
    let base = base
        .as_object_mut()
        .expect("validation overrides were normalized to an object");
    for (key, value) in local {
        base.insert(key.clone(), value.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(attribute_id: Uuid, position: i32) -> AttributeBinding {
        AttributeBinding {
            attribute_id,
            group_code: Some("main".to_string()),
            is_required: false,
            is_disabled: false,
            position,
            visibility_overrides: AttributeVisibilityOverrides::default(),
            validation_overrides: Value::Object(Default::default()),
            source: EffectiveAttributeSource::Schema,
        }
    }

    fn structural_category(
        category_id: Uuid,
        parent_category_id: Option<Uuid>,
        mode: CategorySchemaMode,
    ) -> CatalogCategorySchema {
        CatalogCategorySchema {
            category_id,
            parent_category_id,
            kind: CatalogCategoryKind::Structural,
            mode,
            schema_id: None,
            clone_snapshot: Vec::new(),
            local_attributes: Vec::new(),
        }
    }

    #[test]
    fn child_inherits_parent_schema_and_overrides_required() {
        let attr_size = Uuid::new_v4();
        let parent_id = Uuid::new_v4();
        let child_id = Uuid::new_v4();
        let schema_id = Uuid::new_v4();

        let mut parent = structural_category(parent_id, None, CategorySchemaMode::UseSchema);
        parent.schema_id = Some(schema_id);

        let mut child = structural_category(child_id, Some(parent_id), CategorySchemaMode::Inherit);
        child.local_attributes.push(CategoryAttributeBinding {
            attribute_id: attr_size,
            group_code: Some("dimensions".to_string()),
            binding_kind: CategoryAttributeBindingKind::Override,
            is_required: Some(true),
            is_disabled: false,
            position: Some(10),
            visibility_overrides: AttributeVisibilityOverrides {
                is_filterable: Some(false),
                is_searchable: Some(true),
                ..Default::default()
            },
            validation_overrides: Value::Object(Default::default()),
        });

        let categories = HashMap::from([(parent_id, parent), (child_id, child)]);
        let schemas = HashMap::from([(
            schema_id,
            ProductAttributeSchema {
                id: schema_id,
                code: "shoes".to_string(),
                attributes: vec![binding(attr_size, 30)],
            },
        )]);

        let form = resolve_effective_product_form(child_id, &categories, &schemas, &[]).unwrap();

        assert_eq!(form.attributes.len(), 1);
        assert_eq!(form.attributes[0].group_code.as_deref(), Some("dimensions"));
        assert!(form.attributes[0].is_required);
        assert_eq!(form.attributes[0].position, 10);
        assert_eq!(
            form.attributes[0].visibility_overrides.is_filterable,
            Some(false)
        );
        assert_eq!(
            form.attributes[0].visibility_overrides.is_searchable,
            Some(true)
        );
        assert_eq!(
            form.attributes[0].source,
            EffectiveAttributeSource::CategoryLocal
        );
    }

    #[test]
    fn removal_disables_inherited_attribute_without_deleting_it() {
        let attr_brand = Uuid::new_v4();
        let parent_id = Uuid::new_v4();
        let child_id = Uuid::new_v4();
        let schema_id = Uuid::new_v4();

        let mut parent = structural_category(parent_id, None, CategorySchemaMode::UseSchema);
        parent.schema_id = Some(schema_id);

        let mut child = structural_category(child_id, Some(parent_id), CategorySchemaMode::Inherit);
        child.local_attributes.push(CategoryAttributeBinding {
            attribute_id: attr_brand,
            group_code: None,
            binding_kind: CategoryAttributeBindingKind::Removal,
            is_required: None,
            is_disabled: true,
            position: None,
            visibility_overrides: AttributeVisibilityOverrides::default(),
            validation_overrides: Value::Object(Default::default()),
        });

        let categories = HashMap::from([(parent_id, parent), (child_id, child)]);
        let schemas = HashMap::from([(
            schema_id,
            ProductAttributeSchema {
                id: schema_id,
                code: "base".to_string(),
                attributes: vec![binding(attr_brand, 1)],
            },
        )]);

        let form =
            resolve_effective_product_form(child_id, &categories, &schemas, &[attr_brand]).unwrap();

        assert!(form.attributes[0].is_disabled);
        assert_eq!(form.detached_attribute_ids, vec![attr_brand]);
    }

    #[test]
    fn clone_snapshot_does_not_follow_source_schema_changes() {
        let attr_material = Uuid::new_v4();
        let attr_season = Uuid::new_v4();
        let clone_id = Uuid::new_v4();

        let mut clone = structural_category(clone_id, None, CategorySchemaMode::CloneFromCategory);
        clone.clone_snapshot = vec![binding(attr_material, 1)];
        clone.local_attributes.push(CategoryAttributeBinding {
            attribute_id: attr_season,
            group_code: Some("marketing".to_string()),
            binding_kind: CategoryAttributeBindingKind::Addition,
            is_required: Some(false),
            is_disabled: false,
            position: Some(2),
            visibility_overrides: AttributeVisibilityOverrides::default(),
            validation_overrides: Value::Object(Default::default()),
        });

        let categories = HashMap::from([(clone_id, clone)]);
        let form =
            resolve_effective_product_form(clone_id, &categories, &HashMap::new(), &[]).unwrap();

        assert_eq!(
            form.attributes
                .iter()
                .map(|binding| binding.attribute_id)
                .collect::<Vec<_>>(),
            vec![attr_material, attr_season]
        );
        assert_eq!(
            form.attributes[0].source,
            EffectiveAttributeSource::CloneSnapshot
        );
        assert_eq!(
            form.attributes[1].source,
            EffectiveAttributeSource::CategoryLocal
        );
    }

    #[test]
    fn collection_category_cannot_define_primary_product_form() {
        let category_id = Uuid::new_v4();
        let mut category = structural_category(category_id, None, CategorySchemaMode::Custom);
        category.kind = CatalogCategoryKind::Collection;

        let error = resolve_effective_product_form(
            category_id,
            &HashMap::from([(category_id, category)]),
            &HashMap::new(),
            &[],
        )
        .unwrap_err();

        assert_eq!(
            error,
            SchemaResolutionError::NonStructuralPrimaryCategory(category_id)
        );
    }
}
