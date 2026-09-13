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
}

impl fmt::Display for RelationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for RelationType {
    type Err = ProductRelationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "cross_sell" | "cross-sell" | "crosssell" => Ok(Self::CrossSell),
            "up_sell" | "up-sell" | "upsell" => Ok(Self::UpSell),
            "related" => Ok(Self::Related),
            "accessory" | "accessories" => Ok(Self::Accessory),
            "alternative" | "alternatives" => Ok(Self::Alternative),
            other if !other.is_empty() => Ok(Self::Custom(other.to_owned())),
            _ => Err(ProductRelationError::InvalidRelationType(s.to_owned())),
        }
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
