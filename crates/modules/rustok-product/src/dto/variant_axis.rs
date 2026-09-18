use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum VariantAxisPolicy {
    Forbidden,
    Allowed,
    Required,
}

impl VariantAxisPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Forbidden => "forbidden",
            Self::Allowed => "allowed",
            Self::Required => "required",
        }
    }

    pub fn from_str_lossy(s: &str) -> Self {
        match s {
            "allowed" => Self::Allowed,
            "required" => Self::Required,
            _ => Self::Forbidden,
        }
    }
}

impl Default for VariantAxisPolicy {
    fn default() -> Self {
        Self::Forbidden
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VariantAxisConfigResponse {
    pub id: Uuid,
    pub product_id: Uuid,
    pub attribute_id: Uuid,
    pub code: String,
    pub name: String,
    pub position: i32,
    pub allowed_values: Vec<AxisAllowedValueResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AxisAllowedValueResponse {
    pub option_id: Uuid,
    pub value: String,
    pub position: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct SetVariantAxesInput {
    #[validate(nested)]
    pub axes: Vec<VariantAxisInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct VariantAxisInput {
    pub attribute_id: Uuid,
    #[serde(default)]
    pub position: i32,
    #[validate(length(min = 1, message = "At least one option required per variant axis"))]
    pub allowed_option_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate, PartialEq, Eq)]
pub struct VariantAxisValueInput {
    pub attribute_id: Uuid,
    pub option_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct VariantAxisValueResponse {
    pub attribute_id: Uuid,
    pub option_id: Uuid,
    pub code: Option<String>,
    pub label: Option<String>,
}
