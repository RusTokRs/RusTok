use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

use crate::error::ProductRelationError;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationType {
    CrossSell,
    UpSell,
    Related,
    Accessory,
    Alternative,
    Custom(String),
}

/// Maximum character width accepted by the `product_relations.relation_type` column.
pub const MAX_RELATION_TYPE_LEN: usize = 32;

impl RelationType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::CrossSell => "cross_sell",
            Self::UpSell => "up_sell",
            Self::Related => "related",
            Self::Accessory => "accessory",
            Self::Alternative => "alternative",
            Self::Custom(s) => s.as_str(),
        }
    }

    /// Returns the canonical, database-safe key for this relation type.
    ///
    /// Transport adapters normally construct this value through `FromStr`, but the DTO is public
    /// and callers can construct `RelationType::Custom` directly. Keeping the bound here makes
    /// every write and type-filter path reject a malformed key before PostgreSQL can turn it into
    /// a storage failure.
    pub fn storage_key(&self) -> Result<String, ProductRelationError> {
        normalize_relation_type(self.as_str())
            .map(|relation_type| relation_type.as_str().to_owned())
    }
}

fn normalize_relation_type(value: &str) -> Result<RelationType, ProductRelationError> {
    let normalized = value.trim().to_lowercase();
    if normalized.is_empty()
        || normalized.chars().count() > MAX_RELATION_TYPE_LEN
        || normalized.contains('\0')
    {
        return Err(ProductRelationError::InvalidRelationType(value.to_owned()));
    }

    Ok(match normalized.as_str() {
        "cross_sell" | "cross-sell" | "crosssell" => RelationType::CrossSell,
        "up_sell" | "up-sell" | "upsell" => RelationType::UpSell,
        "related" => RelationType::Related,
        "accessory" | "accessories" => RelationType::Accessory,
        "alternative" | "alternatives" => RelationType::Alternative,
        other => RelationType::Custom(other.to_owned()),
    })
}

impl fmt::Display for RelationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for RelationType {
    type Err = ProductRelationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        normalize_relation_type(s)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProductRelationDto {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub product_id: Uuid,
    pub related_product_id: Uuid,
    pub relation_type: RelationType,
    pub position: i32,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateProductRelationInput {
    pub product_id: Uuid,
    pub related_product_id: Uuid,
    pub relation_type: RelationType,
    pub position: Option<i32>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateProductRelationInput {
    pub position: Option<i32>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReorderProductRelationsInput {
    pub product_id: Uuid,
    pub relation_type: RelationType,
    pub ordered_relation_ids: Vec<Uuid>,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{MAX_RELATION_TYPE_LEN, RelationType};
    use crate::error::ProductRelationError;

    #[test]
    fn relation_type_normalizes_standard_aliases_and_custom_keys() {
        assert_eq!(
            RelationType::from_str(" Cross-Sell ").expect("standard alias"),
            RelationType::CrossSell
        );
        assert_eq!(
            RelationType::from_str("Seasonal").expect("custom type"),
            RelationType::Custom("seasonal".to_owned())
        );
    }

    #[test]
    fn relation_type_rejects_database_unsafe_custom_keys() {
        let oversized = "x".repeat(MAX_RELATION_TYPE_LEN + 1);
        assert!(matches!(
            RelationType::from_str(&oversized),
            Err(ProductRelationError::InvalidRelationType(_))
        ));
        assert!(matches!(
            RelationType::from_str("\0"),
            Err(ProductRelationError::InvalidRelationType(_))
        ));

        // Public DTO construction must not bypass the storage bound.
        assert!(matches!(
            RelationType::Custom(oversized).storage_key(),
            Err(ProductRelationError::InvalidRelationType(_))
        ));
    }
}
