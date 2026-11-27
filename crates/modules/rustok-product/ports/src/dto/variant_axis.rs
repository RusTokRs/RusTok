use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use super::MAX_PRODUCT_VARIANTS;

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
    // Keep this literal aligned with MAX_VARIANT_AXES below.
    #[validate(length(max = 32, message = "A Product can have at most 32 variant axes"))]
    #[validate(nested)]
    pub axes: Vec<VariantAxisInput>,
    /// Required for a new application; never part of the proposal fingerprint itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_state: Option<String>,
    /// Explicit full target tuple for each reassigned SKU. Omitted SKUs retain their EAV values.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    // Keep this literal aligned with MAX_VARIANT_AXIS_RECONFIGURATION_VARIANTS below.
    #[validate(length(max = 256, message = "A Product can reassign at most 256 variants"))]
    #[validate(nested)]
    pub reassignments: Vec<VariantAxisReassignmentInput>,
}

pub const MAX_VARIANT_AXES: usize = 32;
pub const MAX_VARIANT_AXIS_VALUES: usize = 256;
/// One Product-wide bound shared by creation, deletion, reconfiguration and complete projections.
/// Public detail/quote selection is complete within this bound, never a truncated SKU matrix.
pub const MAX_STOREFRONT_PRODUCT_VARIANTS: u64 = MAX_PRODUCT_VARIANTS as u64;
pub const MAX_VARIANT_AXIS_RECONFIGURATION_VARIANTS: usize = MAX_PRODUCT_VARIANTS;
pub const MAX_VARIANT_AXIS_RECONFIGURATION_CONFLICTS: usize = 16;
pub const MAX_VARIANT_AXIS_CONFLICT_EXAMPLES: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct VariantAxisReassignmentInput {
    pub variant_id: Uuid,
    // Keep this literal aligned with MAX_VARIANT_AXES.
    #[validate(length(max = 32, message = "A variant can have at most 32 axis assignments"))]
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
    // Keep this literal aligned with MAX_VARIANT_AXIS_VALUES below.
    #[validate(length(
        min = 1,
        max = 256,
        message = "A variant axis must contain 1-256 allowed options"
    ))]
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

#[cfg(test)]
mod dto_bound_tests {
    use super::*;
    use validator::Validate;

    fn axis() -> VariantAxisInput {
        VariantAxisInput {
            attribute_id: Uuid::new_v4(),
            position: 0,
            allowed_option_ids: vec![Uuid::new_v4()],
        }
    }

    #[test]
    fn variant_axis_dtos_enforce_owner_batch_bounds() {
        let mut axis_input = axis();
        axis_input.allowed_option_ids = vec![Uuid::new_v4(); MAX_VARIANT_AXIS_VALUES];
        assert!(axis_input.validate().is_ok());
        axis_input.allowed_option_ids.push(Uuid::new_v4());
        assert!(axis_input.validate().is_err());

        let mut axes = SetVariantAxesInput {
            axes: vec![axis(); MAX_VARIANT_AXES],
            ..Default::default()
        };
        assert!(axes.validate().is_ok());
        axes.axes.push(axis());
        assert!(axes.validate().is_err());

        let mut reassignments = SetVariantAxesInput {
            reassignments: vec![
                VariantAxisReassignmentInput {
                    variant_id: Uuid::new_v4(),
                    axis_values: Vec::new(),
                };
                MAX_VARIANT_AXIS_RECONFIGURATION_VARIANTS
            ],
            ..Default::default()
        };
        assert!(reassignments.validate().is_ok());
        reassignments
            .reassignments
            .push(VariantAxisReassignmentInput {
                variant_id: Uuid::new_v4(),
                axis_values: Vec::new(),
            });
        assert!(reassignments.validate().is_err());

        let mut reassignment = VariantAxisReassignmentInput {
            variant_id: Uuid::new_v4(),
            axis_values: vec![
                VariantAxisValueInput {
                    attribute_id: Uuid::new_v4(),
                    option_id: Uuid::new_v4(),
                };
                MAX_VARIANT_AXES
            ],
        };
        assert!(reassignment.validate().is_ok());
        reassignment.axis_values.push(VariantAxisValueInput {
            attribute_id: Uuid::new_v4(),
            option_id: Uuid::new_v4(),
        });
        assert!(reassignment.validate().is_err());
    }
}
