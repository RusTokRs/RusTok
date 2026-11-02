use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum VariantAxisPolicy {
    #[default]
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
}

impl std::str::FromStr for VariantAxisPolicy {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "forbidden" => Ok(Self::Forbidden),
            "allowed" => Ok(Self::Allowed),
            "required" => Ok(Self::Required),
            _ => Err("variant_axis_policy must be forbidden, allowed, or required"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VariantAxisPolicyViolationKind {
    AxisNotAllowed,
    RequiredAxisMissing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariantAxisPolicyViolation {
    pub product_id: Uuid,
    pub category_id: Uuid,
    pub attribute_id: Uuid,
    pub kind: VariantAxisPolicyViolationKind,
}

/// Bounded, tenant-scoped diagnostics. No authored labels, SQL or arbitrary error text.
#[derive(Debug, Clone)]
pub struct VariantAxisPolicyConflict {
    pub violations: Vec<VariantAxisPolicyViolation>,
    pub has_more: bool,
}

impl std::fmt::Display for VariantAxisPolicyConflict {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Schema change would invalidate existing variant axes. "
        )?;
        for violation in self.violations.iter().take(8) {
            let reason = match violation.kind {
                VariantAxisPolicyViolationKind::AxisNotAllowed => "axis is no longer allowed",
                VariantAxisPolicyViolationKind::RequiredAxisMissing => "required axis is missing",
            };
            write!(
                formatter,
                "Product {}, category {}, attribute {}: {}; ",
                violation.product_id, violation.category_id, violation.attribute_id, reason
            )?;
        }
        if self.has_more || self.violations.len() > 8 {
            write!(formatter, "more affected products exist. ")?;
        }
        write!(
            formatter,
            "Reconfigure the affected products before changing the policy."
        )
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

/// The existing axes command also owns safe reconfiguration. Omitted new fields serialize
/// exactly as before, so persisted pre-preview request hashes remain replayable.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema, Validate)]
pub struct SetVariantAxesInput {
    #[validate(nested)]
    pub axes: Vec<VariantAxisInput>,
    /// Required for a new application; never part of the proposal fingerprint itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_state: Option<String>,
    /// Explicit full target tuple for each reassigned SKU. Omitted SKUs retain their EAV values.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[validate(nested)]
    pub reassignments: Vec<VariantAxisReassignmentInput>,
}

pub const MAX_VARIANT_AXES: usize = 32;
pub const MAX_VARIANT_AXIS_VALUES: usize = 256;
/// Public detail/quote selection is complete within this bound; never a truncated SKU matrix.
pub const MAX_STOREFRONT_PRODUCT_VARIANTS: u64 = 256;
pub const MAX_VARIANT_AXIS_RECONFIGURATION_VARIANTS: usize = 256;
pub const MAX_VARIANT_AXIS_RECONFIGURATION_CONFLICTS: usize = 16;
pub const MAX_VARIANT_AXIS_CONFLICT_EXAMPLES: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct VariantAxisReassignmentInput {
    pub variant_id: Uuid,
    #[validate(nested)]
    pub axis_values: Vec<VariantAxisValueInput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum VariantAxesConflictKind {
    MissingAssignment,
    DuplicateAssignment,
    UnexpectedAssignment,
    ValueNotAllowed,
    DuplicateCombination,
    MultipleDefaultVariants,
}

impl VariantAxesConflictKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MissingAssignment => "missing_assignment",
            Self::DuplicateAssignment => "duplicate_assignment",
            Self::UnexpectedAssignment => "unexpected_assignment",
            Self::ValueNotAllowed => "value_not_allowed",
            Self::DuplicateCombination => "duplicate_combination",
            Self::MultipleDefaultVariants => "multiple_default_variants",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VariantAxesConflictResponse {
    pub kind: VariantAxesConflictKind,
    pub attribute_id: Option<Uuid>,
    pub option_id: Option<Uuid>,
    /// Bounded examples, not an instruction to delete or merge these SKUs.
    pub variant_ids: Vec<Uuid>,
    pub has_more_variants: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VariantAxesPreviewVariant {
    pub variant_id: Uuid,
    pub sku: Option<String>,
    pub current_values: Vec<VariantAxisValueInput>,
    pub proposed_values: Vec<VariantAxisValueInput>,
    pub changes: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VariantAxesPreviewResponse {
    pub product_id: Uuid,
    pub product_revision: i32,
    /// Opaque, proposal-specific predecessor token; not a permission or a receipt.
    pub state_token: String,
    pub variant_count: i32,
    pub can_apply: bool,
    pub has_changes: bool,
    pub conflicts: Vec<VariantAxesConflictResponse>,
    pub has_more_conflicts: bool,
    /// Complete within the owner limit. Oversized products fail closed instead of truncating.
    pub variants: Vec<VariantAxesPreviewVariant>,
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
