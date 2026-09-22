use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Product {
    pub id: String,
    pub name: String,
    pub status: ProductStatus,
    pub price: Option<f64>,
    pub tenant_name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProductStatus {
    Active,
    Draft,
    Archived,
    #[serde(other)]
    Unknown,
}

impl std::fmt::Display for ProductStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProductStatus::Active => write!(f, "Active"),
            ProductStatus::Draft => write!(f, "Draft"),
            ProductStatus::Archived => write!(f, "Archived"),
            ProductStatus::Unknown => write!(f, "Unknown"),
        }
    }
}
