/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttributeDefinitionCopy {
    pub label: String,
    pub help_text: Option<String>,
    pub facet_label: Option<String>,
    pub seo_label: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttributeDefinitionPatch {
    pub translation: Option<AttributeDefinitionCopy>,
    pub is_filterable: Option<bool>,
    pub is_searchable: Option<bool>,
    pub is_sortable: Option<bool>,
    pub is_comparable: Option<bool>,
    pub show_on_storefront: Option<bool>,
    pub show_in_admin_grid: Option<bool>,
    pub search_weight: Option<i32>,
    pub position: Option<i32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttributeOptionDefinitionPatch {
    pub label: Option<String>,
    pub position: Option<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributeDefinitionAction {
    Update,
    Archive,
    Restore,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateAttributeDefinitionDraft {
    pub attribute_id: String,
    pub option_id: Option<String>,
    pub locale: String,
    pub expected_state: String,
    pub action: AttributeDefinitionAction,
    pub attribute_patch: Option<AttributeDefinitionPatch>,
    pub option_patch: Option<AttributeOptionDefinitionPatch>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttributeLifecycleRequest {
    pub attribute_id: Option<String>,
    pub option_id: Option<String>,
    pub include_archived: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttributeDefinition {
    pub id: String,
    pub code: String,
    pub value_type: String,
    pub scope: String,
    pub is_localized: bool,
    pub is_filterable: bool,
    pub is_searchable: bool,
    pub is_sortable: bool,
    pub is_comparable: bool,
    pub show_on_storefront: bool,
    pub show_in_admin_grid: bool,
    pub search_weight: i32,
    pub position: i32,
    pub is_archived: bool,
    pub translation: Option<AttributeDefinitionCopy>,
    pub state_token: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttributeOptionDefinition {
    pub id: String,
    pub attribute_id: String,
    pub code: String,
    pub position: i32,
    pub is_archived: bool,
    pub attribute_archived: bool,
    pub label: Option<String>,
    pub state_token: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributeReferenceKind {
    ProductValue,
    VariantValue,
    SchemaBinding,
    CategoryBinding,
    CloneSnapshot,
    VariantAxis,
    AllowedOption,
    AttributeGroup,
    ChannelSetting,
    OptionRule,
    VirtualCategory,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttributeReference {
    pub kind: AttributeReferenceKind,
    pub resource_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttributeReferenceImpact {
    pub references: Vec<AttributeReference>,
    pub has_more: bool,
    pub inspection_complete: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttributeLifecycleWorkspace {
    pub attributes: Vec<AttributeDefinition>,
    pub options: Option<Vec<AttributeOptionDefinition>>,
    pub impact: Option<AttributeReferenceImpact>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributeDefinitionRefusal {
    Stale,
    InUse,
    InspectionLimit,
    NotFound,
    ParentArchived,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttributeDefinitionResult {
    pub attribute_id: String,
    pub option_id: Option<String>,
    pub applied: bool,
    pub state_token: Option<String>,
    pub refusal: Option<AttributeDefinitionRefusal>,
    pub impact: Option<AttributeReferenceImpact>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum AttributeDefinitionOutcome {
    Decision { result: AttributeDefinitionResult },
    Rejected { code: String },
    Unknown,
}
